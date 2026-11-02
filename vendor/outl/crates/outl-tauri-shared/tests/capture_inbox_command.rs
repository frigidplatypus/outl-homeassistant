//! RFC 0363: `drain_capture_inbox` lands the captures as blocks,
//! projects them, and empties the inbox — from the mobile shell's inbox
//! and from the workspace's own, which is the one `outl capture` writes.

use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use outl_actions::SyncTransport;
use outl_core::hlc::HlcGenerator;
use outl_core::id::ActorId;
use outl_core::workspace::Workspace;
use outl_exec::RuntimeRegistry;
use outl_tauri_shared::capture_inbox::{drain_capture_dir, drain_capture_inbox};
use outl_tauri_shared::host::AppHost;
use parking_lot::Mutex;
use tempfile::TempDir;

struct TestHost {
    workspace: Arc<Mutex<Option<Workspace>>>,
    hlc: HlcGenerator,
    root: PathBuf,
    registry: Arc<RuntimeRegistry>,
}

impl AppHost for TestHost {
    fn workspace(&self) -> &Mutex<Option<Workspace>> {
        &self.workspace
    }
    fn workspace_arc(&self) -> Arc<Mutex<Option<Workspace>>> {
        self.workspace.clone()
    }
    fn hlc(&self) -> &HlcGenerator {
        &self.hlc
    }
    fn storage_root(&self) -> Result<PathBuf, String> {
        Ok(self.root.clone())
    }
    fn sync_transport(&self) -> Option<Arc<dyn SyncTransport>> {
        None
    }
    fn exec_registry(&self) -> Arc<RuntimeRegistry> {
        self.registry.clone()
    }
}

fn host() -> (TempDir, TestHost) {
    let tmp = TempDir::new().unwrap();
    let actor = ActorId::new();
    let hlc = HlcGenerator::new(actor);
    let ws = Workspace::open_in_memory(actor).unwrap();
    let host = TestHost {
        workspace: Arc::new(Mutex::new(Some(ws))),
        hlc,
        root: tmp.path().to_path_buf(),
        registry: Arc::new(RuntimeRegistry::new()),
    };
    (tmp, host)
}

#[test]
fn drained_captures_become_blocks_and_leave_the_inbox() {
    let (_tmp, host) = host();
    let inbox = TempDir::new().unwrap();
    let now = chrono::Utc::now().timestamp_millis();
    for (i, text) in ["first", "second"].iter().enumerate() {
        let id = ulid::Ulid::from_parts((now + i as i64) as u64, 1);
        let body = serde_json::json!({ "text": text, "captured_at_ms": now + i as i64, "source": "share" });
        fs::write(inbox.path().join(format!("{id}.json")), body.to_string()).unwrap();
    }

    let out = drain_capture_dir(&host, inbox.path()).expect("drain");

    assert_eq!(out.landed, 2);
    assert_eq!(out.page_ids.len(), 1, "both land in the same journal");
    assert_eq!(fs::read_dir(inbox.path()).unwrap().count(), 0);
    let again = drain_capture_dir(&host, inbox.path()).expect("second drain");
    assert_eq!(again.landed, 0);
}

#[test]
fn an_empty_inbox_is_a_no_op() {
    let (_tmp, host) = host();
    let inbox = TempDir::new().unwrap();
    let out = drain_capture_dir(&host, inbox.path()).expect("drain");
    assert_eq!(out.landed, 0);
    assert!(out.page_ids.is_empty());
}

#[test]
fn the_workspace_inbox_is_drained_without_any_env_var() {
    let (_tmp, host) = host();
    let inbox = outl_actions::capture::inbox_in_workspace(&host.storage_root().unwrap());
    let now = chrono::Utc::now().timestamp_millis();
    outl_actions::capture::write_capture(&inbox, "from a terminal", "cli", now).unwrap();

    // No `OUTL_CAPTURE_INBOX` here: a desktop has only this inbox, and
    // finding it must not depend on a shell having exported anything.
    let out = drain_capture_inbox(&host).expect("drain");

    assert_eq!(out.landed, 1);
    assert_eq!(fs::read_dir(&inbox).unwrap().count(), 0);
}
