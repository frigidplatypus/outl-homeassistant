//! `JsonlStorage::actor_timestamps` — the metadata-only read behind every
//! vector clock.
//!
//! Two questions, and the second is the one that regresses: does it agree
//! with the `all_ops` fold it replaced, and does it stay off the two things
//! that made that fold cost a full workspace load — the node index and the
//! ops themselves?
//!
//! The cheap path exists because `peer_status` used to answer "what is my
//! sync status" by opening the whole op log: both index sidecars for every
//! actor, then every `Op::Edit`'s `text_op` parsed back out of the `.jsonl`.
//! On a 222,536-op workspace that is 132 MB of I/O per call, and on iOS it
//! ran on the main thread until the scene-update watchdog killed the app.

use std::collections::{BTreeSet, HashMap};
use std::path::Path;

use outl_core::fractional::Fractional;
use outl_core::hlc::{Hlc, HlcGenerator};
use outl_core::id::{ActorId, NodeId};
use outl_core::op::{LogOp, Op};
use outl_core::storage::sidecar::{self, SidecarKind};
use outl_core::storage::{JsonlStorage, PageScope, Storage};

/// Write one actor's `.jsonl` by hand, so the directory starts with no
/// sidecars at all — the state a fresh install and a post-`outl compact`
/// workspace are both in.
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
    std::fs::write(ops_dir.join(format!("ops-{actor}.jsonl")), body).expect("write log");
    ops
}

/// The census the `all_ops` fold produces: every distinct HLC, grouped by
/// the producer inside it. Written out longhand rather than shared with the
/// implementation — a shared helper would let one bug satisfy both sides.
fn fold_census(ops_dir: &Path, actor: ActorId) -> HashMap<ActorId, BTreeSet<Hlc>> {
    let storage = JsonlStorage::open(ops_dir.to_path_buf(), actor).expect("open");
    let mut out: HashMap<ActorId, BTreeSet<Hlc>> = HashMap::new();
    for op in storage.all_ops().expect("all_ops") {
        out.entry(op.ts.actor).or_default().insert(op.ts);
    }
    out
}

fn sidecar_paths(ops_dir: &Path, actor: ActorId) -> (std::path::PathBuf, std::path::PathBuf) {
    (
        sidecar::path_for(ops_dir, actor, &PageScope::Global, SidecarKind::Offset),
        sidecar::path_for(ops_dir, actor, &PageScope::Global, SidecarKind::Node),
    )
}

/// The replaced read and the replacement must see the same op log —
/// sidecars or not, and whether the `.jsonl` grew since they were written.
#[test]
fn the_index_census_matches_an_all_ops_fold() {
    let tmp = tempfile::tempdir().expect("tmp");
    let ops_dir = tmp.path().join("ops");
    let (a, b, c) = (ActorId::new(), ActorId::new(), ActorId::new());
    write_log(&ops_dir, a, 12);
    write_log(&ops_dir, b, 5);
    write_log(&ops_dir, c, 1);

    // Cold: no sidecars anywhere, so the census falls back to the shared
    // parse-lite stream.
    let cold = JsonlStorage::actor_timestamps(&ops_dir).expect("census");

    // The fold also writes the sidecars out as a side effect, so the next
    // census takes the byte-exact-fresh arm of the ladder instead.
    let folded = fold_census(&ops_dir, a);
    let warm = JsonlStorage::actor_timestamps(&ops_dir).expect("census");

    // Grown: one more line than the sidecar accounts for — the state every
    // peer's file is in right after a sync ingest, which appends to the
    // `.jsonl` without touching the sidecars.
    let extra = write_log(&ops_dir, b, 7);
    let grown = JsonlStorage::actor_timestamps(&ops_dir).expect("census");

    assert_eq!(cold, folded, "cold census must match the all_ops fold");
    assert_eq!(warm, folded, "a fresh sidecar must not change the answer");
    assert_eq!(
        grown,
        fold_census(&ops_dir, a),
        "a grown .jsonl must be re-counted, not reported at the sidecar's extent"
    );
    assert_eq!(grown[&b].len(), extra.len());
    assert_eq!(grown.len(), 3, "every actor with a file is counted");
}

/// The node index is the expensive half of `reload` — a
/// `HashMap<NodeId, Vec<_>>` filled one `entry()` per op, and the innermost
/// frame of the iOS watchdog kill. The census has no use for it and must
/// neither read nor rebuild it.
///
/// Deleting the sidecars and asserting they stay deleted is what makes this
/// fail if the census is ever rerouted back through `JsonlStorage::open`:
/// that path rebuilds both indexes from the `.jsonl` and persists them.
#[test]
fn the_actor_census_neither_reads_nor_writes_the_node_index() {
    let tmp = tempfile::tempdir().expect("tmp");
    let ops_dir = tmp.path().join("ops");
    let actor = ActorId::new();
    let ops = write_log(&ops_dir, actor, 9);

    // Materialize both sidecars, then take the node one away.
    let expected = fold_census(&ops_dir, actor);
    let (offset_idx, node_idx) = sidecar_paths(&ops_dir, actor);
    assert!(node_idx.exists(), "the fold should have written it");
    std::fs::remove_file(&node_idx).expect("remove node sidecar");

    assert_eq!(JsonlStorage::actor_timestamps(&ops_dir).unwrap(), expected);
    assert!(
        !node_idx.exists(),
        "the census rebuilt the node index — it was rerouted through a full open"
    );

    // Same with no sidecar at all to lean on: the fallback streams the
    // `.jsonl` and still writes nothing.
    std::fs::remove_file(&offset_idx).expect("remove offset sidecar");
    let census = JsonlStorage::actor_timestamps(&ops_dir).expect("census");
    assert_eq!(census[&actor].len(), ops.len());
    assert!(
        !offset_idx.exists() && !node_idx.exists(),
        "a status read must not persist an index; advancing the offset sidecar \
         past the node sidecar makes the next real boot rebuild both"
    );
}

/// A workspace that has never synced has no `ops/` directory. Answering
/// "nothing" is right; creating the directory is `open`'s job.
#[test]
fn a_missing_ops_dir_is_an_empty_census_not_a_new_directory() {
    let tmp = tempfile::tempdir().expect("tmp");
    let ops_dir = tmp.path().join("ops");
    assert!(JsonlStorage::actor_timestamps(&ops_dir).unwrap().is_empty());
    assert!(!ops_dir.exists(), "a read must not create the ops dir");
}

/// A file-sync conflict copy matches `ops-*.jsonl` but carries no valid
/// actor id, so there is no sidecar name to derive. `reload_global` skips
/// it; the census has to skip the same file or the two disagree about what
/// the workspace holds.
#[test]
fn a_conflict_copy_is_skipped_the_way_reload_skips_it() {
    let tmp = tempfile::tempdir().expect("tmp");
    let ops_dir = tmp.path().join("ops");
    let (kept, stray) = (ActorId::new(), ActorId::new());
    write_log(&ops_dir, kept, 4);

    // The copy holds a DIFFERENT actor's ops, so reading it would show up as
    // an extra actor in the census rather than dedup away against the ops we
    // already counted.
    write_log(&ops_dir, stray, 3);
    std::fs::rename(
        ops_dir.join(format!("ops-{stray}.jsonl")),
        ops_dir.join(format!("ops-{stray} 2.jsonl")),
    )
    .expect("conflict copy");

    let census = JsonlStorage::actor_timestamps(&ops_dir).expect("census");
    assert_eq!(census, fold_census(&ops_dir, kept));
    assert_eq!(census[&kept].len(), 4);
    assert!(
        !census.contains_key(&stray),
        "a conflict copy has no derivable sidecar name; reload skips it, so must the census"
    );
}
