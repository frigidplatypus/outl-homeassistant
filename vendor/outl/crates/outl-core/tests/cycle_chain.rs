//! Transitive cycle detection.
//!
//! `creates_cycle` must follow the parent chain, not just compare against
//! the immediate parent. This test builds A→B→C→D and then concurrently
//! moves D under A — which would close a 4-node loop — and asserts that
//! the algorithm handles it the same way it handles the 2-node case.

mod common;

use common::{assert_trees_equal, create_op, move_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};

#[test]
fn deep_chain_cycle_is_detected() {
    let actor = ActorId::new();
    let root = NodeId::root();
    let a = NodeId::new();
    let b = NodeId::new();
    let c = NodeId::new();
    let d = NodeId::new();

    let mut r = Replica::new(actor);

    // Build A → B → C → D
    r.apply(op_at(actor, 1, 0, create_op(a, root, pos("a"))));
    r.apply(op_at(actor, 2, 0, create_op(b, a, pos("a"))));
    r.apply(op_at(actor, 3, 0, create_op(c, b, pos("a"))));
    r.apply(op_at(actor, 4, 0, create_op(d, c, pos("a"))));

    // Move A under D — this closes the loop. Must be no-op on tree.
    r.apply(op_at(actor, 5, 0, move_op(a, d, pos("m"))));

    // Tree unchanged: A still under root.
    assert_eq!(r.tree.parent(a), Some(root));
    assert_eq!(r.tree.parent(b), Some(a));
    assert_eq!(r.tree.parent(c), Some(b));
    assert_eq!(r.tree.parent(d), Some(c));

    // Op still in log.
    assert_eq!(r.log.len(), 5);
}

#[test]
fn cycle_with_late_arrival_converges() {
    let actor1 = ActorId::new();
    let actor2 = ActorId::new();
    let root = NodeId::root();
    let a = NodeId::new();
    let b = NodeId::new();
    let c = NodeId::new();

    // Setup: A→B→C (sequential).
    let setup = [
        op_at(actor1, 1, 0, create_op(a, root, pos("a"))),
        op_at(actor1, 2, 0, create_op(b, a, pos("a"))),
        op_at(actor1, 3, 0, create_op(c, b, pos("a"))),
    ];

    // Concurrent: actor1 moves c under a (legal), actor2 moves a under c (cycle).
    let move_c_under_a = op_at(actor1, 10, 0, move_op(c, a, pos("c")));
    let move_a_under_c = op_at(actor2, 11, 0, move_op(a, c, pos("c")));

    // Replica 1: setup, then move_c_under_a, then move_a_under_c.
    let mut r1 = Replica::new(actor1);
    for op in &setup {
        r1.apply(op.clone());
    }
    r1.apply(move_c_under_a.clone());
    r1.apply(move_a_under_c.clone());

    // Replica 2: setup, then move_a_under_c first (forces reorder later).
    let mut r2 = Replica::new(actor2);
    for op in &setup {
        r2.apply(op.clone());
    }
    r2.apply(move_a_under_c.clone());
    r2.apply(move_c_under_a.clone());

    assert_trees_equal(&r1.tree, &r2.tree);
    assert_eq!(r1.log.len(), 5);
    assert_eq!(r2.log.len(), 5);
}
