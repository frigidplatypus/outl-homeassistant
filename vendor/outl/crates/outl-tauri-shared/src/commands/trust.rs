//! The commands behind "trust you can see": who wrote what, and taking
//! your notes with you.
//!
//! Both are thin wrappers. `outl_actions::activity` owns the per-device
//! summary and `outl_actions::export_md` owns what an export copies.

use std::path::PathBuf;

use serde::Serialize;

use crate::host::AppHost;
use crate::offload;

/// One device's share of the op log.
#[derive(Debug, Clone, Serialize)]
pub struct ActorActivityDto {
    /// The actor id (ULID). Not a peer node id: the two are different
    /// identities, so the client labels these by recency, not by alias.
    pub actor: String,
    /// Ops from this actor in the resident log.
    pub ops: usize,
    /// Wall-clock ms of its newest op, `0` when it has none.
    pub last_at_ms: u64,
    /// Whether this is the device asking.
    pub is_local: bool,
}

/// The whole summary.
#[derive(Debug, Clone, Serialize)]
pub struct DeviceActivityDto {
    /// Most recently active first.
    pub actors: Vec<ActorActivityDto>,
    /// `false` when the counts only cover the ops since the last snapshot.
    pub complete: bool,
}

/// What the op log holds from each device.
///
/// Async: the summary is one pass over the whole resident op log, under
/// the workspace lock. That is cheap per op and 200k+ ops on a synced
/// workspace, so it is time the IPC thread would spend accepting no
/// keystrokes ([`crate::offload`]).
pub async fn device_activity<S: AppHost>(state: &S) -> Result<DeviceActivityDto, String> {
    let local = state.hlc().actor();
    let workspace = state.workspace_arc();
    offload::run("device_activity", move || {
        offload::locked(&workspace, |ws| {
            let activity = outl_actions::device_activity(ws, local);
            Ok(DeviceActivityDto {
                complete: activity.complete,
                actors: activity
                    .actors
                    .into_iter()
                    .map(|a| ActorActivityDto {
                        actor: a.actor.0.to_string(),
                        ops: a.ops,
                        last_at_ms: a.last_at_ms,
                        is_local: a.is_local,
                    })
                    .collect(),
            })
        })
    })
    .await
}

/// Where an export landed.
#[derive(Debug, Clone, Serialize)]
pub struct ExportReportDto {
    /// Absolute path of the new folder.
    pub dir: String,
    /// `.md` files copied.
    pub pages: usize,
    /// Asset files copied.
    pub assets: usize,
}

/// Copy the workspace's Markdown and assets into a new folder under
/// `dest_dir`, or next to the workspace folder when `None`. Next to the
/// workspace is the default on every client because it is the one place
/// the user already knows how to find: `~/Documents` on a desktop, and
/// the outl folder in iCloud Drive on iOS, which the Files app shows.
///
/// Async: it copies every page and every asset in the workspace. This is
/// the most I/O-heavy command in the surface, and the one where a
/// synchronous body most obviously froze the window ([`crate::offload`]).
pub async fn export_markdown<S: AppHost>(
    state: &S,
    dest_dir: Option<String>,
) -> Result<ExportReportDto, String> {
    let root = state.storage_root()?;
    let dest = match dest_dir {
        Some(dir) if !dir.is_empty() => PathBuf::from(dir),
        _ => root
            .parent()
            .map(PathBuf::from)
            .ok_or_else(|| format!("{} has no parent folder", root.display()))?,
    };
    // Stamped on the calling thread, so the folder is named for the
    // moment the user pressed the button, not for whenever the pool got
    // round to it.
    let stamp = chrono::Local::now().format("%Y-%m-%d-%H%M%S").to_string();
    offload::run("export_markdown", move || {
        let report = outl_actions::export_markdown(&root, &dest, &stamp)
            .map_err(|e| format!("could not export to {}: {e}", dest.display()))?;
        Ok(ExportReportDto {
            dir: report.dir.to_string_lossy().into_owned(),
            pages: report.pages,
            assets: report.assets,
        })
    })
    .await
}
