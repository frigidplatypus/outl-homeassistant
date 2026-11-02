//! An op arriving with an HLC older than the log's tail must force the
//! algorithm to undo newer ops, apply the late one, then replay.
//!
//! This is the mechanism that makes `apply_op` correct under arbitrary
//! delivery order.

mod common;

use common::{create_op, move_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};

#[test]
fn late_op_repositions_in_log() {
    let actor = ActorId::new();
    let root = NodeId::root();
    let n = NodeId::new();

    let mut r = Replica::new(actor);

    // Apply newer ops first.
    r.apply(op_at(actor, 5, 0, create_op(n, root, pos("a"))));
    r.apply(op_at(actor, 6, 0, move_op(n, root, pos("k"))));

    // Then a strictly older op.
    r.apply(op_at(actor, 3, 0, move_op(n, root, pos("c"))));

    // Final state: the newest op wins (it ran after the late one was
    // replayed). Position should be "k".
    assert_eq!(r.tree.position(n).map(|p| p.as_str()), Some("k"));

    // Log is in HLC order.
    let physicals: Vec<u64> = r.log.iter().map(|o| o.ts.physical_ms).collect();
    assert_eq!(physicals, vec![3, 5, 6]);
}

#[test]
fn many_late_ops_in_random_order_converge_to_same_state() {
    let actor = ActorId::new();
    let root = NodeId::root();
    let n = NodeId::new();

    // Build canonical ordering.
    let ops = vec![
        op_at(actor, 1, 0, create_op(n, root, pos("a"))),
        op_at(actor, 2, 0, move_op(n, root, pos("b"))),
        op_at(actor, 3, 0, move_op(n, root, pos("c"))),
        op_at(actor, 4, 0, move_op(n, root, pos("d"))),
        op_at(actor, 5, 0, move_op(n, root, pos("e"))),
    ];

    // Apply in order.
    let mut r_ordered = Replica::new(actor);
    for op in &ops {
        r_ordered.apply(op.clone());
    }

    // Apply in reverse order — every apply except the last forces reorder.
    let mut r_reversed = Replica::new(actor);
    for op in ops.iter().rev() {
        r_reversed.apply(op.clone());
    }

    common::assert_trees_equal(&r_ordered.tree, &r_reversed.tree);
    assert_eq!(r_ordered.tree.position(n).map(|p| p.as_str()), Some("e"));
    assert_eq!(r_reversed.tree.position(n).map(|p| p.as_str()), Some("e"));
}
