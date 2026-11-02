//! Applying the same op N times must produce the same state and the
//! same log as applying it once.

mod common;

use common::{create_op, move_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};

#[test]
fn create_idempotent() {
    let actor = ActorId::new();
    let n = NodeId::new();
    let op = op_at(actor, 1, 0, create_op(n, NodeId::root(), pos("a")));

    let mut r = Replica::new(actor);
    for _ in 0..10 {
        r.apply(op.clone());
    }

    assert_eq!(r.log.len(), 1);
    assert_eq!(r.tree.node_count(), 1);
    assert_eq!(r.tree.parent(n), Some(NodeId::root()));
}

#[test]
fn move_idempotent() {
    let actor = ActorId::new();
    let n = NodeId::new();
    let p = NodeId::new();

    let mut r = Replica::new(actor);
    r.apply(op_at(actor, 1, 0, create_op(n, NodeId::root(), pos("a"))));
    r.apply(op_at(actor, 2, 0, create_op(p, NodeId::root(), pos("b"))));

    let mv = op_at(actor, 3, 0, move_op(n, p, pos("m")));
    for _ in 0..10 {
        r.apply(mv.clone());
    }

    assert_eq!(r.log.len(), 3);
    assert_eq!(r.tree.parent(n), Some(p));
}

#[test]
fn mixed_ops_idempotent() {
    let actor = ActorId::new();
    let root = NodeId::root();
    let n1 = NodeId::new();
    let n2 = NodeId::new();

    let ops = [
        op_at(actor, 1, 0, create_op(n1, root, pos("a"))),
        op_at(actor, 2, 0, create_op(n2, root, pos("b"))),
        op_at(actor, 3, 0, move_op(n2, n1, pos("m"))),
    ];

    let mut r = Replica::new(actor);
    // Apply the full sequence 5 times in a row.
    for _ in 0..5 {
        for op in &ops {
            r.apply(op.clone());
        }
    }

    assert_eq!(r.log.len(), ops.len());
    assert_eq!(r.tree.parent(n1), Some(root));
    assert_eq!(r.tree.parent(n2), Some(n1));
}
