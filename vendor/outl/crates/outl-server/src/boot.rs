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
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use outl_actions::journal::reproject_stale_pages;
use outl_actions::{SyncEngine, SyncTransport};
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
    // One wake for the projection sweeper, fed by every source that can
    // move the tree: iroh ingest, and the ops/ watcher (whose peer files
    // are how sync degrades when no process holds the endpoint). Created
    // here so the sweeper outlives transport wiring and covers every path.
    let (wake_tx, wake_rx) = std::sync::mpsc::channel::<()>();

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
            // change after boot is captured. In shared-fs mode (no process
            // holds the iroh endpoint) its wake is the sweeper's only feed.
            start_ops_watcher(&root, actor, events.clone(), wake_tx.clone());

            *workspace_slot.lock() = Some(workspace);
            *storage_root_slot.lock() = Some(root.clone());
            events.emit("workspace-ready");
            info!("workspace open complete: {}", root.display());

            // Wire the iroh P2P transport (best-effort, gated on
            // `[sync] transport = "iroh"`).
            wire_iroh(&iroh_slot, root.clone(), actor, events.clone(), wake_tx.clone());

            // The `tree → .md` executor, alive on every sync path — iroh,
            // shared-fs, or neither. See its docs.
            spawn_projection_sweeper(state, wake_rx);

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
fn start_ops_watcher(
    root: &PathBuf,
    own_actor: ActorId,
    events: Hub,
    sweep_wake: std::sync::mpsc::Sender<()>,
) {
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
                    sweep_wake.send(()).ok();
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
    sweep_wake: std::sync::mpsc::Sender<()>,
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
    // Tauri `AppHandle` to emit. The same wake feeds the projection sweeper
    // the opener spawned: ops arriving by sync move the tree and nothing
    // else renders the page onto disk, so a headless peer's files follow the
    // op log on this wake.
    let (progress_tx, progress_rx) = std::sync::mpsc::channel::<outl_actions::SyncProgress>();
    transport.set_progress_sink(progress_tx);
    let (peer_ready_tx, peer_ready_rx) = std::sync::mpsc::channel::<()>();
    transport.start(workspace_root.clone(), actor, peer_ready_tx);

    let ev = events.clone();
    thread::Builder::new()
        .name("outl-iroh-bridge".into())
        .spawn(move || {
            while peer_ready_rx.recv().is_ok() {
                ev.emit("peer-ops-changed");
                // A stopped sweeper must not silence the frontend event.
                sweep_wake.send(()).ok();
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
        .expect("spawning the iroh progress bridge should not fail");

    *slot.lock() = Some(Arc::new(transport) as Arc<dyn SyncTransport>);
    info!("iroh sync transport wired");
}

/// Floor on how often the `tree → .md` sweep runs, matching `outl serve`'s
/// `PROJECTION_MIN_INTERVAL`: the sweep renders every page to compare it
/// with disk, so a sync burst must not render the graph once per batch.
const PROJECTION_MIN_INTERVAL: Duration = Duration::from_secs(30);

/// The server's `tree → .md` executor, owned by the same wake as the
/// frontend event bridge.
///
/// Replays the op log into a throwaway tree (`SyncEngine::reload_workspace`,
/// exactly what `outl serve`'s sweep loop does) and re-projects only pages
/// the survey classifies stale. The throwaway tree is deliberate: the sweep
/// must not race live commands for the shared slot, and disk wants the op
/// log's truth, which the replay is. A refused or withheld page is named
/// once per change, never per sweep — a repeating daemon log is silence
/// with more lines.
fn spawn_projection_sweeper(state: Arc<AppState>, wake_rx: std::sync::mpsc::Receiver<()>) {
    thread::Builder::new()
        .name("outl-projection-sweeper".into())
        .spawn(move || {
            let hlc = state.hlc.clone();
            let mut last_sweep = Instant::now() - PROJECTION_MIN_INTERVAL;
            let mut last_reported: std::collections::BTreeSet<String> = Default::default();
            loop {
                // Idle: park until sync says the tree moved. Without a wake
                // there is nothing new to render, so the sweep sleeps on the
                // channel rather than on a clock — a poll loop is a CPU burn
                // that produces nothing on an idle always-on peer.
                if wake_rx.recv().is_err() {
                    break;
                }
                while wake_rx.try_recv().is_ok() {}
                // Floor: a sync burst renders the graph once, not per batch.
                // Wakes that land during the wait are coalesced — the replay
                // below reads the op log, which already holds them.
                let remaining = PROJECTION_MIN_INTERVAL.saturating_sub(last_sweep.elapsed());
                if !remaining.is_zero()
                    && wake_rx.recv_timeout(remaining) == Err(RecvTimeoutError::Disconnected)
                {
                    break;
                }
                last_sweep = Instant::now();

                let Some(root) = state.storage_root.lock().clone() else {
                    continue;
                };
                let engine = SyncEngine::new(root.clone(), hlc.actor());
                let fresh = match engine.reload_workspace(&hlc) {
                    Ok(ws) => ws,
                    Err(e) => {
                        warn!("projection sweep: op-log replay failed: {e}");
                        continue;
                    }
                };
                let sweep = reproject_stale_pages(&fresh, &root);
                if !sweep.written.is_empty() {
                    info!("projection sweep: re-projected {} page(s)", sweep.written.len());
                }
                let attention: std::collections::BTreeSet<String> = sweep
                    .withheld
                    .iter()
                    .map(|p| p.path.display().to_string())
                    .chain(sweep.refused.iter().map(|p| p.path.display().to_string()))
                    .collect();
                if attention != last_reported {
                    last_reported = attention.clone();
                    for path in &attention {
                        warn!("projection sweep: page projection stalled for {path}");
                    }
                }
            }
            info!("projection sweeper ended");
        })
        .expect("spawning the projection sweeper should not fail");
}
