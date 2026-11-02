//! Answering questions about the op log from its **index alone**.
//!
//! [`JsonlStorage::actor_timestamps`] is the cheap read: which actors have
//! written into an `ops/` directory, and which distinct HLCs this device
//! holds for each. That set is everything a vector clock is, and none of it
//! needs an op.
//!
//! It is deliberately **not** a [`crate::storage::Storage`] method. A trait
//! method takes `&self`, which means a storage already opened — and opening
//! is the cost. `open` → `reload` loads BOTH index sidecars for every actor
//! (the node index being a `HashMap<NodeId, Vec<_>>` filled one `entry()`
//! per op), and the read it feeds, `all_ops`, parses every `Op::Edit`'s
//! `text_op` back into a `Vec<u8>`. On the reference 222,536-op workspace
//! that is 52 MB of sidecar plus 80 MB of `.jsonl` to answer a question
//! whose answer is twenty numbers. `outl-sync-iroh`'s `peer_status` paid
//! exactly that on the iOS main thread, and the scene-update watchdog killed
//! the app before it could paint.
//!
//! What this reads instead is the offset sidecar — `HLC → byte offset`,
//! whose **keys are** the distinct HLC set — through [`fresh_offset_index`],
//! the offset half of the boot path's own freshness ladder
//! (`try_fresh_actor_indexes` calls it and adds the node-sidecar extent
//! check on top). One owner for "what does this sidecar cover", because a
//! second opinion about that is how a stale index becomes a wrong read.
//!
//! Two things it never does, and both are load-bearing:
//!
//! - **It never builds the node index.** It has no use for "which ops
//!   touched this block", and that map is the single most expensive thing
//!   `open` does.
//! - **It never persists anything.** The boot ladder extends and re-saves a
//!   sidecar it found grown. Doing that here would advance the offset
//!   sidecar past the node sidecar, and the next real boot would reject the
//!   mismatched pair and rebuild both from scratch — a status poll arming a
//!   full reindex. Read-only means a grown tail is re-scanned instead, which
//!   costs what was appended since the last write and nothing more.
//!
//! A damaged line costs this read the same way it costs every other
//! sequential pass: the record is skipped, never the rest of the file. The
//! resulting count is then *short*, which is the safe direction for a vector
//! clock — a peer reading a low count concludes we have a gap and resends,
//! where a high one would let it skip ops we never got.

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

use tracing::warn;

use crate::hlc::Hlc;
use crate::id::ActorId;
use crate::storage::sidecar::{self, SidecarKind};
use crate::storage::{OffsetIndex, PageScope, StorageError};

use super::{parse_actor_from_ops_filename, JsonlStorage};

/// How much of a `.jsonl` a loaded offset sidecar accounts for.
pub(super) enum Coverage {
    /// The sidecar spans the file byte-exactly; nothing to re-read.
    Exact,
    /// The log appended since the sidecar was written. The op log only ever
    /// appends, so the prefix `[0, tail_from)` is already indexed
    /// identically and only `[tail_from, EOF)` needs a pass.
    Grew {
        /// First byte the sidecar does not account for.
        tail_from: u64,
    },
}

/// Load `idx_path` and decide what it covers of `jsonl_path`, or `None`
/// meaning "do not trust it".
///
/// The offset half of the boot ladder (`JsonlStorage::load_actor_indexes`
/// documents the whole thing). Conservative in the same way and for the same
/// reason: the sidecar is a cache, so a wrong "fresh" verdict silently loses
/// ops while a needless rebuild is only slower.
pub(super) fn fresh_offset_index(
    jsonl_path: &Path,
    idx_path: &Path,
) -> Option<(OffsetIndex, Coverage)> {
    // Missing, corrupt, or an I/O error: the caller rebuilds.
    let offset_index = OffsetIndex::load(idx_path).ok().flatten()?;
    let file_size = std::fs::metadata(jsonl_path).ok()?.len();

    // Extent the index claims to cover: the record at its MAX offset. An
    // empty index is only trustworthy over an empty file.
    let (off_max, entries_at_max) = match offset_index.max_offset_and_count() {
        Some(v) => v,
        None => return (file_size == 0).then_some((offset_index, Coverage::Exact)),
    };

    // A non-empty index over an empty file is inconsistent.
    if file_size == 0 {
        return None;
    }

    // Read the physical record at `off_max`. It must parse, its byte length
    // gives the covered extent, and its ops must be exactly the entries the
    // index pins to `off_max` — a glued line recovers several ops at one
    // offset, so the two counts have to agree.
    let (consumed, ops_at_max) = JsonlStorage::read_record_at(jsonl_path, off_max)?;
    if ops_at_max.len() != entries_at_max {
        return None;
    }
    for (ts, _) in &ops_at_max {
        if offset_index.get(ts) != Some(off_max) {
            return None;
        }
    }

    let covered_end = off_max + consumed;
    match covered_end.cmp(&file_size) {
        Ordering::Equal => Some((offset_index, Coverage::Exact)),
        // The index points past EOF: the `.jsonl` was truncated or shrank.
        Ordering::Greater => None,
        Ordering::Less => Some((
            offset_index,
            Coverage::Grew {
                tail_from: covered_end,
            },
        )),
    }
}

impl JsonlStorage {
    /// Every distinct HLC the op log under `ops_dir` holds, grouped by the
    /// actor that produced it.
    ///
    /// The group key is [`Hlc::actor`] — the producer the total order
    /// tiebreaks on, and the one actor derivable without reading the op.
    /// `LogOp::actor` carries the same value (it is documented as "also
    /// embedded inside `ts`"), so this groups what an `all_ops` fold groups;
    /// `the_index_census_matches_an_all_ops_fold` pins that.
    ///
    /// Reads no op, builds no node index, and writes nothing — see this
    /// module's header for why each of those matters. A missing `ops_dir`
    /// answers "this device holds nothing" rather than creating the
    /// directory: that is [`JsonlStorage::open`]'s job, and this is not
    /// `open`.
    pub fn actor_timestamps(
        ops_dir: &Path,
    ) -> Result<HashMap<ActorId, BTreeSet<Hlc>>, StorageError> {
        let dir = match std::fs::read_dir(ops_dir) {
            Ok(d) => d,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(HashMap::new()),
            Err(e) => {
                return Err(StorageError::Backend(format!(
                    "read {}: {e}",
                    ops_dir.display()
                )))
            }
        };

        let mut out: HashMap<ActorId, BTreeSet<Hlc>> = HashMap::new();
        for entry in dir {
            let entry = match entry {
                Ok(e) => e,
                Err(e) => {
                    warn!("census: skipping unreadable entry: {e}");
                    continue;
                }
            };
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|s| s.to_str())
                .unwrap_or_default()
                .to_string();
            if !name.starts_with("ops-") || !name.ends_with(".jsonl") {
                continue;
            }
            // A file-sync conflict copy (`ops-<id> 2.jsonl`, `…sync-conflict…`)
            // matches the shape but carries no valid actor id. `reload_global`
            // skips it, so must we — the sidecar naming is derived from the
            // actor, and there isn't one.
            let Some(file_actor) = parse_actor_from_ops_filename(&name) else {
                continue;
            };
            let idx_path =
                sidecar::path_for(ops_dir, file_actor, &PageScope::Global, SidecarKind::Offset);
            for ts in Self::offset_index_only(&path, &idx_path).timestamps() {
                out.entry(ts.actor).or_default().insert(*ts);
            }
        }
        Ok(out)
    }

    /// The offset index for one `.jsonl`: from the sidecar when the ladder
    /// trusts it, from a parse-lite stream otherwise.
    fn offset_index_only(jsonl_path: &Path, idx_path: &Path) -> OffsetIndex {
        match fresh_offset_index(jsonl_path, idx_path) {
            Some((index, Coverage::Exact)) => index,
            Some((mut index, Coverage::Grew { tail_from })) => {
                // `None` is a hard I/O error on the tail. Fall back to the
                // full pass rather than advertise a clock that stops at the
                // last byte the sidecar happened to cover.
                match Self::reindex_tail(jsonl_path, tail_from, &mut index, None) {
                    Some(()) => index,
                    None => Self::stream_offset_index(jsonl_path),
                }
            }
            None => Self::stream_offset_index(jsonl_path),
        }
    }

    /// Full parse-lite pass over one `.jsonl`, offset index only.
    ///
    /// Shares `index_stream` with the boot rebuild, so the skip-don't-stop
    /// discipline invariant #5 hangs on has one implementation. Unlike the
    /// rebuild it passes `None` for the node index: the census has no use for
    /// it, and that map is what makes the rebuild expensive.
    fn stream_offset_index(jsonl_path: &Path) -> OffsetIndex {
        let mut index = OffsetIndex::new();
        let file = match File::open(jsonl_path) {
            Ok(f) => f,
            // A peer's file can vanish mid-sync; it contributes nothing.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return index,
            Err(e) => {
                warn!(
                    "census: cannot open {}: {e} — none of this actor's ops are counted",
                    jsonl_path.display()
                );
                return index;
            }
        };
        Self::index_stream(&mut BufReader::new(file), jsonl_path, &mut index, None);
        index
    }
}
