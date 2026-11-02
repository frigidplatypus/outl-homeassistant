//! Read-only filesystem API over the workspace, for other Home Assistant
//! processes.
//!
//! The workspace is a shared volume: other addons and HA itself can read the
//! `.md` files directly. This API adds a convenient, path-traversal-safe way
//! to browse and read the workspace over HTTP (for dashboards, integrations,
//! or a quick `curl`).
//!
//! Writes are deliberately NOT exposed here. The correct write path for
//! external processes is the MCP server (every CLI subcommand as a tool) or
//! the `outl` CLI — both go through the op log, so the CRDT stays consistent.
//! A raw `.md` write is also picked up by the file watcher and reconciled,
//! but that is a fallback, not the intended path.

use std::path::{Component, Path, PathBuf};

use axum::extract::Query;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::Deserialize;
use outl_tauri_shared::AppHost;

#[derive(Deserialize)]
pub struct FsQuery {
    /// Workspace-relative path. Empty / omitted = the workspace root.
    path: Option<String>,
}

/// Resolve a workspace-relative path, refusing to escape the root.
fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim();
    if rel.is_empty() {
        return Ok(root.to_path_buf());
    }
    let candidate = root.join(rel);
    // Canonicalize both and confirm the candidate is under the root.
    let root = root
        .canonicalize()
        .map_err(|e| format!("workspace root: {e}"))?;
    let candidate = candidate
        .canonicalize()
        .map_err(|e| format!("path {rel}: {e}"))?;
    if !candidate.starts_with(&root) {
        return Err("path escapes the workspace".into());
    }
    // Reject any `..` component in the input itself (defense in depth).
    for c in Path::new(rel).components() {
        if matches!(c, Component::ParentDir) {
            return Err("path contains `..`".into());
        }
    }
    Ok(candidate)
}

/// `GET /api/fs/list?path=…` — list a directory's entries.
pub async fn list(Query(q): Query<FsQuery>, state: axum::extract::State<std::sync::Arc<crate::state::AppState>>) -> Response {
    let root = match state.storage_root() {
        Ok(r) => r,
        Err(e) => return bad(&e),
    };
    let dir = match resolve(&root, q.path.as_deref().unwrap_or("")) {
        Ok(d) => d,
        Err(e) => return bad(&e),
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(rd) => rd,
        Err(e) => return bad(&format!("read dir: {e}")),
    };
    let mut items = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
        items.push(serde_json::json!({ "name": name, "dir": is_dir, "size": size }));
    }
    items.sort_by(|a, b| {
        let ad = a["dir"].as_bool().unwrap_or(false);
        let bd = b["dir"].as_bool().unwrap_or(false);
        bd.cmp(&ad).then_with(|| a["name"].as_str().unwrap_or("").cmp(b["name"].as_str().unwrap_or("")))
    });
    (StatusCode::OK, Json(serde_json::json!({ "path": q.path.clone().unwrap_or_default(), "entries": items }))).into_response()
}

/// `GET /api/fs/read?path=…` — read a file's text content.
pub async fn read(Query(q): Query<FsQuery>, state: axum::extract::State<std::sync::Arc<crate::state::AppState>>) -> Response {
    let root = match state.storage_root() {
        Ok(r) => r,
        Err(e) => return bad(&e),
    };
    let file = match resolve(&root, q.path.as_deref().unwrap_or("")) {
        Ok(f) => f,
        Err(e) => return bad(&e),
    };
    if !file.is_file() {
        return bad("not a file");
    }
    match std::fs::read_to_string(&file) {
        Ok(text) => (StatusCode::OK, Json(serde_json::json!({ "path": q.path.clone().unwrap_or_default(), "content": text }))).into_response(),
        Err(e) => bad(&format!("read file: {e}")),
    }
}

fn bad(msg: &str) -> Response {
    (StatusCode::BAD_REQUEST, Json(serde_json::json!({ "error": msg }))).into_response()
}
