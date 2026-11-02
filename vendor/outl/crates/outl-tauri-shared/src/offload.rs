//! Getting a command body off the Tauri IPC thread.
//!
//! A `#[tauri::command]` declared **without** `async fn` is registered
//! with `ExecutionContext::Blocking`, so Tauri runs it inline on the
//! thread that received the IPC message. Under `wry` / WKWebView that
//! thread is the UI main thread (the webview's custom URL-scheme
//! handler), which makes a slow synchronous body three problems at once:
//! every other `invoke` queues behind it, the window stops accepting
//! keystrokes ([`UX.md` → Never block a keystroke]), and on iOS the
//! scene-update watchdog kills the process once the main thread has been
//! busy for 10 wall-clock seconds (`FRONTBOARD` 0x8BADF00D).
//!
//! The third one is not hypothetical. `commands::peers::peer_status` is
//! polled every 5s by both clients and reached `JsonlStorage::open` →
//! `load_actor_indexes` → `NodeIndex::load` → `sidecar::load_entries`
//! across a whole 2,800-page workspace. Nine crash reports off one
//! iPhone carry the identical `com.apple.main-thread` stack and 33s of
//! CPU per launch: the app was killed before it could paint.
//!
//! The shape that fixes it is the one `commands::page_backlinks`
//! introduced, and this module is its single owner. Three steps:
//!
//! 1. the catalog entry becomes `async fn` — only those get Tauri's
//!    threadpool (see [`crate::wrappers::catalog`]);
//! 2. the body clones what it needs **out of the borrowed `AppHost`
//!    before awaiting**, because a blocking task needs `'static`:
//!    `storage_root()`, `workspace_arc()`, `exec_registry()`, and
//!    `hlc().clone()` — an `HlcGenerator` is `Arc`-backed, so a clone is
//!    the *same* generator, never a second one issuing duplicate
//!    `(time, counter, actor)` triples on one actor id;
//! 3. the blocking half runs through [`run`], and takes the workspace
//!    lock — when it needs one at all — through [`locked`] /
//!    [`locked_mut`], for as short a span as it can.
//!
//! What this does **not** change is lock order: the workspace `Mutex`
//! stays the outermost lock, exactly as on the IPC thread. Moving a body
//! to the pool makes the stall concurrent, not free — see
//! `commands::page_backlinks` for the ABBA deadlock that lives one step
//! the other way round.
//!
//! [`UX.md` → Never block a keystroke]: https://github.com/outlmd/outl/blob/main/UX.md#2-never-block-a-keystroke

use outl_core::workspace::Workspace;
use parking_lot::Mutex;

use crate::state::ERR_LOADING;

/// Run `job` on the blocking pool and await its result.
///
/// `what` names the command, and is used only for the join-error text —
/// one owner for that wording, so a panic inside a body reads as
/// `"<command> task join: …"` on every surface instead of whatever each
/// call site invented. A panic in the job surfaces here as an `Err`
/// rather than as a future the frontend waits on forever.
pub async fn run<T, F>(what: &'static str, job: F) -> Result<T, String>
where
    F: FnOnce() -> Result<T, String> + Send + 'static,
    T: Send + 'static,
{
    tauri::async_runtime::spawn_blocking(job)
        .await
        .map_err(|e| format!("{what} task join: {e}"))?
}

/// [`crate::helpers::with_ws`] against the slot handle itself.
///
/// A body that has been moved onto the pool no longer has an `AppHost`
/// to ask (the host is borrowed; the job is `'static`), so it carries an
/// `Arc<Mutex<Option<Workspace>>>` from [`crate::AppHost::workspace_arc`]
/// instead. Same contract as `with_ws`, including the [`ERR_LOADING`]
/// sentinel while no workspace is published — which `with_ws` now
/// delegates here, so the two cannot answer "is a workspace open" two
/// different ways.
pub fn locked<T, F>(slot: &Mutex<Option<Workspace>>, f: F) -> Result<T, String>
where
    F: FnOnce(&Workspace) -> Result<T, String>,
{
    let guard = slot.lock();
    match guard.as_ref() {
        Some(ws) => f(ws),
        None => Err(ERR_LOADING.to_string()),
    }
}

/// The `&mut` twin of [`locked`].
pub fn locked_mut<T, F>(slot: &Mutex<Option<Workspace>>, f: F) -> Result<T, String>
where
    F: FnOnce(&mut Workspace) -> Result<T, String>,
{
    let mut guard = slot.lock();
    match guard.as_mut() {
        Some(ws) => f(ws),
        None => Err(ERR_LOADING.to_string()),
    }
}
