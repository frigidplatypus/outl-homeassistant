//! Undo steps for changes this device did not type: peer ops landing in
//! a reload, and plugin writes. RFC 0362, "Undo across sync and plugins".
//!
//! An outside change is pushed onto the touched page's undo stack as the
//! page's render *before* the change, so `Cmd+Z` steps it back through
//! `restore_page_md` like any other undo: new ops, never a rewind.
//! History stays linear in time, so an older snapshot is only restored
//! after the outside change itself was undone.

use std::collections::{HashMap, HashSet};

use outl_actions::{render_page_md, HistoryStacks};
use outl_core::hlc::Hlc;
use outl_core::id::NodeId;
use outl_core::op::op_node;
use outl_core::workspace::Workspace;

use crate::helpers::{parse_node_id, with_ws};
use crate::host::AppHost;

type History = HashMap<NodeId, HistoryStacks<String>>;

/// Pages whose blocks the ops in `fresh` but not in `old` touch, looked
/// up in both trees so a block moved across pages counts for each.
fn pages_touched(old: &Workspace, fresh: &Workspace) -> HashSet<NodeId> {
    let seen: HashSet<Hlc> = old.log().iter().map(|op| op.ts).collect();
    let mut pages = HashSet::new();
    for op in fresh.log().iter().filter(|op| !seen.contains(&op.ts)) {
        let Some(node) = op_node(&op.op) else {
            continue;
        };
        for ws in [old, fresh] {
            if let Some(page) = outl_actions::enclosing_page_id(ws, node) {
                pages.insert(page);
            }
        }
    }
    pages
}

/// Called with the pre- and post-reload trees before the swap. Every page
/// the reload changed gains an undo step holding its old render. With no
/// baseline (`old` is `None`) there is nothing to diff, so every stack is
/// dropped, as before.
pub fn record_external_changes(old: Option<&Workspace>, fresh: &Workspace, history: &mut History) {
    let Some(old) = old else {
        history.clear();
        return;
    };
    // Pages with a stack are always diffed too: after a snapshot boot the
    // "new ops" set can miss what changed.
    let mut pages = pages_touched(old, fresh);
    pages.extend(history.keys().copied());
    for page in pages {
        let before = render_page_md(old, page);
        if before != render_page_md(fresh, page) {
            history.entry(page).or_default().record(before);
        }
    }
}

/// After a plugin run: push `before` when the page on screen changed.
pub fn record_plugin_change(page: NodeId, before: String, after: &str, history: &mut History) {
    if before != after {
        history.entry(page).or_default().record(before);
    }
}

/// The on-screen page's render before a plugin runs, so its write can be
/// undone (RFC 0362, "Undo across sync and plugins").
pub fn page_before<S: AppHost>(state: &S, page_id: Option<&str>) -> Option<(NodeId, String)> {
    let page = parse_node_id(page_id?).ok()?;
    state.history()?;
    with_ws(state, |ws| Ok(render_page_md(ws, page)))
        .ok()
        .map(|md| (page, md))
}

/// After a plugin applied ops: push the before render if the page changed.
pub fn record_plugin_step<S: AppHost>(state: &S, before: Option<(NodeId, String)>) {
    let (Some((page, md)), Some(history)) = (before, state.history()) else {
        return;
    };
    let Ok(after) = with_ws(state, |ws| Ok(render_page_md(ws, page))) else {
        return;
    };
    record_plugin_change(page, md, &after, &mut history.lock());
}

#[cfg(test)]
mod tests;
