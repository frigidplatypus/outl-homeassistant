//! What this device holds, and what a peer is therefore missing.
//!
//! The read half of the wire's op-log access. [`oplog`](crate::oplog) owns
//! the write half — the append serialization and the fsync-before-ack rule —
//! and those are durability invariants about bytes landing. This module owns
//! a different question: *which ops exist here*, answered without reading one.
//!
//! That distinction is the whole reason the module exists. A vector clock is
//! a max HLC and a distinct-op count per actor, and the op log's offset index
//! is keyed by HLC — so its keys **are** the answer. Deriving it from
//! `JsonlStorage::open` + `all_ops` instead cost a full workspace load per
//! question: both index sidecars for every actor, then every `Op::Edit`'s
//! `text_op` parsed back into a `Vec<u8>`. On the reference 222,536-op
//! workspace that is 52 MB of sidecar plus 80 MB of `.jsonl`, measured at
//! 342 ms per call in release and 3.7 s in debug.
//!
//! Three callers asked it on a loop. `peer_status` polls every 5s from both
//! GUI clients — on iOS from the IPC thread, which is the UI main thread, so
//! the scene-update watchdog killed the app before it could paint. The
//! catch-up loop asks it per peer every 8s. And the ingest asked it per
//! received batch *while holding the device-wide append flock*, so every
//! other sync write on the machine queued behind a workspace load.
//!
//! [`actor_census`] is the single owner of the answer, and it delegates to
//! `outl_core::storage::JsonlStorage::actor_timestamps`, which is the single
//! owner of reading it off disk.

use std::collections::{BTreeSet, HashMap, HashSet};

use anyhow::{Context, Result};
use outl_core::hlc::Hlc;
use outl_core::id::ActorId;
use outl_core::storage::{JsonlStorage, Storage};
use outl_core::LogOp;
use tracing::info;

use crate::protocol::ActorClock;

/// Per-actor census of the local op log: the set of DISTINCT HLCs held for
/// each actor. Distinct because historic unsynchronized concurrent pulls left
/// duplicated append lines on real disks — a raw line count would inflate the
/// `count` and mask a genuine gap on the advertising side. The reference
/// workspace carries 148 such duplicates among 222,536 lines.
///
/// Read from the op log's offset index, never from the ops. See this module's
/// header for what that replaced and why.
pub(crate) fn actor_census(ops_dir: &std::path::Path) -> Result<HashMap<ActorId, BTreeSet<Hlc>>> {
    JsonlStorage::actor_timestamps(ops_dir).context("read the actor census from the op-log index")
}

/// Compute our vector clock (per-actor max HLC + distinct-op count) from the
/// local op log. Max + count both fall out of the same census pass.
pub(crate) fn local_vector_clock(
    ops_dir: &std::path::Path,
) -> Result<HashMap<ActorId, ActorClock>> {
    Ok(actor_census(ops_dir)?
        .into_iter()
        .filter_map(|(actor_id, hlcs)| {
            let max = *hlcs.last()?;
            Some((
                actor_id,
                ActorClock {
                    max,
                    count: hlcs.len() as u64,
                },
            ))
        })
        .collect())
}

/// Collect the ops the peer is missing, given the peer's vector clock.
///
/// Per actor in OUR log:
/// - peer has no entry → send everything we hold for that actor;
/// - peer has `(max_r, count_r)` → let `below` = how many DISTINCT ops of
///   that actor we hold with `ts <= max_r`. If `below > count_r` the peer
///   has a GAP below its own watermark (an op landed ahead of a pending
///   backlog, so the bare max lies about what's underneath) → send the
///   actor's FULL log; the receiver's ingest dedup absorbs the overlap.
///   Otherwise → fast path, send only `ts > max_r`.
///
/// Theoretical limitation (accepted): two replicas holding EQUAL counts of
/// DIFFERENT subsets below the same max are indistinguishable to this check,
/// so the fast path under-sends between them. Convergence is still
/// guaranteed via the origin: each op's authoring device holds its own
/// actor's complete log, so any replica with a partial subset trips the
/// `below > count` check the next time it syncs (directly or transitively)
/// with the origin's full prefix.
pub(crate) fn ops_missing_for(
    ops_dir: &std::path::Path,
    actor: ActorId,
    peer_clock: &HashMap<ActorId, ActorClock>,
) -> Result<Vec<LogOp>> {
    let census = actor_census(ops_dir)?;

    // Actors whose FULL log must cross (unknown to the peer, or gap detected
    // below the peer's watermark).
    let full_resend: HashSet<ActorId> = census
        .iter()
        .filter_map(|(actor_id, hlcs)| match peer_clock.get(actor_id) {
            None => Some(*actor_id),
            Some(peer) => {
                let below = hlcs.range(..=peer.max).count() as u64;
                (below > peer.count).then(|| {
                    info!(
                        actor = %actor_id,
                        below,
                        peer_count = peer.count,
                        "gap below peer watermark detected; resending full actor log"
                    );
                    *actor_id
                })
            }
        })
        .collect();

    // Nothing to send? Then read nothing. The census already answers the
    // question per actor — its sets are ordered, so "do we hold anything
    // above the peer's max" is one comparison against the last element — and
    // "nothing" is the answer on a converged workspace, which is the state
    // the catch-up loop asks about every 8s per peer and `sync_now` asks
    // about on every pull-to-refresh. Reading the whole log into a
    // `Vec<LogOp>` and filtering it down to empty is the standing cost of a
    // quiet device.
    let sends_anything = census.iter().any(|(actor_id, hlcs)| match hlcs.last() {
        None => false,
        Some(max) => {
            full_resend.contains(actor_id)
                || peer_clock.get(actor_id).is_none_or(|peer| *max > peer.max)
        }
    });
    if !sends_anything {
        return Ok(Vec::new());
    }

    let storage =
        JsonlStorage::open(ops_dir.to_path_buf(), actor).context("open storage for delta")?;
    let all_ops = storage.all_ops().context("load all ops")?;
    Ok(all_ops
        .into_iter()
        .filter(|op| {
            // `op.ts.actor`, not `op.actor`: the same value (`LogOp::actor`
            // is documented as "also embedded inside `ts`"), but the census
            // above can only see the one inside the HLC, and the early-out
            // is only sound if both sides key on the same field.
            if full_resend.contains(&op.ts.actor) {
                return true;
            }
            match peer_clock.get(&op.ts.actor) {
                None => true,
                Some(peer) => op.ts > peer.max,
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use outl_core::fractional::Fractional;
    use outl_core::hlc::HlcGenerator;
    use outl_core::id::NodeId;
    use outl_core::op::Op;
    use outl_core::storage::sidecar::{self, SidecarKind};
    use outl_core::storage::PageScope;
    use std::path::{Path, PathBuf};

    /// Write one actor's `ops-<actor>.jsonl` by hand, returning its ops.
    fn write_log(ops_dir: &Path, actor: ActorId, count: usize) -> Vec<LogOp> {
        let gen = HlcGenerator::new(actor);
        let ops: Vec<LogOp> = (0..count)
            .map(|_| {
                let ts = gen.next();
                LogOp {
                    ts,
                    actor,
                    op: Op::Create {
                        node: NodeId::new(),
                        parent: NodeId::root(),
                        position: Fractional::first(),
                    },
                }
            })
            .collect();
        let body: String = ops
            .iter()
            .map(|op| format!("{}\n", serde_json::to_string(op).expect("encode")))
            .collect();
        std::fs::create_dir_all(ops_dir).expect("ops dir");
        std::fs::write(ops_dir.join(format!("ops-{actor}.jsonl")), body).expect("write");
        ops
    }

    /// The two index sidecars for `actor`. Their absence is what the tests
    /// below watch: a path that goes through `JsonlStorage::open` rebuilds
    /// and persists both, so "still missing" means "no full open happened".
    fn sidecars(ops_dir: &Path, actor: ActorId) -> [PathBuf; 2] {
        [SidecarKind::Offset, SidecarKind::Node]
            .map(|kind| sidecar::path_for(ops_dir, actor, &PageScope::Global, kind))
    }

    /// The clock is a max and a count per actor, and both come off the
    /// offset index. Rerouting it back through `JsonlStorage::open` —
    /// `reload` loading the node sidecar for every actor, then `all_ops`
    /// parsing every op body — is what put a full workspace load behind one
    /// `peer_status` call and got the iOS app killed by the watchdog.
    #[test]
    fn the_vector_clock_is_read_from_the_index_not_from_the_ops() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ops_dir = tmp.path().join("ops");
        let (a, b) = (ActorId::new(), ActorId::new());
        let ops_a = write_log(&ops_dir, a, 11);
        write_log(&ops_dir, b, 4);

        let clock = local_vector_clock(&ops_dir).expect("clock");
        assert_eq!(clock[&a].count, 11);
        assert_eq!(clock[&a].max, ops_a.last().expect("op").ts);
        assert_eq!(clock[&b].count, 4);

        for path in sidecars(&ops_dir, a).iter().chain(&sidecars(&ops_dir, b)) {
            assert!(
                !path.exists(),
                "{} was built to answer a vector clock; the clock needs neither \
                 the node index nor an op body",
                path.display()
            );
        }
    }

    /// The catch-up loop asks "what does this peer still need" every 8s per
    /// peer, and on a converged workspace the answer is always "nothing".
    /// Reading the whole log to filter it down to empty is the standing cost
    /// of a quiet device, so the census answers first.
    #[test]
    fn a_peer_that_is_current_costs_no_op_read() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ops_dir = tmp.path().join("ops");
        let actor = ActorId::new();
        write_log(&ops_dir, actor, 6);

        let theirs = local_vector_clock(&ops_dir).expect("clock");
        assert!(ops_missing_for(&ops_dir, actor, &theirs)
            .expect("delta")
            .is_empty());
        for path in sidecars(&ops_dir, actor) {
            assert!(
                !path.exists(),
                "{} means the ops were read; a peer holding our clock needs nothing",
                path.display()
            );
        }
    }

    /// The allow case, and it is the load-bearing one: an early-out that
    /// refuses everything passes every "sent nothing" assertion ever
    /// written.
    #[test]
    fn a_peer_behind_us_still_gets_every_op_it_lacks() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ops_dir = tmp.path().join("ops");
        let (a, b) = (ActorId::new(), ActorId::new());
        let ops_a = write_log(&ops_dir, a, 6);
        let ops_b = write_log(&ops_dir, b, 3);

        // The peer holds the first two of A's ops and has never heard of B.
        let peer_clock: HashMap<ActorId, ActorClock> = [(
            a,
            ActorClock {
                max: ops_a[1].ts,
                count: 2,
            },
        )]
        .into_iter()
        .collect();

        let sent = ops_missing_for(&ops_dir, a, &peer_clock).expect("delta");
        let sent_ts: BTreeSet<Hlc> = sent.iter().map(|op| op.ts).collect();
        let expected: BTreeSet<Hlc> = ops_a[2..].iter().chain(&ops_b).map(|op| op.ts).collect();
        assert_eq!(sent_ts, expected);
    }

    /// A peer whose count is short of what we hold below its own watermark
    /// has a gap underneath it, so its actor's FULL log crosses — and the
    /// early-out must not swallow that, since nothing sits above the max.
    #[test]
    fn a_gap_below_the_peers_watermark_still_resends_the_full_log() {
        let tmp = tempfile::tempdir().expect("tmp");
        let ops_dir = tmp.path().join("ops");
        let actor = ActorId::new();
        let ops = write_log(&ops_dir, actor, 5);

        let peer_clock: HashMap<ActorId, ActorClock> = [(
            actor,
            ActorClock {
                max: ops.last().expect("op").ts,
                count: 2,
            },
        )]
        .into_iter()
        .collect();

        let sent = ops_missing_for(&ops_dir, actor, &peer_clock).expect("delta");
        assert_eq!(sent.len(), ops.len(), "the whole actor log must cross");
    }
}
