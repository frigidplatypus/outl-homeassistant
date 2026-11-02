//! Which devices wrote to this workspace, and when they last did.
//!
//! The sync panel's honest half. Peer reachability (`peer_health`) says
//! who is online *now*; this says what the op log actually holds from
//! each actor, which is the thing a user wants to know before trusting
//! that "my laptop's edits are here".
//!
//! Read straight off the in-memory log, one pass, no disk. After a
//! snapshot boot the log only holds the ops since the snapshot, so the
//! answer carries [`DeviceActivity::complete`] and a client must say
//! "since the last snapshot" instead of presenting partial counts as
//! totals.

use std::collections::HashMap;

use outl_core::id::ActorId;
use outl_core::workspace::Workspace;

/// What one actor contributed to the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActorActivity {
    /// The actor (one per device per workspace).
    pub actor: ActorId,
    /// Ops from this actor in the resident log.
    pub ops: usize,
    /// Wall-clock half of the newest op's HLC.
    pub last_at_ms: u64,
    /// Whether this actor is the device asking.
    pub is_local: bool,
}

/// Every actor in the log, most recently active first.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeviceActivity {
    /// One row per actor.
    pub actors: Vec<ActorActivity>,
    /// `false` when the resident log starts at a snapshot, so the counts
    /// cover only the ops since then.
    pub complete: bool,
}

/// Summarise the log per actor. `local` marks this device's row; a device
/// that has not written yet still gets a zero row so the panel can show it.
pub fn device_activity(ws: &Workspace, local: ActorId) -> DeviceActivity {
    let mut by_actor: HashMap<ActorId, (usize, u64)> = HashMap::new();
    for op in ws.log().iter() {
        let entry = by_actor.entry(op.actor).or_insert((0, 0));
        entry.0 += 1;
        entry.1 = entry.1.max(op.ts.physical_ms);
    }
    by_actor.entry(local).or_insert((0, 0));
    let mut actors: Vec<ActorActivity> = by_actor
        .into_iter()
        .map(|(actor, (ops, last_at_ms))| ActorActivity {
            actor,
            ops,
            last_at_ms,
            is_local: actor == local,
        })
        .collect();
    actors.sort_by(|a, b| {
        b.last_at_ms
            .cmp(&a.last_at_ms)
            .then_with(|| a.actor.0.cmp(&b.actor.0))
    });
    DeviceActivity {
        actors,
        complete: !ws.booted_from_snapshot(),
    }
}

#[cfg(test)]
mod tests;
