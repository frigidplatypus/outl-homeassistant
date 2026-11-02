//! Property-based assertion of strong eventual consistency.
//!
//! Generates pseudo-random op sequences and applies them to two replicas
//! in two different orders, asserting the final trees agree.
//!
//! Uses `proptest`. Default cases = 200; bump via `PROPTEST_CASES` env
//! when you want more confidence (1000+ for release gating).

mod common;

use common::{assert_trees_equal, create_op, move_op, op_at, pos, Replica};
use outl_core::id::{ActorId, NodeId};
use outl_core::op::LogOp;
use proptest::prelude::*;

/// Build the same `LogOp` set on two fresh replicas applied in two
/// distinct orders. The orders are `as_given` and `reversed`.
fn assert_converges(ops: Vec<LogOp>) {
    let actor = ActorId::new();
    let mut r_in_order = Replica::new(actor);
    for op in &ops {
        r_in_order.apply(op.clone());
    }
    let mut r_reversed = Replica::new(actor);
    for op in ops.iter().rev() {
        r_reversed.apply(op.clone());
    }
    assert_trees_equal(&r_in_order.tree, &r_reversed.tree);
    assert_eq!(r_in_order.log.len(), r_reversed.log.len());
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(200))]

    /// Sequence of Creates and Moves over a small fixed node pool.
    #[test]
    fn random_creates_and_moves_converge(seq in proptest::collection::vec((0..5usize, 0..5usize, 1u64..1000), 1..30)) {
        let actor = ActorId::new();
        let nodes: Vec<NodeId> = (0..5).map(|_| NodeId::new()).collect();
        let root = NodeId::root();

        let mut ops = Vec::new();
        let mut created: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
        for (i, (a, b, ts)) in seq.iter().enumerate() {
            let node = nodes[*a];
            let parent_node = if *b == *a { root } else { nodes[*b] };
            // Physical = ts * 1000 + i guarantees uniqueness across (ts, i)
            // pairs while preserving ts as the primary ordering dimension.
            // Without uniqueness, identical HLCs collide and idempotency
            // dedup makes the "as-given" and "reversed" replicas drop
            // different ops — a spurious divergence from a malformed test,
            // not from the CRDT.
            let physical = (*ts) * 1000 + i as u64;
            if !created.contains(&node) {
                ops.push(op_at(actor, physical, 0, create_op(node, root, pos("m"))));
                created.insert(node);
            } else {
                ops.push(op_at(actor, physical, 0, move_op(node, parent_node, pos("m"))));
            }
        }

        assert_converges(ops);
    }

    /// Same pool but stronger: two replicas with distinct actors so HLC
    /// tiebreak by actor exercises that code path.
    #[test]
    fn two_actor_convergence(seq in proptest::collection::vec((0..4usize, 0..4usize, 0u32..2, 1u64..1000), 1..25)) {
        let actor1 = ActorId::new();
        let actor2 = ActorId::new();
        let nodes: Vec<NodeId> = (0..4).map(|_| NodeId::new()).collect();
        let root = NodeId::root();

        let mut ops = Vec::new();
        let mut created: std::collections::HashSet<NodeId> = std::collections::HashSet::new();
        for (i, (a, b, who, ts)) in seq.iter().enumerate() {
            let actor = if *who == 0 { actor1 } else { actor2 };
            let node = nodes[*a];
            let parent_node = if *b == *a { root } else { nodes[*b] };
            // Physical = ts * 1000 + i guarantees uniqueness across (ts, i)
            // pairs while preserving ts as the primary ordering dimension.
            // Without uniqueness, identical HLCs collide and idempotency
            // dedup makes the "as-given" and "reversed" replicas drop
            // different ops — a spurious divergence from a malformed test,
            // not from the CRDT.
            let physical = (*ts) * 1000 + i as u64;
            if !created.contains(&node) {
                ops.push(op_at(actor, physical, 0, create_op(node, root, pos("m"))));
                created.insert(node);
            } else {
                ops.push(op_at(actor, physical, 0, move_op(node, parent_node, pos("m"))));
            }
        }

        assert_converges(ops);
    }
}
