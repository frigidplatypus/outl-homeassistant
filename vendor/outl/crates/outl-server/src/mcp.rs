//! MCP bridge: expose outl's Model Context Protocol server over HTTP.
//!
//! `outl mcp serve` speaks JSON-RPC 2.0 over stdio (newline-delimited). This
//! module runs it as a child process and bridges each HTTP request to one
//! stdio round-trip, so any MCP client can reach every outl CLI subcommand as
//! a tool.
//!
//! The child is spawned once and kept alive. Spawn and I/O are serialized
//! through a single mutex because stdio is a single stream — the MCP protocol
//! is request/response, so one in-flight call at a time is correct. The
//! blocking stdio read runs on a `spawn_blocking` thread so it never occupies
//! a tokio worker. If the child dies it is reaped and respawned on the next
//! call.

use std::io::{BufRead, BufReader, Write};
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use parking_lot::Mutex;
use serde_json::Value;

use crate::state::AppState;

/// The stdio bridge to a single `outl mcp serve` child.
pub struct McpBridge {
    /// Serializes spawn + I/O over the single stdio stream.
    lock: Mutex<()>,
    child: Mutex<Option<Child>>,
    stdin: Mutex<Option<ChildStdin>>,
    stdout: Mutex<Option<BufReader<std::process::ChildStdout>>>,
    workspace: PathBuf,
}

impl McpBridge {
    pub fn new(workspace: PathBuf) -> Self {
        Self {
            lock: Mutex::new(()),
            child: Mutex::new(None),
            stdin: Mutex::new(None),
            stdout: Mutex::new(None),
            workspace,
        }
    }

    /// Reap a dead child so the next spawn starts clean. Caller holds `lock`.
    fn reap_dead(&self) {
        let mut child = self.child.lock();
        if let Some(c) = child.as_mut() {
            if c.try_wait().ok().flatten().is_some() {
                tracing::warn!("mcp child exited; will respawn");
                *child = None;
                *self.stdin.lock() = None;
                *self.stdout.lock() = None;
            }
        }
    }

    /// Spawn the child if it is not running. Caller holds `lock`.
    fn spawn_locked(&self) -> Result<(), String> {
        self.reap_dead();
        if self.child.lock().is_some() {
            return Ok(());
        }
        let mut child = Command::new("outl")
            .args(["mcp", "serve"])
            .arg("-w")
            .arg(&self.workspace)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("spawn `outl mcp serve`: {e}"))?;
        // Drain stderr to the log so a failed start is visible. The thread
        // ends when the child's stderr pipe closes (on reap/exit).
        if let Some(stderr) = child.stderr.take() {
            std::thread::Builder::new()
                .name("outl-mcp-stderr".into())
                .spawn(move || {
                    for line in BufReader::new(stderr).lines().flatten() {
                        tracing::warn!(target: "outl_mcp", "mcp: {line}");
                    }
                })
                .expect("spawning the mcp stderr drain should not fail");
        }
        let stdin = child.stdin.take().ok_or("no stdin")?;
        let stdout = child.stdout.take().ok_or("no stdout")?;
        *self.child.lock() = Some(child);
        *self.stdin.lock() = Some(stdin);
        *self.stdout.lock() = Some(BufReader::new(stdout));
        Ok(())
    }

    /// One JSON-RPC round-trip. Blocking — call via `spawn_blocking`.
    fn roundtrip(&self, request: &Value) -> Result<Value, String> {
        let _guard = self.lock.lock();
        self.spawn_locked()?;

        let mut stdin = self.stdin.lock();
        let mut stdout = self.stdout.lock();
        let stdin = stdin.as_mut().ok_or("mcp stdin closed")?;
        let stdout = stdout.as_mut().ok_or("mcp stdout closed")?;

        let mut line = serde_json::to_string(request).map_err(|e| e.to_string())?;
        line.push('\n');
        stdin
            .write_all(line.as_bytes())
            .and_then(|_| stdin.flush())
            .map_err(|e| format!("write to mcp: {e}"))?;

        // Read until a line parses as a JSON-RPC response with our id.
        let mut buf = String::new();
        loop {
            buf.clear();
            let n = stdout
                .read_line(&mut buf)
                .map_err(|e| format!("read from mcp: {e}"))?;
            if n == 0 {
                return Err("mcp server closed stdout".into());
            }
            let trimmed = buf.trim();
            if trimmed.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<Value>(trimmed) {
                // A response has "id" and either "result" or "error".
                if v.get("id").is_some() && (v.get("result").is_some() || v.get("error").is_some()) {
                    return Ok(v);
                }
                // Otherwise it's a notification/log line; keep reading.
            }
        }
    }

    /// Handle an MCP JSON-RPC request from the HTTP client. The blocking
    /// stdio round-trip runs on a `spawn_blocking` thread.
    pub async fn call(self: &std::sync::Arc<Self>, request: &Value) -> Response {
        let bridge = self.clone();
        let req = request.clone();
        let outcome = tokio::task::spawn_blocking(move || bridge.roundtrip(&req)).await;
        match outcome {
            Ok(Ok(resp)) => (StatusCode::OK, Json(resp)).into_response(),
            Ok(Err(e)) => Self::error_response(request, e),
            Err(e) => Self::error_response(request, format!("mcp task: {e}")),
        }
    }

    fn error_response(request: &Value, msg: String) -> Response {
        (
            StatusCode::BAD_GATEWAY,
            Json(serde_json::json!({
                "jsonrpc": "2.0",
                "id": request.get("id").cloned().unwrap_or(Value::Null),
                "error": { "code": -32603, "message": msg }
            })),
        )
            .into_response()
    }
}

/// `POST /api/mcp` — forward a JSON-RPC request to the MCP child.
pub async fn mcp_handler(
    State(state): State<std::sync::Arc<AppState>>,
    Json(req): Json<Value>,
) -> Response {
    state.mcp.call(&req).await
}
