use super::*;
use outl_actions::{append_block, open_or_create_page as open_or_create, PageKind};
use outl_core::hlc::HlcGenerator;
use outl_core::id::ActorId;

/// A workspace, then a copy that replays the same log plus `extra`.
fn base() -> (Workspace, HlcGenerator, NodeId, NodeId) {
    let actor = ActorId::new();
    let hlc = HlcGenerator::new(actor);
    let mut ws = Workspace::open_in_memory(actor).unwrap();
    let alpha = open_or_create(&mut ws, &hlc, "alpha", "Alpha", PageKind::Page).unwrap();
    append_block(&mut ws, &hlc, Some(alpha), Some("alpha one")).unwrap();
    let beta = open_or_create(&mut ws, &hlc, "beta", "Beta", PageKind::Page).unwrap();
    append_block(&mut ws, &hlc, Some(beta), Some("beta one")).unwrap();
    (ws, hlc, alpha, beta)
}

fn replay(ws: &Workspace) -> Workspace {
    let mut copy = Workspace::open_in_memory(ActorId::new()).unwrap();
    for op in ws.log().iter() {
        copy.apply(op.clone()).unwrap();
    }
    copy
}

#[test]
fn a_peer_edit_becomes_its_own_undo_step() {
    let (old, _hlc, alpha, _) = base();
    let peer = HlcGenerator::new(ActorId::new());
    let mut fresh = replay(&old);
    append_block(&mut fresh, &peer, Some(alpha), Some("from the phone")).unwrap();

    let mut history = History::new();
    record_external_changes(Some(&old), &fresh, &mut history);

    let stacks = history.get_mut(&alpha).expect("alpha gained a stack");
    let current = render_page_md(&fresh, alpha);
    assert_eq!(stacks.undo(current), Some(render_page_md(&old, alpha)));
}

#[test]
fn history_stays_linear_across_a_peer_edit() {
    let (old, _hlc, alpha, _) = base();
    let mut history = History::new();
    history
        .entry(alpha)
        .or_default()
        .record("edit1-before".into());
    let peer = HlcGenerator::new(ActorId::new());
    let mut fresh = replay(&old);
    append_block(&mut fresh, &peer, Some(alpha), Some("peer")).unwrap();

    record_external_changes(Some(&old), &fresh, &mut history);

    let stacks = history.get_mut(&alpha).unwrap();
    assert_eq!(stacks.undo("now".into()), Some(render_page_md(&old, alpha)));
    assert_eq!(stacks.undo("x".into()), Some("edit1-before".into()));
}

#[test]
fn a_page_only_the_peer_touched_gains_a_stack() {
    let (old, _hlc, alpha, beta) = base();
    let peer = HlcGenerator::new(ActorId::new());
    let mut fresh = replay(&old);
    append_block(&mut fresh, &peer, Some(beta), Some("peer")).unwrap();

    let mut history = History::new();
    record_external_changes(Some(&old), &fresh, &mut history);
    assert!(history.get(&beta).is_some_and(|h| h.can_undo()));
    assert!(!history.contains_key(&alpha), "alpha did not change");
}

#[test]
fn no_baseline_still_drops_everything() {
    let (fresh, _hlc, alpha, _) = base();
    let mut history = History::new();
    history.entry(alpha).or_default().record(String::new());
    record_external_changes(None, &fresh, &mut history);
    assert!(history.is_empty());
}

#[test]
fn a_plugin_write_to_the_page_on_screen_is_undoable() {
    let (mut ws, hlc, alpha, _) = base();
    let before = render_page_md(&ws, alpha);
    append_block(&mut ws, &hlc, Some(alpha), Some("plugin wrote this")).unwrap();
    let mut history = History::new();
    record_plugin_change(
        alpha,
        before.clone(),
        &render_page_md(&ws, alpha),
        &mut history,
    );
    assert_eq!(
        history.get_mut(&alpha).unwrap().undo("x".into()),
        Some(before)
    );

    let same = render_page_md(&ws, alpha);
    let mut untouched = History::new();
    record_plugin_change(alpha, same.clone(), &same, &mut untouched);
    assert!(untouched.is_empty(), "a no-op run adds no step");
}
