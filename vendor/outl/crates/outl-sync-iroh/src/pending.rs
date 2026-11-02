//! What each paired device still has to receive from this one.
//!
//! The sync panel's "pending" column. Every completed exchange tells us
//! a floor for what the peer holds, per actor: its own advertised clock,
//! plus (on the initiator side, after the durable-ingest ack) everything
//! we held when we pushed. Ops this device holds above that floor are
//! the ones the peer has not got yet, as far as we can know.
//!
//! The floor is kept per workspace and per peer and is only ever raised.
//! It is persisted to `<root>/.outl/state/sync-pending.json` after every
//! exchange and read back on first use, so a restart keeps the column
//! instead of falling back to "unknown". A floor is a fact about the peer
//! ("it holds at least N ops of actor A"), so merging a file written by
//! another device by maximum is still true. The file is bounded to the
//! [`MAX_PEERS`] most recently seen peers. A peer never exchanged with
//! has no floor and reports "unknown" rather than a number we would have
//! to invent. Counts are distinct ops per actor, the same unit the wire's
//! vector clock uses, so a snapshot boot does not skew them.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use anyhow::Result;
use outl_core::id::ActorId;
use serde::{Deserialize, Serialize};

use crate::oplog_census::local_vector_clock;
use crate::protocol::{ActorClock, SyncRequest};

type Floor = HashMap<ActorId, u64>;

/// What we know a peer holds, and when we last learned it (Unix ms).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub(crate) struct PeerFloor {
    floor: Floor,
    seen_ms: i64,
}

type Book = HashMap<PathBuf, HashMap<String, PeerFloor>>;

/// Peers kept on disk; the least recently seen are dropped past this.
pub const MAX_PEERS: usize = 64;

fn book() -> &'static Mutex<Book> {
    static BOOK: OnceLock<Mutex<Book>> = OnceLock::new();
    BOOK.get_or_init(Mutex::default)
}

/// `<root>/.outl/state/sync-pending.json`.
pub fn state_path(root: &Path) -> PathBuf {
    root.join(".outl").join("state").join("sync-pending.json")
}

fn load(root: &Path) -> HashMap<String, PeerFloor> {
    std::fs::read(state_path(root))
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn save(root: &Path, peers: &HashMap<String, PeerFloor>) {
    let path = state_path(root);
    let write = || -> std::io::Result<()> {
        std::fs::create_dir_all(path.parent().expect("state dir"))?;
        let tmp = path.with_extension("json.tmp");
        std::fs::write(&tmp, serde_json::to_vec(peers)?)?;
        std::fs::rename(&tmp, &path)
    };
    if let Err(e) = write() {
        tracing::warn!("pending: could not persist {}: {e}", path.display());
    }
}

/// The peers for `root`, read from disk the first time this process asks.
fn peers_for<'a>(book: &'a mut Book, root: &Path) -> &'a mut HashMap<String, PeerFloor> {
    book.entry(root.to_path_buf()).or_insert_with(|| load(root))
}

/// Drop the least recently seen peers past [`MAX_PEERS`].
fn bound(peers: &mut HashMap<String, PeerFloor>) {
    while peers.len() > MAX_PEERS {
        let oldest = peers
            .iter()
            .min_by_key(|(_, p)| p.seen_ms)
            .map(|(k, _)| k.clone());
        match oldest {
            Some(k) => peers.remove(&k),
            None => break,
        };
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Raise the floor for `peer` with every clock in `clocks`.
pub(crate) fn record_known(
    workspace_root: &Path,
    peer: &str,
    clocks: &[&HashMap<ActorId, ActorClock>],
) {
    let Ok(mut book) = book().lock() else { return };
    let peers = peers_for(&mut book, workspace_root);
    // Another device sharing this folder may have learned more; a floor
    // is a fact about the peer, so taking the maximum stays true.
    for (k, theirs) in load(workspace_root) {
        let ours = peers.entry(k).or_default();
        for (actor, n) in theirs.floor {
            let slot = ours.floor.entry(actor).or_insert(0);
            *slot = (*slot).max(n);
        }
        ours.seen_ms = ours.seen_ms.max(theirs.seen_ms);
    }
    let entry = peers.entry(peer.to_string()).or_default();
    entry.seen_ms = now_ms();
    for clock in clocks {
        for (actor, c) in clock.iter() {
            let slot = entry.floor.entry(*actor).or_insert(0);
            *slot = (*slot).max(c.count);
        }
    }
    bound(peers);
    save(workspace_root, peers);
}

/// Initiator, after the durable-ingest ack: the peer holds its own clock
/// plus everything we held when we pushed.
pub(crate) fn pushed(
    root: &Path,
    peer: iroh::EndpointId,
    peer_clock: &HashMap<ActorId, ActorClock>,
    ours: &SyncRequest,
) {
    record_known(root, &peer.to_string(), &[peer_clock, &ours.vector_clock]);
}

/// Responder: the initiator's request clock is a floor for what it holds.
pub(crate) fn served(root: &Path, peer: iroh::EndpointId, theirs: &SyncRequest) {
    record_known(root, &peer.to_string(), &[&theirs.vector_clock]);
}

/// Ops in `local` above `floor`, summed across actors.
pub(crate) fn pending_count(local: &HashMap<ActorId, ActorClock>, floor: &Floor) -> u64 {
    local
        .iter()
        .map(|(actor, c)| {
            c.count
                .saturating_sub(floor.get(actor).copied().unwrap_or(0))
        })
        .sum()
}

/// Per peer node id, how many ops this device holds that the peer has not
/// confirmed. Peers never exchanged with (on any run) are absent.
///
/// `actor` is this device's write actor. It is unused: the local clock is
/// read out of the op log's offset index, which needs no write actor, and the
/// count spans every actor's ops rather than only ours. Kept on the signature
/// because both GUI clients and the TUI call this with the actor in hand.
pub fn pending_by_peer(workspace_root: &Path, _actor: ActorId) -> Result<HashMap<String, u64>> {
    let floors: HashMap<String, Floor> = match book().lock() {
        Ok(mut book) => peers_for(&mut book, workspace_root)
            .iter()
            .map(|(k, p)| (k.clone(), p.floor.clone()))
            .collect(),
        Err(_) => HashMap::new(),
    };
    if floors.is_empty() {
        return Ok(HashMap::new());
    }
    let local = local_vector_clock(&workspace_root.join("ops"))?;
    Ok(floors
        .into_iter()
        .map(|(peer, floor)| (peer, pending_count(&local, &floor)))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use outl_core::hlc::Hlc;

    fn clock(entries: &[(ActorId, u64)]) -> HashMap<ActorId, ActorClock> {
        entries
            .iter()
            .map(|(a, n)| {
                (
                    *a,
                    ActorClock {
                        max: Hlc::new(0, 0, *a),
                        count: *n,
                    },
                )
            })
            .collect()
    }

    #[test]
    fn counts_only_what_sits_above_the_floor() {
        let (a, b) = (ActorId::new(), ActorId::new());
        let local = clock(&[(a, 10), (b, 3)]);
        let floor: Floor = [(a, 7)].into_iter().collect();
        assert_eq!(pending_count(&local, &floor), 3 + 3);
    }

    #[test]
    fn a_peer_ahead_of_us_is_not_negative() {
        let a = ActorId::new();
        let floor: Floor = [(a, 9)].into_iter().collect();
        assert_eq!(pending_count(&clock(&[(a, 4)]), &floor), 0);
    }

    /// Forget the in-memory book for `root`, as a fresh process would.
    fn restart(root: &Path) {
        book().lock().unwrap().remove(root);
    }

    #[test]
    fn floors_survive_a_restart() {
        let tmp = tempfile::tempdir().unwrap();
        let a = ActorId::new();
        record_known(tmp.path(), "peer", &[&clock(&[(a, 6)])]);
        restart(tmp.path());
        let floors = peers_for(&mut book().lock().unwrap(), tmp.path()).clone();
        assert_eq!(floors["peer"].floor[&a], 6);
        assert!(state_path(tmp.path()).exists());
    }

    #[test]
    fn a_corrupt_state_file_starts_empty() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(state_path(tmp.path()).parent().unwrap()).unwrap();
        std::fs::write(state_path(tmp.path()), "not json").unwrap();
        assert!(peers_for(&mut book().lock().unwrap(), tmp.path()).is_empty());
    }

    #[test]
    fn the_file_keeps_only_the_most_recent_peers() {
        let tmp = tempfile::tempdir().unwrap();
        let a = ActorId::new();
        for i in 0..(MAX_PEERS + 5) {
            record_known(tmp.path(), &format!("peer-{i}"), &[&clock(&[(a, 1)])]);
        }
        restart(tmp.path());
        let peers = peers_for(&mut book().lock().unwrap(), tmp.path()).clone();
        assert_eq!(peers.len(), MAX_PEERS);
    }

    #[test]
    fn floor_only_rises_and_merges_both_clocks() {
        let tmp = tempfile::tempdir().unwrap();
        let root = tmp.path().to_path_buf();
        let (a, b) = (ActorId::new(), ActorId::new());
        record_known(
            &root,
            "peer",
            &[&clock(&[(a, 5)]), &clock(&[(a, 2), (b, 4)])],
        );
        record_known(&root, "peer", &[&clock(&[(a, 1)])]);
        let floors = book().lock().unwrap().get(&root).cloned().unwrap();
        let floor = &floors["peer"].floor;
        assert_eq!(floor[&a], 5);
        assert_eq!(floor[&b], 4);
    }

    #[test]
    fn unknown_workspace_reports_nothing() {
        let root = PathBuf::from("/tmp/outl-pending-never-synced");
        assert!(pending_by_peer(&root, ActorId::new()).unwrap().is_empty());
    }
}
