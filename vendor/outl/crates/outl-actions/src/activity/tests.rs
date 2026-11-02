//! The per-device summary the sync panel shows.

use outl_core::fractional::Fractional;
use outl_core::hlc::HlcGenerator;
use outl_core::id::{ActorId, NodeId};
use outl_core::op::{LogOp, Op};
use outl_core::workspace::Workspace;

use super::*;

fn create(ws: &mut Workspace, hlc: &HlcGenerator) {
    let ts = hlc.next();
    ws.apply(LogOp {
        ts,
        actor: ts.actor,
        op: Op::Create {
            node: NodeId::new(),
            parent: NodeId::root(),
            position: Fractional::first(),
        },
    })
    .expect("apply");
}

#[test]
fn each_actor_gets_its_own_count() {
    let local = ActorId::new();
    let peer = ActorId::new();
    let mut ws = Workspace::open_in_memory(local).expect("ws");
    let mine = HlcGenerator::new(local);
    let theirs = HlcGenerator::with_state(peer, u64::MAX / 2, 0);
    create(&mut ws, &mine);
    create(&mut ws, &mine);
    create(&mut ws, &theirs);

    let activity = device_activity(&ws, local);
    assert!(activity.complete);
    assert_eq!(activity.actors.len(), 2);
    // The peer wrote last, so it sorts first.
    assert_eq!(activity.actors[0].actor, peer);
    assert_eq!(activity.actors[0].ops, 1);
    assert!(!activity.actors[0].is_local);
    assert_eq!(activity.actors[1].actor, local);
    assert_eq!(activity.actors[1].ops, 2);
    assert!(activity.actors[1].is_local);
}

/// A fresh device has written nothing, and the panel still has to show
/// "this device" rather than an empty list that reads like a broken sync.
#[test]
fn the_local_device_is_listed_before_it_writes() {
    let local = ActorId::new();
    let ws = Workspace::open_in_memory(local).expect("ws");
    let activity = device_activity(&ws, local);
    assert_eq!(activity.actors.len(), 1);
    assert!(activity.actors[0].is_local);
    assert_eq!(activity.actors[0].ops, 0);
    assert_eq!(activity.actors[0].last_at_ms, 0);
}
