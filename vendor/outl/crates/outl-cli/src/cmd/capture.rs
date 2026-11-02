//! `outl capture` — drop a line into the workspace's capture inbox
//! without opening the workspace (RFC 0363).
//!
//! This is the entry point every scriptable gesture on a desktop goes
//! through: a macOS Service or Shortcut, Raycast, Alfred, a hotkey
//! daemon, a shell alias. The line waits as one file in
//! `<root>/.outl/captures/` and the GUI app turns it into a block the
//! next time it is open, under its own actor.
//!
//! **Why not `outl daily append`.** That writes the block straight into
//! the op log and is the better command when a workspace boot is free:
//! the line is durable and syncing before the process exits. Two things
//! make it the wrong tool for a capture gesture. It boots the workspace
//! — seconds on a large one, for one line — and if a GUI client is
//! running it holds the device's write actor, so the CLI falls back to
//! an ephemeral one and leaves a fresh `ops-<ulid>.jsonl` behind per
//! call (the litter root `CLAUDE.md` invariant 11 measures). Writing a
//! file costs neither.
//!
//! The trade is stated rather than hidden: a capture that nothing drains
//! is a line that has not synced. `outl capture --list` is how you see
//! what is still waiting.

use std::io::Read;
use std::path::Path;

use clap::Args;
use serde_json::{json, Value};

use outl_actions::capture::{inbox_in_workspace, pending_captures, shared_line, write_capture};
use outl_ws::layout::Paths;

use crate::output::{codes, emit, ApiError};

/// `outl capture …` arguments.
#[derive(Args, Debug)]
pub struct CaptureArgs {
    /// The line to capture. `-` reads it from stdin, which is how a
    /// macOS Service or an editor pipes the current selection in.
    /// Omit it with `--list`.
    pub text: Option<String>,

    /// Show what is still waiting in the inbox instead of adding to it.
    #[arg(long)]
    pub list: bool,

    /// Force JSON output.
    #[arg(long)]
    pub json: bool,
}

/// Run `outl capture` against the workspace at `path`.
pub fn run(args: &CaptureArgs, path: &Path) -> i32 {
    if args.list {
        return emit(args.json, list(path), print_waiting);
    }
    emit(args.json, add(args, path), |v| {
        println!(
            "captured {} — lands in today's journal the next time outl is open",
            v.get("id").and_then(Value::as_str).unwrap_or("?")
        );
    })
}

/// The inbox folder, once `path` is known to be a workspace.
///
/// Checked with the same `.outl/` marker `outl_ws::open` uses, and
/// deliberately without opening: booting the workspace is the cost this
/// command exists to avoid, so it must not pay it just to validate.
fn inbox(path: &Path) -> Result<std::path::PathBuf, ApiError> {
    let paths = Paths::at(path.to_path_buf());
    if !paths.dot_outl.exists() {
        return Err(ApiError::new(
            codes::NO_WORKSPACE,
            format!("{} is not an outl workspace", paths.root.display()),
        ));
    }
    Ok(inbox_in_workspace(&paths.root))
}

fn add(args: &CaptureArgs, path: &Path) -> Result<Value, ApiError> {
    let dir = inbox(path)?;
    let text = match args.text.as_deref() {
        None => {
            return Err(ApiError::new(
                codes::INVALID_ARG,
                "nothing to capture — pass the line, or `-` to read it from stdin",
            ))
        }
        Some("-") => read_stdin()?,
        Some(text) => text.to_string(),
    };
    // Same rule every other capture writer uses: one capture is one
    // block, so a multi-line selection piped in becomes one line. Reach
    // for `outl daily append` when the shape of the text matters.
    let line = shared_line(Some(&text), None).ok_or_else(|| {
        ApiError::new(
            codes::INVALID_ARG,
            "there's nothing to save — the text is empty",
        )
    })?;
    // `internal` covers everything left: `shared_line` returning `Some`
    // is what rules out `ActionError::EmptyCapture`, so the only failures
    // that reach here are filesystem ones.
    let file = write_capture(&dir, &line, "cli", chrono::Utc::now().timestamp_millis())
        .map_err(ApiError::internal)?;
    let id = file
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or_default()
        .to_string();
    Ok(json!({ "id": id, "path": file.to_string_lossy(), "source": "cli" }))
}

fn read_stdin() -> Result<String, ApiError> {
    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .map_err(ApiError::internal)?;
    Ok(buf)
}

fn list(path: &Path) -> Result<Value, ApiError> {
    let dir = inbox(path)?;
    let (pending, skipped) = pending_captures(&dir);
    Ok(json!({
        "waiting": pending.iter().map(|c| json!({
            "id": c.id.to_string(),
            "text": c.record.text,
            "captured_at_ms": c.record.captured_at_ms,
            "source": c.record.source,
        })).collect::<Vec<_>>(),
        "skipped": skipped,
        "inbox": dir.display().to_string(),
    }))
}

fn print_waiting(v: &Value) {
    let waiting = v.get("waiting").and_then(Value::as_array);
    match waiting.map(|w| w.len()).unwrap_or(0) {
        0 => println!("nothing waiting in the capture inbox"),
        n => {
            println!("{n} waiting in the capture inbox:");
            for c in waiting.into_iter().flatten() {
                println!(
                    "  {}  {}",
                    c.get("id").and_then(Value::as_str).unwrap_or("?"),
                    c.get("text").and_then(Value::as_str).unwrap_or("")
                );
            }
        }
    }
    // A file the drain could not read is left in place, so it would
    // otherwise be invisible here as well as in the journal. It names
    // the folder rather than `outl doctor`, which has no inbox check —
    // pointing at a command that reports nothing is worse than saying
    // where the files are.
    if let Some(n) = v.get("skipped").and_then(Value::as_u64).filter(|n| *n > 0) {
        match v.get("inbox").and_then(Value::as_str) {
            Some(dir) => println!("{n} unreadable file(s) left in {dir}"),
            None => println!("{n} unreadable file(s) left in place"),
        }
    }
}
