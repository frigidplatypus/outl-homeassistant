//! The textbook tree-CRDT conflict: two replicas concurrently move
//! nodes such that combining naively would form a cycle.
//!
//! Expected behavior:
//!   - Both replicas converge to the same final tree.
//!   - The losing move (by HLC order) is a no-op on the materialized
//!     tree but still recorded in the log.
//!   - Total ops applied == number of unique LogOps.

mod common;

use common::{assert_trees_equal, create_op, move_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};

#[test]
fn ab_cycle_converges_with_op_preserved() {
    let actor1 = ActorId::new();
    let actor2 = ActorId::new();
    let root = NodeId::root();
    let a = NodeId::new();
    let b = NodeId::new();
    let x = NodeId::new();
    let y = NodeId::new();

    // Setup (applied at the same HLCs on both replicas).
    let setup = [
        op_at(actor1, 1, 0, create_op(x, root, pos("a"))),
        op_at(actor1, 2, 0, create_op(y, root, pos("b"))),
        op_at(actor1, 3, 0, create_op(a, x, pos("a"))),
        op_at(actor1, 4, 0, create_op(b, y, pos("a"))),
    ];

    // Concurrent moves (different actors, different HLCs):
    //   actor1: Move(a, b)  (ts=10)
    //   actor2: Move(b, a)  (ts=11)
    let move_ab = op_at(actor1, 10, 0, move_op(a, b, pos("m")));
    let move_ba = op_at(actor2, 11, 0, move_op(b, a, pos("m")));

    // Replica A: applies setup, then Move(a,b), then Move(b,a).
    let mut r1 = Replica::new(actor1);
    for op in setup.iter() {
        r1.apply(op.clone());
    }
    r1.apply(move_ab.clone());
    r1.apply(move_ba.clone());

    // Replica B: applies setup, then Move(b,a), then Move(a,b) (force reorder).
    let mut r2 = Replica::new(actor2);
    for op in setup.iter() {
        r2.apply(op.clone());
    }
    r2.apply(move_ba.clone());
    r2.apply(move_ab.clone());

    // Both replicas must agree.
    assert_trees_equal(&r1.tree, &r2.tree);

    // Both replicas must have the full log (no silent loss).
    assert_eq!(r1.log.len(), setup.len() + 2);
    assert_eq!(r2.log.len(), setup.len() + 2);

    // Exactly one of the two moves is materialized; the other is a no-op.
    let a_parent = r1.tree.parent(a).expect("a is in tree");
    let b_parent = r1.tree.parent(b).expect("b is in tree");
    let move_ab_applied = a_parent == b;
    let move_ba_applied = b_parent == a;
    assert!(
        move_ab_applied ^ move_ba_applied,
        "exactly one of the moves must be applied, got a_parent={a_parent}, b_parent={b_parent}"
    );
}

#[test]
fn self_parent_is_always_cycle() {
    let actor = ActorId::new();
    let root = NodeId::root();
    let n = NodeId::new();
    let mut r = Replica::new(actor);
    r.apply(op_at(actor, 1, 0, create_op(n, root, pos("a"))));
    // Move n under n — must be no-op.
    r.apply(op_at(actor, 2, 0, move_op(n, n, pos("b"))));
    assert_eq!(r.tree.parent(n), Some(root));
    // Op still in log.
    assert_eq!(r.log.len(), 2);
}
