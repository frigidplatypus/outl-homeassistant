//! `outl capture` end to end, through the real binary (RFC 0363).
//!
//! The library side is covered by `outl-actions`' own tests, which call
//! `write_capture` / `pending_captures` directly. Nothing exercised the
//! command: argument parsing, the `-` stdin path, the JSON envelope, or
//! the exit code. Every sibling CLI surface is covered that way by
//! `cli_machine.rs`, and this was the one recent addition that was not.
//!
//! Lives in its own file rather than at the end of `cli_machine.rs`
//! because that file is at 585 lines against a 600-line ratchet.

use std::io::Write;
use std::process::{Command, Stdio};

use serde_json::Value;
use tempfile::TempDir;

fn outl() -> Command {
    Command::new(env!("CARGO_BIN_EXE_outl"))
}

fn ok(out: std::process::Output) -> Value {
    if !out.status.success() {
        panic!(
            "command failed:\nstatus: {:?}\nstdout: {}\nstderr: {}",
            out.status,
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr),
        );
    }
    serde_json::from_slice(&out.stdout).expect("non-JSON stdout")
}

fn init_workspace() -> TempDir {
    let dir = TempDir::new().unwrap();
    assert!(
        outl()
            .arg("init")
            .arg(dir.path())
            .status()
            .expect("init failed")
            .success(),
        "outl init must succeed"
    );
    dir
}

fn capture(ws: &TempDir) -> Command {
    let mut c = outl();
    c.args(["--workspace"]).arg(ws.path()).arg("capture");
    c
}

/// The whole point of the command: a line lands in the inbox without the
/// workspace being opened, so no `ops-<ulid>.jsonl` is left behind by a
/// second actor (root `CLAUDE.md` invariant 11).
/// Every `ops-*.jsonl` in the workspace, by name.
///
/// `outl init` writes one (the journal template page), so the claim
/// under test is a *delta*: capture adds no op-log file of its own.
fn op_logs(ws: &TempDir) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(ws.path().join("ops"))
        .map(|rd| {
            rd.filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .filter(|n| n.ends_with(".jsonl"))
                .collect()
        })
        .unwrap_or_default();
    names.sort();
    names
}

#[test]
fn capture_writes_a_file_without_opening_the_workspace() {
    let ws = init_workspace();
    let before = op_logs(&ws);

    let env = ok(capture(&ws)
        .args(["read this later", "--json"])
        .output()
        .unwrap());

    assert_eq!(env["ok"], true);
    let id = env["data"]["id"].as_str().expect("the reply names the id");
    let file = ws.path().join(".outl/captures").join(format!("{id}.json"));
    assert!(file.is_file(), "expected a capture at {}", file.display());

    // The reason the command exists rather than being `outl daily
    // append`: no workspace boot, so no op written under a second actor
    // and no fresh `ops-<ulid>.jsonl` per call (the litter root
    // `CLAUDE.md` invariant 11 measures).
    assert_eq!(
        op_logs(&ws),
        before,
        "capture must not add an op-log file of its own"
    );
}

#[test]
fn list_shows_what_is_waiting() {
    let ws = init_workspace();
    ok(capture(&ws).args(["first", "--json"]).output().unwrap());
    ok(capture(&ws).args(["second", "--json"]).output().unwrap());

    let env = ok(capture(&ws).args(["--list", "--json"]).output().unwrap());

    let waiting = env["data"]["waiting"]
        .as_array()
        .expect("waiting is a list");
    let texts: Vec<&str> = waiting.iter().filter_map(|c| c["text"].as_str()).collect();
    assert_eq!(texts, ["first", "second"], "listed in capture order");
    assert_eq!(env["data"]["skipped"], 0);
}

#[test]
fn a_dash_reads_the_line_from_stdin() {
    // How a macOS Service or an editor pipes the current selection in,
    // and the route the docs should lead with: an argument puts private
    // content in argv, which `/proc/<pid>/cmdline` exposes to other
    // local users on Linux.
    let ws = init_workspace();
    let mut child = capture(&ws)
        .args(["-", "--json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(b"piped in from a selection")
        .unwrap();

    let env = ok(child.wait_with_output().unwrap());

    assert_eq!(env["ok"], true);
    let listed = ok(capture(&ws).args(["--list", "--json"]).output().unwrap());
    assert_eq!(
        listed["data"]["waiting"][0]["text"],
        "piped in from a selection"
    );
}

/// A capture is one block, and the inbox is read by a drain that trusts
/// that. Landing newlines raw makes the renderer emit indented
/// continuation lines, which the parser reads back as tree structure and
/// `key:: value` properties.
#[test]
fn a_multi_line_capture_is_collapsed_to_one_line() {
    let ws = init_workspace();

    ok(capture(&ws)
        .args(["first\n\n  second  \nthird", "--json"])
        .output()
        .unwrap());

    let listed = ok(capture(&ws).args(["--list", "--json"]).output().unwrap());
    assert_eq!(listed["data"]["waiting"][0]["text"], "first second third");
}

#[test]
fn a_blank_capture_is_refused() {
    let ws = init_workspace();

    let out = capture(&ws).args(["   \n\t ", "--json"]).output().unwrap();

    assert!(
        !out.status.success(),
        "a blank capture must not report success"
    );
    let listed = ok(capture(&ws).args(["--list", "--json"]).output().unwrap());
    assert_eq!(
        listed["data"]["waiting"].as_array().unwrap().len(),
        0,
        "and nothing is left in the inbox"
    );
}

/// `--list` against a folder with nothing in it is not an error: the
/// inbox is allowed to be empty, and a non-zero exit here would make a
/// `watch`-style script treat "nothing waiting" as a failure.
#[test]
fn list_on_an_empty_inbox_succeeds() {
    let ws = init_workspace();

    let env = ok(capture(&ws).args(["--list", "--json"]).output().unwrap());

    assert_eq!(env["ok"], true);
    assert_eq!(env["data"]["waiting"].as_array().unwrap().len(), 0);
}
