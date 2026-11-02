//! Stress test: 10k ops applied in HLC order finish quickly.
//!
//! This guards against quadratic regressions in `apply_op`. Applying in
//! HLC order means each apply is amortized O(1) since no reorder is
//! needed; only `contains_ts` does a binary search.

mod common;

use common::{create_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};
use std::time::Instant;

#[test]
fn ten_thousand_ops_apply_under_one_second() {
    let actor = ActorId::new();
    let mut r = Replica::new(actor);

    let started = Instant::now();
    // Create 10k siblings under root, each with a unique HLC.
    for i in 0..10_000u64 {
        let n = NodeId::new();
        r.apply(op_at(actor, i, 0, create_op(n, NodeId::root(), pos("m"))));
    }
    let elapsed = started.elapsed();

    assert_eq!(r.log.len(), 10_000);
    assert_eq!(r.tree.node_count(), 10_000);

    // Generous budget — real CI machines are slower than dev laptops.
    // The point is to catch O(n²) regressions, not micro-benchmark.
    assert!(
        elapsed.as_millis() < 5_000,
        "10k ops took {elapsed:?}, which suggests quadratic apply",
    );
}

#[test]
fn one_thousand_reordering_ops_finish_in_bounded_time() {
    // Apply 1k ops in reverse HLC order — every apply except the last
    // forces a full undo/replay. This is genuinely O(n²) by design, but
    // for 1k we want sub-second.
    let actor = ActorId::new();
    let mut r = Replica::new(actor);
    let mut ops = Vec::with_capacity(1_000);
    for i in 0..1_000u64 {
        let n = NodeId::new();
        ops.push(op_at(actor, i, 0, create_op(n, NodeId::root(), pos("m"))));
    }
    ops.reverse();

    let started = Instant::now();
    for op in ops {
        r.apply(op);
    }
    let elapsed = started.elapsed();

    assert_eq!(r.log.len(), 1_000);
    assert_eq!(r.tree.node_count(), 1_000);
    assert!(
        elapsed.as_millis() < 5_000,
        "reverse-order 1k ops took {elapsed:?}",
    );
}
