//! Server `AppState` + its [`AppHost`] projection.
//!
//! This is the same shape as the desktop's `AppState`, minus the Tauri-only
//! fields (the `AppHandle`, the OS notification path, the deep-link buffer).
//! The command bodies in `outl_tauri_shared::commands` are generic over
//! `S: AppHost`, so they run unchanged against this state — the server is a
//! second host for the exact same business logic the desktop app uses.
//!
//! The one structural difference from the desktop is absorbed the same way:
//! `storage_root()` returns the [`ERR_LOADING`] sentinel while no workspace
//! is open. In the addon the workspace is a fixed mounted volume opened at
//! boot, so the slot is populated before the first request.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use outl_actions::{BacklinkIndex, HistoryStacks, SyncTransport};
use outl_core::hlc::HlcGenerator;
use outl_core::id::NodeId;
use outl_core::workspace::Workspace;
use outl_tauri_shared::{AppHost, PluginService, ProjectionWriter};
use outl_exec::RuntimeRegistry;
use outl_tauri_shared::workspace_open::WorkspaceGuards;
use parking_lot::Mutex;

use crate::events::Hub;
use crate::settings::Settings;

pub(crate) use outl_tauri_shared::ERR_LOADING;

/// Shared mutable state for the server process.
pub struct AppState {
    /// The active workspace. `None` until the boot opener completes.
    pub workspace: Arc<Mutex<Option<Workspace>>>,
    /// Filesystem root of the active workspace (parent of `ops/`,
    /// `journals/`, `pages/`). Tracks `workspace` 1:1.
    pub storage_root: Arc<Mutex<Option<PathBuf>>>,
    /// Cross-process workspace locks, held for as long as a workspace is
    /// open. Written only by `open_workspace_at`.
    pub workspace_guards: Arc<Mutex<Option<WorkspaceGuards>>>,
    /// Per-device HLC generator. Static for the process lifetime.
    pub hlc: HlcGenerator,
    /// User preferences and last opened workspace path.
    pub settings: Arc<Mutex<Settings>>,
    /// Code-block runtimes built once at startup.
    pub registry: Arc<RuntimeRegistry>,
    /// Active P2P sync transport, when `[sync] transport = "iroh"`.
    pub iroh_transport: Arc<Mutex<Option<Arc<dyn SyncTransport>>>>,
    /// Per-page undo / redo stacks of rendered `.md` snapshots.
    pub history: Mutex<HashMap<NodeId, HistoryStacks<String>>>,
    /// Pre-computed backlinks index over the whole workspace.
    pub backlink_index: Arc<Mutex<Option<BacklinkIndex>>>,
    /// Background `.md` + sidecar projection writer.
    pub projection_writer: ProjectionWriter,
    /// Event fan-out to the connected browsers (replaces `AppHandle::emit`).
    pub events: Hub,
    /// MCP stdio bridge to the `outl mcp serve` child.
    pub mcp: std::sync::Arc<crate::mcp::McpBridge>,
    /// Plugin host (Boa `Context` on a dedicated thread).
    pub plugins: PluginService,
}

impl AppHost for AppState {
    fn workspace(&self) -> &Mutex<Option<Workspace>> {
        &self.workspace
    }

    fn workspace_arc(&self) -> Arc<Mutex<Option<Workspace>>> {
        self.workspace.clone()
    }

    fn hlc(&self) -> &HlcGenerator {
        &self.hlc
    }

    fn storage_root(&self) -> Result<PathBuf, String> {
        self.storage_root
            .lock()
            .clone()
            .ok_or_else(|| ERR_LOADING.to_string())
    }

    fn sync_transport(&self) -> Option<Arc<dyn SyncTransport>> {
        self.iroh_transport.lock().clone()
    }

    fn exec_registry(&self) -> Arc<RuntimeRegistry> {
        self.registry.clone()
    }

    fn history(&self) -> Option<&Mutex<HashMap<NodeId, HistoryStacks<String>>>> {
        Some(&self.history)
    }

    fn backlink_index(&self) -> Option<Arc<Mutex<Option<BacklinkIndex>>>> {
        Some(self.backlink_index.clone())
    }

    fn projection_writer(&self) -> Option<&ProjectionWriter> {
        Some(&self.projection_writer)
    }
}
