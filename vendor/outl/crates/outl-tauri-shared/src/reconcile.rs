//! The background reconcile sweep both GUI clients run once the
//! workspace is published, and the one rule it exists to hold: **a
//! user-initiated command must be able to interleave.**
//!
//! Each client used to keep its own copy of this pass, and the copies had
//! drifted by omission — the same failure
//! [`crate::wrappers::catalog`] exists to delete. The desktop had the
//! pacing (`try_lock` + retry + [`yield_to_user`]) and no
//! desynced-projection recovery at all; mobile had the recovery and took
//! every lock with a blocking `lock()`. Mobile's own comment claimed it
//! "mirrors the desktop's `spawn_background_reconcile`", which nothing
//! could fail.
//!
//! ## The wait was never the body
//!
//! Measured on a clone of a real 2,923-page / 20-actor / 366 MB
//! workspace, release build: `open_today_journal`'s own work is **1.8
//! ms**, and it was observed blocking for **374–1,056 ms** purely waiting
//! on the workspace mutex. The sweep starts on `workspace-ready`, which
//! is exactly when the frontend issues its first `open_today_journal`, so
//! every launch booted into that window. Phase costs:
//!
//! ```text
//! scan_for_orphans                 170 ms   no lock
//! scan_for_desynced_projections    129 ms   one lock, whole workspace
//! repair_doubled_journal_titles      1 ms   one lock, whole workspace
//! repair_namespaced_titles         185 ms   one lock, whole workspace
//! ```
//!
//! `parking_lot::Mutex` has no timeout, so a command arriving mid-hold
//! just stops. Moving the open commands onto the blocking pool
//! ([`crate::offload`]) took that wait off the UI thread; it did not
//! shorten it.
//!
//! ## Two phases chunk, two cannot, and the difference is recorded
//!
//! A pass that used to run under one lock and now runs in chunks sees a
//! workspace that **changes between chunks**, so each one had to be asked
//! what it assumes is stable across the whole sweep:
//!
//! - **Orphan reconcile** — per page, independent. Chunked (this was
//!   already the desktop's shape, and is the shape everything else here
//!   copies).
//! - **Desync recovery** — the repair is per page and
//!   `recover_desynced_projection` re-validates the hash, the sidecar
//!   lockstep and every id under its *own* lock, so the scan's output is
//!   a hint and a stale hint costs `Ok(0)`. Chunked.
//! - **The desync scan** — not chunkable from this crate. The per-page
//!   predicate (`desync::is_desynced`) is private to `outl-actions`, and
//!   the only public per-page entry point is
//!   `recover_desynced_projection`, which calls
//!   `outl_md::reconcile::ensure_page_root_in_tree` — that emits
//!   `Op::SetProp` whenever a page's `page-slug` / `page-kind` disagrees
//!   with its filename. Running it over every page instead of the flagged
//!   ones would turn a read-only 129 ms scan into a potential
//!   2,923-page write. So the scan keeps its one lock.
//! - **`repair_namespaced_titles`** — not chunkable **at all**, and this
//!   is the one worth not re-litigating. It builds a mention index over
//!   *every block in the workspace* and then refuses any slug two
//!   spellings claim, rather than flipping a coin into the op log. A
//!   partial index is precisely how one spelling is seen when two exist,
//!   and the write it would then make is **permanent**: the pass skips
//!   any page that already has a title, so it can never correct itself.
//!   Chunking it would trade a 185 ms wait for a wrong title nobody can
//!   see is wrong. It keeps its one lock.
//!
//! ## What the two whole-workspace phases do instead
//!
//! They stay out of the launch window. The chunked orphan pass runs
//! immediately (pages it materialises become visible sooner), then the
//! sweep **settles** for [`SETTLE_BEFORE_WHOLE_WORKSPACE`] before taking
//! a long lock. Nothing about these repairs needs to happen in the first
//! seconds of a launch: both are idempotent, both have run on every
//! previous boot, and both will run again on the next one.
//!
//! Every lock here — chunked or whole — is taken through
//! `lock_when_free`, never `lock()`. `parking_lot` is only *eventually*
//! fair, so a blocking `lock()` from this thread can be handed the mutex
//! ahead of a command that started waiting earlier; `try_lock` means the
//! sweep only ever takes a mutex that is already free. It retries the
//! unit and never skips it — a `continue` on a busy lock steps over a
//! page for good, which is the silent-skip class of bug this whole pass
//! exists to close.
//!
//! ## Who was standing on "the sweep holds the lock for the whole sweep"
//!
//! Root `CLAUDE.md` invariant 10. Three consumers assumed it, and all
//! three survive the change:
//!
//! - **The desktop's `workspace-reconciled` event** fires once on
//!   completion, and `AppShell.tsx` re-reads the active page on it. It is
//!   still emitted once, at the end, and it still means "the tree moved";
//!   it arrives later now, which is what it already meant for the orphan
//!   pass.
//! - **`repair_namespaced_titles`' ambiguity verdict** depended on seeing
//!   the whole workspace at once. It still does — that is why it was not
//!   chunked.
//! - **The desync scan's output** was consumed under a lock it no longer
//!   shares with the repair. It never could: mobile already split the two,
//!   and `recover_desynced_projection` re-asks every question.
//!
//! The one thing that did *not* survive is the assumption that the sweep
//! finishes quickly. It is now a pass that can take minutes on a large
//! workspace, by design — see [`yield_to_user`].

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use outl_core::hlc::HlcGenerator;
use outl_core::workspace::Workspace;
use parking_lot::{Mutex, MutexGuard};
use tracing::{info, warn};

use crate::workspace_open::yield_to_user;

/// How long to wait for a busy workspace mutex before probing again.
///
/// Short enough that the sweep is not idling on a quiet device, long
/// enough that the probe itself is not the contention.
const LOCK_RETRY: Duration = Duration::from_millis(20);

/// How long the sweep waits after the chunked orphan pass before it takes
/// a lock it cannot release mid-phase.
///
/// The sweep is spawned the instant the workspace is published, which is
/// the instant the frontend issues its first `open_today_journal`. That
/// open is 1.8 ms of work, and the whole boot burst behind it (stats,
/// backlinks, themes, the shortcut catalog) is a few hundred ms of
/// millisecond-long locks. Two seconds clears it with room to spare,
/// without pretending to know when the first paint happened.
pub const SETTLE_BEFORE_WHOLE_WORKSPACE: Duration = Duration::from_secs(2);

/// The workspace was closed under the sweep (the user picked another
/// root), so the rest of the batch is about a workspace nobody has open.
struct WorkspaceClosed;

/// Take the workspace mutex, but only ever when it is already free.
///
/// `parking_lot::Mutex::lock` is FIFO-ish and only eventually fair, so a
/// blocking acquire from a background pass can be granted ahead of a
/// command that started waiting earlier — and on this pass's phases that
/// is a user watching a window do nothing. Probing with `try_lock`
/// inverts the priority: the sweep waits for the user, never the other
/// way round.
///
/// It **retries**, it does not give up. A unit of work skipped because
/// the lock happened to be busy is a page silently left unrepaired.
fn lock_when_free(slot: &Mutex<Option<Workspace>>) -> MutexGuard<'_, Option<Workspace>> {
    loop {
        if let Some(guard) = slot.try_lock() {
            return guard;
        }
        thread::sleep(LOCK_RETRY);
    }
}

/// Run `work` against the published workspace under a lock taken only
/// when free, release the lock, then yield for as long as the work took.
///
/// The yield is **outside** the lock on purpose — sleeping while holding
/// it is the same stall with extra steps.
fn locked_step<T>(
    slot: &Mutex<Option<Workspace>>,
    work: impl FnOnce(&mut Workspace) -> T,
) -> Result<T, WorkspaceClosed> {
    let (out, took) = {
        let mut guard = lock_when_free(slot);
        let Some(ws) = guard.as_mut() else {
            return Err(WorkspaceClosed);
        };
        let started = Instant::now();
        let out = work(ws);
        (out, started.elapsed())
    };
    yield_to_user(took);
    Ok(out)
}

/// Spawn [`reconcile_sweep`] on a worker thread, calling `on_changed`
/// once at the end if the pass moved the tree.
///
/// `on_changed` is the clients' one genuine divergence: the desktop emits
/// `workspace-reconciled` (which `AppShell.tsx` listens for), mobile has
/// no listener for it yet and passes a no-op. Same shape as
/// [`crate::ProjectionWriter`]'s `report_failure`.
pub fn spawn_background_reconcile<F>(
    workspace: Arc<Mutex<Option<Workspace>>>,
    storage_root: PathBuf,
    hlc: HlcGenerator,
    on_changed: F,
) -> thread::JoinHandle<()>
where
    F: FnOnce() + Send + 'static,
{
    thread::spawn(move || {
        if reconcile_sweep(
            &workspace,
            &storage_root,
            &hlc,
            SETTLE_BEFORE_WHOLE_WORKSPACE,
        ) {
            on_changed();
        }
    })
}

/// Reconcile what the op log does not reflect yet, and repair what two
/// devices or an importer left malformed.
///
/// Returns `true` when at least one phase moved the tree, which is what a
/// client surfaces so the open page re-reads itself.
///
/// `settle` is [`SETTLE_BEFORE_WHOLE_WORKSPACE`] in production and `0` in
/// a test: it is the gap between the chunked phases and the two that hold
/// one lock across the whole workspace (module docs). Taking it as a
/// parameter keeps the delay out of the test suite without making the
/// production value a second opinion.
pub fn reconcile_sweep(
    workspace: &Mutex<Option<Workspace>>,
    storage_root: &Path,
    hlc: &HlcGenerator,
    settle: Duration,
) -> bool {
    let engine = outl_actions::SyncEngine::new(storage_root.to_path_buf(), hlc.actor());
    let mut changed = false;

    if reconcile_orphans(workspace, &engine, hlc, &mut changed).is_err() {
        return changed;
    }

    // Everything past here takes a lock it cannot release mid-phase.
    thread::sleep(settle);

    if recover_desynced(workspace, &engine, hlc, storage_root, &mut changed).is_err() {
        return changed;
    }
    if repair_doubled_titles(workspace, hlc, &mut changed).is_err() {
        return changed;
    }
    let _ = repair_namespace_titles(workspace, hlc, &mut changed);
    changed
}

/// Reconcile every `.md` the op log does not reflect yet: pages authored
/// in vim, pulled from a peer without a sidecar, imported by `outl serve`
/// or dropped in by a Roam / Logseq export.
///
/// Per page, lock released between pages. The filesystem walk that
/// selects them needs no lock at all.
fn reconcile_orphans(
    workspace: &Mutex<Option<Workspace>>,
    engine: &outl_actions::SyncEngine,
    hlc: &HlcGenerator,
    changed: &mut bool,
) -> Result<(), WorkspaceClosed> {
    let orphans_log = engine.orphans_log();
    let orphans = engine.scan_for_orphans();
    if orphans.is_empty() {
        return Ok(());
    }
    info!(
        "background reconcile: {} orphan(s) to process",
        orphans.len()
    );
    let started = Instant::now();
    for path in &orphans {
        // `Some(..)`, never `None`: a block that fails to match drops to
        // matching level 3, which trashes it, and `outl-md`'s hard rule
        // is that such a block is recorded before it goes.
        let ok = locked_step(workspace, |ws| {
            outl_md::reconcile::reconcile_md(ws, hlc, path, Some(&orphans_log))
        })?;
        match ok {
            Ok(_) => *changed = true,
            Err(e) => warn!("orphan reconcile failed for {}: {e}", path.display()),
        }
    }
    info!(
        "background reconcile complete: {} page(s) in {:?}",
        orphans.len(),
        started.elapsed()
    );
    Ok(())
}

/// Recover pages whose projection ran ahead of the op log — the `.md` and
/// its sidecar carry blocks whose ids exist in no log, because a crash
/// landed between "projection written" and "ops appended".
///
/// The **scan** holds one lock over the whole workspace; the module docs
/// say why it cannot be split from here. The **repair** is per page, and
/// the scan's list is only a hint: `recover_desynced_projection` re-reads
/// the file, re-checks the hash and the sidecar lockstep, and asks the
/// tree about every id again under its own lock, so a page something else
/// fixed in between costs an `Ok(0)`.
fn recover_desynced(
    workspace: &Mutex<Option<Workspace>>,
    engine: &outl_actions::SyncEngine,
    hlc: &HlcGenerator,
    storage_root: &Path,
    changed: &mut bool,
) -> Result<(), WorkspaceClosed> {
    let desynced = locked_step(workspace, |ws| engine.scan_for_desynced_projections(ws))?;
    for path in &desynced {
        let recovered = locked_step(workspace, |ws| {
            outl_actions::recover_desynced_projection(ws, hlc, storage_root, path)
        })?;
        match recovered {
            Ok(0) => {}
            Ok(n) => {
                info!(
                    "recovered {n} lost op(s) from desynced projection {}",
                    path.display()
                );
                *changed = true;
            }
            Err(e) => warn!("desync recovery failed for {}: {e}", path.display()),
        }
    }
    Ok(())
}

/// Repair journal titles doubled by concurrent offline creation (two
/// devices auto-created the same day's journal; each wrote the slug as the
/// root's Yrs text and the two concurrent inserts concatenated).
///
/// **One lock, deliberately.** The pass is 1 ms on a 2,923-page
/// workspace, and splitting its two halves — collect the doubled roots,
/// then clear each — would let a root's text change between them, so the
/// clearing `Op::Edit` could land on a title that is no longer a doubled
/// slug. That is a repair acting on a stale read, bought for a saving
/// below the noise floor.
fn repair_doubled_titles(
    workspace: &Mutex<Option<Workspace>>,
    hlc: &HlcGenerator,
    changed: &mut bool,
) -> Result<(), WorkspaceClosed> {
    match locked_step(workspace, |ws| {
        outl_actions::repair_doubled_journal_titles(ws, hlc)
    })? {
        Ok(0) => {}
        Ok(n) => {
            info!("repaired {n} doubled journal title(s)");
            *changed = true;
        }
        Err(e) => warn!("doubled-title repair: {e}"),
    }
    Ok(())
}

/// Give an ingested page back the namespaced `title::` it never got
/// (issue #275): `outl_actions::namespace` reads a page's hierarchy off
/// its title, and a page that arrived as a `.md` has none, so the
/// nested-pages section rendered empty for every namespace on an imported
/// graph. The names are still in the mentions; this joins them back.
///
/// **One lock, and the module docs own the reason.** The ambiguity
/// verdict needs the complete mention index, and the write it makes
/// cannot be corrected later.
fn repair_namespace_titles(
    workspace: &Mutex<Option<Workspace>>,
    hlc: &HlcGenerator,
    changed: &mut bool,
) -> Result<(), WorkspaceClosed> {
    match locked_step(workspace, |ws| {
        outl_actions::repair_namespaced_titles(ws, hlc)
    })? {
        Ok(report) if report.is_clean() => {}
        Ok(report) => {
            if !report.repaired.is_empty() {
                info!("recovered {} namespaced title(s)", report.repaired.len());
                *changed = true;
            }
            // Reported, never guessed: two spellings of one slug would
            // make the choice a coin flip in the op log.
            for (slug, names) in &report.ambiguous {
                warn!("namespace title for `{slug}` is ambiguous: {names:?}");
            }
        }
        Err(e) => warn!("namespaced-title repair: {e}"),
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use outl_core::id::ActorId;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn published() -> (Arc<Mutex<Option<Workspace>>>, HlcGenerator) {
        let actor = ActorId::new();
        let ws = Workspace::open_in_memory(actor).expect("in-memory workspace");
        (Arc::new(Mutex::new(Some(ws))), HlcGenerator::new(actor))
    }

    /// A clean workspace reports nothing changed, so a client does not
    /// refresh the open page for a pass that did nothing.
    #[test]
    fn a_clean_workspace_reports_no_change() {
        let root = tempfile::tempdir().expect("tempdir");
        let (slot, hlc) = published();
        assert!(!reconcile_sweep(&slot, root.path(), &hlc, Duration::ZERO));
    }

    /// The whole point of the pass: a user-initiated command can take the
    /// workspace mutex **while the sweep is running**, because every phase
    /// releases it. A sweep that held one lock end to end would make this
    /// time out instead.
    #[test]
    fn a_command_interleaves_with_a_running_sweep() {
        let root = tempfile::tempdir().expect("tempdir");
        let (slot, hlc) = published();
        let opens = Arc::new(AtomicUsize::new(0));

        let sweeping = {
            let slot = Arc::clone(&slot);
            let hlc = hlc.clone();
            let root = root.path().to_path_buf();
            thread::spawn(move || reconcile_sweep(&slot, &root, &hlc, Duration::from_millis(50)))
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while !sweeping.is_finished() && Instant::now() < deadline {
            if let Some(mut guard) = slot.try_lock() {
                if let Some(ws) = guard.as_mut() {
                    let _ = ws.tree().node_count();
                    opens.fetch_add(1, Ordering::Relaxed);
                }
            }
            thread::sleep(Duration::from_millis(1));
        }
        sweeping.join().expect("sweep thread");
        assert!(
            opens.load(Ordering::Relaxed) > 0,
            "no command got the lock while the sweep ran"
        );
    }

    /// A workspace closed under the sweep (the user picked another root)
    /// aborts it instead of repairing a workspace nobody has open.
    #[test]
    fn a_closed_workspace_aborts_the_sweep() {
        let root = tempfile::tempdir().expect("tempdir");
        let actor = ActorId::new();
        let hlc = HlcGenerator::new(actor);
        let slot: Arc<Mutex<Option<Workspace>>> = Arc::new(Mutex::new(None));
        assert!(!reconcile_sweep(&slot, root.path(), &hlc, Duration::ZERO));
    }

    /// `on_changed` is only called for a pass that moved the tree.
    #[test]
    fn a_clean_sweep_does_not_notify() {
        let root = tempfile::tempdir().expect("tempdir");
        let (slot, hlc) = published();
        let notified = Arc::new(AtomicUsize::new(0));
        let seen = Arc::clone(&notified);
        spawn_background_reconcile(slot, root.path().to_path_buf(), hlc, move || {
            seen.fetch_add(1, Ordering::Relaxed);
        })
        .join()
        .expect("sweep thread");
        assert_eq!(notified.load(Ordering::Relaxed), 0);
    }
}
