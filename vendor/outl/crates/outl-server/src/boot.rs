//! Boot opener: open the workspace, start the file watcher, wire the iroh
//! P2P transport, and run the background reconcile.
//!
//! This is the server's version of the desktop's `spawn_workspace_opener`.
//! The differences are all in the event path: where the desktop emits
//! `workspace-ready` / `peer-ops-changed` / `workspace-reconciled` through
//! the Tauri `AppHandle`, the server pushes the same events to the
//! [`EventHub`](crate::events::EventHub), which fans them out to the
//! connected browsers over WebSocket.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use outl_actions::SyncTransport;
use outl_core::id::ActorId;
use outl_tauri_shared::reconcile::spawn_background_reconcile;
use outl_tauri_shared::workspace_open::open_workspace_at;
use parking_lot::Mutex;
use tracing::{info, warn};

use crate::events::Hub;
use crate::state::AppState;

/// Open the workspace at `root`, start the watcher + iroh, and publish the
/// workspace. Runs on a background thread so the HTTP server is already
/// accepting connections (and the frontend can poll `workspace_stats`)
/// before the filesystem work completes.
pub fn spawn_workspace_opener(state: Arc<AppState>, root: PathBuf) {
    let hlc = state.hlc.clone();
    let events = state.events.clone();
    let workspace_slot = state.workspace.clone();
    let storage_root_slot = state.storage_root.clone();
    let workspace_guards = state.workspace_guards.clone();
    let iroh_slot = state.iroh_transport.clone();

    thread::Builder::new()
        .name("outl-boot-opener".into())
        .spawn(move || {
            if !root.exists() {
                warn!("workspace {} does not exist; nothing to open", root.display());
                return;
            }
            let actor = hlc.actor();
            let lru_cap = outl_config::load().storage.lru_cap;

            let workspace = match open_workspace_at(actor, &hlc, &root, lru_cap, &workspace_guards)
            {
                Ok(w) => w,
                Err(e) => {
                    warn!("workspace open failed for {}: {e}", root.display());
                    return;
                }
            };

            // Start the file watcher before publishing so the first peer
            // change after boot is captured.
            start_ops_watcher(&root, actor, events.clone());

            *workspace_slot.lock() = Some(workspace);
            *storage_root_slot.lock() = Some(root.clone());
            events.emit("workspace-ready");
            info!("workspace open complete: {}", root.display());

            // Wire the iroh P2P transport (best-effort, gated on
            // `[sync] transport = "iroh"`).
            wire_iroh(&iroh_slot, root.clone(), actor, events.clone());

            // Background reconcile: scan and reconcile orphan `.md` files.
            spawn_background_reconcile(
                workspace_slot,
                root,
                hlc,
                move || events.emit("workspace-reconciled"),
            );
        })
        .expect("spawning the boot opener thread should not fail");
}

/// Watch the workspace's `ops/` directory and emit `peer-ops-changed` when a
/// peer writes its `ops-<actor>.jsonl`. Scoped to `ops/` (not the root) to
/// avoid the feedback loop the desktop documents: our own `.md` projections
/// must not re-trigger a reload.
fn start_ops_watcher(root: &PathBuf, own_actor: ActorId, events: Hub) {
    let ops_dir = root.join("ops");
    if !ops_dir.exists() {
        return;
    }
    let own_ops_file = format!("ops-{own_actor}.jsonl");

    let mut debouncer = match notify_debouncer_full::new_debouncer(
        Duration::from_millis(150),
        None,
        move |result: notify_debouncer_full::DebounceEventResult| match result {
            Ok(batch) => {
                let interesting = batch.iter().any(|ev| {
                    ev.paths.iter().any(|p| {
                        let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
                        // Only a peer's ops file, not ours, not a lock/temp file.
                        name.starts_with("ops-")
                            && name.ends_with(".jsonl")
                            && name != own_ops_file
                            && !name.contains(".lock")
                            && !name.contains(".tmp")
                    })
                });
                if interesting {
                    events.emit("peer-ops-changed");
                }
            }
            Err(errors) => {
                for e in errors {
                    warn!("fs watcher error: {e}");
                }
            }
        },
    ) {
        Ok(d) => d,
        Err(e) => {
            warn!("ops watcher failed to start: {e}");
            return;
        }
    };

    if let Err(e) = debouncer.watch(&ops_dir, notify::RecursiveMode::NonRecursive) {
        warn!("ops watcher failed to watch {}: {e}", ops_dir.display());
        return;
    }

    // Keep the debouncer alive for the process lifetime.
    thread::Builder::new()
        .name("outl-ops-watcher".into())
        .spawn(move || {
            let _ = debouncer;
        })
        .expect("spawning the ops watcher thread should not fail");
}

/// Wire the iroh P2P transport into the running server when it is this
/// process's to bind. Best-effort: every failure degrades to the filesystem
/// watcher and is logged, never fatal.
fn wire_iroh(
    slot: &Arc<Mutex<Option<Arc<dyn SyncTransport>>>>,
    workspace_root: PathBuf,
    actor: ActorId,
    events: Hub,
) {
    let transport = match outl_sync_iroh::build_default_transport(&workspace_root) {
        Ok(outl_sync_iroh::TransportOutcome::Ready(t)) => t,
        Ok(outl_sync_iroh::TransportOutcome::EndpointBusy(
            outl_sync_iroh::LeaseDenied::HeldByAnotherProcess,
        )) => {
            warn!(
                "another outl process holds the iroh endpoint; syncing through the shared ops/ dir"
            );
            return;
        }
        Ok(outl_sync_iroh::TransportOutcome::EndpointBusy(denied)) => {
            warn!("no iroh endpoint: {denied}; syncing through the shared ops/ dir");
            return;
        }
        Ok(outl_sync_iroh::TransportOutcome::Disabled) => {
            info!("P2P sync disabled in config; using filesystem watcher");
            return;
        }
        Err(e) => {
            warn!("iroh sync unavailable, using filesystem watcher: {e}");
            return;
        }
    };

    // Bridge the transport's "peer ops landed" signal to the same event the
    // file watcher emits, so the frontend reload path is identical whichever
    // delivery path wins. This is the server's replacement for
    // `outl_tauri_shared::iroh_sync::start_with_reload_bridge`, which needs a
    // Tauri `AppHandle` to emit.
    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<outl_actions::SyncProgress>();
    transport.set_progress_sink(progress_tx);
    let (peer_ready_tx, peer_ready_rx) = std::sync::mpsc::channel::<()>();
    transport.start(workspace_root, actor, peer_ready_tx);

    let ev = events.clone();
    thread::Builder::new()
        .name("outl-iroh-bridge".into())
        .spawn(move || {
            while peer_ready_rx.recv().is_ok() {
                ev.emit("peer-ops-changed");
            }
            info!("iroh peer-ready bridge ended");
        })
        .expect("spawning the iroh bridge thread should not fail");

    let ev2 = events.clone();
    thread::Builder::new()
        .name("outl-iroh-progress".into())
        .spawn(move || {
            while let Ok(progress) = progress_rx.recv() {
                ev2.emit_payload("sync-progress", &progress);
            }
            info!("iroh progress bridge ended");
        })
        .expect("spawning the iroh progress thread should not fail");

    *slot.lock() = Some(Arc::new(transport) as Arc<dyn SyncTransport>);
    info!("iroh sync transport wired");
}
