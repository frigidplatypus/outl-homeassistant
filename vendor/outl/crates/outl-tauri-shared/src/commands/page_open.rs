//! The three page-open commands, and the one reason they live apart from
//! the rest of `commands::page`.
//!
//! They are not navigation logic — navigation is three lines each. They
//! are a **concurrency** problem, the same way `commands::page_backlinks`
//! is, and the reasoning is long enough that keeping it beside
//! `search_pages` buried it.
//!
//! ## The body was never the cost
//!
//! Measured on the reference workspace (2,923 pages, 222k ops, 20
//! actors), release build, in-sync journal:
//!
//! ```text
//! lock acquire (uncontended)   0.00 ms
//! open_today                   0.80 ms
//! reproject_stale_md           0.97 ms   ← writes NOTHING when in sync
//!   read .md                   0.02 ms
//!   hash .md                   0.00 ms
//!   read sidecar               0.03 ms
//!   render_page_md             0.90 ms
//! build_page_view              0.13 ms
//! TOTAL                        2.86 ms
//! ```
//!
//! `apply_page_md_with_sidecar_if_stale` returns `Ok(None)` before it
//! writes a byte whenever the render already matches disk, so the
//! "render + SHA-256 + atomic write + two fsyncs on every open" that this
//! module was suspected of does not happen on an in-sync page. 2,054 of
//! the 2,923 pages are in that state; 837 are genuinely stale and pay the
//! write once, after which they are not.
//!
//! ## The cost is the wait, and the wait was on the UI thread
//!
//! Every one of these takes the workspace mutex. So does the background
//! reconcile pass, which starts the instant `workspace-ready` fires —
//! which is exactly when the frontend asks for today's journal. Measured
//! on the same workspace, the two phases it takes that mutex for in "one
//! brief lock" over the whole workspace:
//!
//! ```text
//! scan_for_desynced_projections   129 ms   [under the workspace lock]
//! repair_namespaced_titles        185 ms   [under the workspace lock]
//! ```
//!
//! `parking_lot::Mutex` has no timeout. A synchronous `#[tauri::command]`
//! runs on the IPC thread, which under `wry` is the UI main thread, so
//! those ~314 ms were 314 ms of a window that painted nothing and
//! accepted no keystroke — and on a phone's flash, with a cold page
//! cache, longer. Going `async` does not shorten the wait; it moves it
//! somewhere the user cannot see ([`crate::offload`]).

use outl_actions::{find_by_slug, open_journal, open_or_create_by_name, open_today, PageKind};
use outl_core::hlc::HlcGenerator;
use outl_core::id::NodeId;
use outl_core::workspace::Workspace;
use parking_lot::Mutex;
use std::path::PathBuf;
use std::sync::Arc;

use crate::helpers::{build_page_view, parse_date, reproject_stale_md};
use crate::host::AppHost;
use crate::offload;
use crate::state::PageView;

/// Everything an off-thread page-open body needs from the host.
///
/// Cloned out of the borrowed [`AppHost`] **before** the `await`, because
/// a blocking task has to be `'static`. The [`HlcGenerator`] clone is the
/// subtle one and it is safe for the reason [`crate::offload`] states:
/// its state is `Arc`-backed, so a clone is the *same* generator, never a
/// second one minting duplicate `(time, counter, actor)` triples on one
/// actor id.
struct OpenHost {
    workspace: Arc<Mutex<Option<Workspace>>>,
    root: PathBuf,
    hlc: HlcGenerator,
}

impl OpenHost {
    fn of<S: AppHost>(state: &S) -> Result<Self, String> {
        Ok(Self {
            workspace: state.workspace_arc(),
            root: state.storage_root()?,
            hlc: state.hlc().clone(),
        })
    }

    /// Refresh `id`'s `.md` when the tree has moved past it, then read the
    /// view back off disk — the tail all three open paths share.
    fn view_of(&self, id: NodeId, context: &str) -> Result<PageView, String> {
        offload::locked(&self.workspace, |ws| {
            // Refresh the `.md` the view reads when a peer's ops moved the
            // tree past the stale projection (issue #166); a no-op when in
            // sync. A refusal comes back as `md_ahead_of_log` and rides the
            // view to the user — the page still opens.
            let ahead = reproject_stale_md(ws, &self.root, id, context).ahead_of_log;
            let view = build_page_view(ws, &self.root, id).map_err(|e| e.to_string())?;
            Ok(view.with_ahead_of_log_check(ahead))
        })
    }
}

/// Open (creating if absent) today's journal — the first thing every
/// client asks for, and the one the user is waiting on.
pub async fn open_today_journal<S: AppHost>(state: &S) -> Result<PageView, String> {
    let host = OpenHost::of(state)?;
    offload::run("open_today_journal", move || {
        let id = offload::locked_mut(&host.workspace, |ws| {
            open_today(ws, &host.hlc).map_err(|e| e.to_string())
        })?;
        host.view_of(id, "open_today_journal")
    })
    .await
}

/// Open the journal for an ISO date slug.
pub async fn open_journal_for<S: AppHost>(state: &S, slug: String) -> Result<PageView, String> {
    let host = OpenHost::of(state)?;
    // Parsed before the hop: a bad slug is the caller's bug and costs no
    // thread.
    let date = parse_date(&slug)?;
    offload::run("open_journal_for", move || {
        let id = offload::locked_mut(&host.workspace, |ws| {
            open_journal(ws, &host.hlc, date).map_err(|e| e.to_string())
        })?;
        host.view_of(id, &format!("open_journal_for {slug}"))
    })
    .await
}

/// The command name says "slug" for backward-compat with the frontend,
/// but the input is whatever the user typed / clicked — try the literal
/// first (programmatic callers and the picker pass a clean slug), then
/// fall through to `open_or_create_by_name`, which slugifies the input
/// for the disk path and keeps the original as the page title. That way
/// clicking a ref to a page that doesn't exist still opens a fresh page
/// in the same round-trip.
///
/// The resolve runs under a **read** lock and only falls through to the
/// mutating one when the page really is new, so the common case (a page
/// that exists) never takes the exclusive path.
///
/// Before building the view, the page's `.md` is refreshed when the tree
/// has moved past it. Without that, a page synced from a peer exists in
/// the in-memory CRDT tree (so it shows up in search) but its `.md` was
/// never written on this device; `read_page_outline` reads through
/// `outl_md::read_for_rewrite`, where a **missing** file legitimately
/// returns `""` (an unreadable one is an error), `parse("")` produces an
/// empty outline, and the page opens blank (issue #120). The staleness
/// gate is what keeps that off the hot path: `build_sidecar` stamps
/// `last_synced_at: now()`, so an unconditional projection would churn
/// the sidecar on every navigation even when nothing changed.
pub async fn open_page_by_slug<S: AppHost>(state: &S, slug: String) -> Result<PageView, String> {
    let host = OpenHost::of(state)?;
    offload::run("open_page_by_slug", move || {
        let existing = offload::locked(&host.workspace, |ws| Ok(find_by_slug(ws, &slug)))?;
        let id = match existing {
            Some(id) => id,
            None => offload::locked_mut(&host.workspace, |ws| {
                open_or_create_by_name(ws, &host.hlc, &slug, PageKind::Page)
                    .map_err(|e| e.to_string())
            })?,
        };
        host.view_of(id, &format!("open_page_by_slug {slug}"))
    })
    .await
}
