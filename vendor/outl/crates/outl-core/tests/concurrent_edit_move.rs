//! Concurrent edit and move of the same block.
//!
//! - Replica 1 emits `Op::Move(b, X)` at ts=10.
//! - Replica 2 emits `Op::Edit(b, ...)` at ts=11.
//! - Both replicas should end up with `b` under X and the edit recorded.
//!
//! Tree-level `Edit` is a no-op (block text lives in Yrs), but the op
//! must still be in the log so a higher layer (Workspace) can hand it to
//! the right `Yrs::Doc` when materializing block content.

mod common;

use common::{assert_trees_equal, create_op, move_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};
use outl_core::op::Op;

#[test]
fn move_and_edit_both_recorded() {
    let actor1 = ActorId::new();
    let actor2 = ActorId::new();
    let root = NodeId::root();
    let b = NodeId::new();
    let target = NodeId::new();

    // Setup
    let setup = [
        op_at(actor1, 1, 0, create_op(b, root, pos("a"))),
        op_at(actor1, 2, 0, create_op(target, root, pos("b"))),
    ];

    let move_b = op_at(actor1, 10, 0, move_op(b, target, pos("m")));
    let edit_b = op_at(
        actor2,
        11,
        0,
        Op::Edit {
            node: b,
            text_op: vec![1, 2, 3, 4],
        },
    );

    let mut r1 = Replica::new(actor1);
    let mut r2 = Replica::new(actor2);

    for op in &setup {
        r1.apply(op.clone());
        r2.apply(op.clone());
    }

    // r1: move then edit
    r1.apply(move_b.clone());
    r1.apply(edit_b.clone());

    // r2: edit then move (reverse order)
    r2.apply(edit_b.clone());
    r2.apply(move_b.clone());

    // Tree structure agrees: b under target.
    assert_eq!(r1.tree.parent(b), Some(target));
    assert_eq!(r2.tree.parent(b), Some(target));
    assert_trees_equal(&r1.tree, &r2.tree);

    // Both logs contain both ops.
    assert_eq!(r1.log.len(), setup.len() + 2);
    assert_eq!(r2.log.len(), setup.len() + 2);
}

#[test]
fn edit_op_for_unknown_block_is_recorded() {
    // Edge: an Edit may arrive before the Create for its target node
    // (e.g. via sync). Tree-level apply must still record the op so a
    // later Create + materialization can pick it up.
    let actor = ActorId::new();
    let mut r = Replica::new(actor);
    let phantom = NodeId::new();
    r.apply(op_at(
        actor,
        1,
        0,
        Op::Edit {
            node: phantom,
            text_op: vec![0xff],
        },
    ));
    assert_eq!(r.log.len(), 1);
    assert_eq!(r.tree.node_count(), 0);
}
