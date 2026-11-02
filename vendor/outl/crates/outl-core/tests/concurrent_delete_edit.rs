//! Concurrent delete (Move to TRASH_ROOT) and edit of the same block.
//!
//! Convergence: the block ends up under TRASH_ROOT (one of the two moves
//! by HLC tiebreak) AND the edit stays in the log. The user-visible block
//! is gone (in trash) but a future restore — Move out of TRASH_ROOT —
//! would still see the edited content.

mod common;

use common::{assert_trees_equal, create_op, move_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};
use outl_core::op::Op;

#[test]
fn delete_wins_edit_preserved() {
    let actor1 = ActorId::new();
    let actor2 = ActorId::new();
    let root = NodeId::root();
    let b = NodeId::new();
    let trash = NodeId::trash();

    let setup = op_at(actor1, 1, 0, create_op(b, root, pos("a")));

    // Concurrent ops.
    let delete = op_at(actor1, 10, 0, move_op(b, trash, pos("a")));
    let edit = op_at(
        actor2,
        11,
        0,
        Op::Edit {
            node: b,
            text_op: vec![9, 8, 7],
        },
    );

    let mut r1 = Replica::new(actor1);
    let mut r2 = Replica::new(actor2);
    r1.apply(setup.clone());
    r2.apply(setup);

    r1.apply(delete.clone());
    r1.apply(edit.clone());

    r2.apply(edit.clone());
    r2.apply(delete.clone());

    // Both replicas agree the block is in trash.
    assert_eq!(r1.tree.parent(b), Some(trash));
    assert_eq!(r2.tree.parent(b), Some(trash));
    assert_trees_equal(&r1.tree, &r2.tree);

    // Edit op still in log on both sides.
    assert!(r1.log.iter().any(|o| matches!(o.op, Op::Edit { .. })));
    assert!(r2.log.iter().any(|o| matches!(o.op, Op::Edit { .. })));
    assert_eq!(r1.log.len(), 3);
    assert_eq!(r2.log.len(), 3);
}
