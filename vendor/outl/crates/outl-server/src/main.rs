//! outl-server: a headless outl host for the Home Assistant addon.
//!
//! Serves the browser UI (the desktop client, built for a plain browser) over
//! HTTP, reuses the desktop's command bodies as the backend, holds the iroh
//! P2P endpoint so paired devices converge, exposes the MCP server over HTTP,
//! and offers a read-only filesystem API over the shared workspace volume.
//!
//! Environment:
//! - `OUTL_WORKSPACE` — the workspace root (default `/data`). This is the
//!   shared volume; other HA processes read it directly or through `/api/fs`.
//! - `OUTL_PORT` — the HTTP port (default `8090`, the HA addon convention).
//! - `OUTL_STATIC_DIR` — the built frontend (default `/app/static`).
//! - `RUST_LOG` — log filter (default `info`).

mod boot;
mod dispatch;
mod events;
mod fs;
mod mcp;
mod settings;
mod state;
mod ws;

use std::path::PathBuf;
use std::sync::Arc;

use axum::response::IntoResponse;
use axum::routing::{get, post};
use axum::Router;
use outl_core::hlc::HlcGenerator;
use outl_core::workspace::Workspace;
use outl_exec::RuntimeRegistry;
use outl_plugins::{Capability, ClientCapabilities};
use outl_tauri_shared::workspace_open::load_or_create_actor;
use outl_tauri_shared::{PluginService, ProjectionWriter};
use parking_lot::Mutex;
use tower_http::services::ServeDir;
use tracing::info;

use crate::events::EventHub;
use crate::state::AppState;

/// The server's plugin capability set — the same as the desktop, since the
/// browser UI is the desktop client and can render every surface.
fn client_capabilities() -> ClientCapabilities {
    [
        Capability::SlashCommand,
        Capability::OpHook,
        Capability::UiRender,
        Capability::Keybinding,
        Capability::ToolbarButton,
        Capability::ContentTransformerText,
        Capability::ContentTransformerRich,
    ]
    .into_iter()
    .collect()
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Log subscriber before anything that logs.
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info,outl_sync_iroh=debug,iroh=info")),
        )
        .log_internal_errors(false)
        .finish();
    let _ = tracing::subscriber::set_global_default(subscriber);

    // rustls needs a process-wide crypto provider before iroh builds a client.
    let _ = rustls::crypto::ring::default_provider().install_default();

    let workspace_root = PathBuf::from(env_or("OUTL_WORKSPACE", "/data"));
    let port: u16 = env_or("OUTL_PORT", "8090").parse().unwrap_or(8090);
    let static_dir = PathBuf::from(env_or("OUTL_STATIC_DIR", "/app/static"));

    // Local-only state (the per-device `actor` ULID and `config.toml`) lives
    // at `~/.config/outl/` — the XDG path the TUI and CLI also read, so all
    // clients on the machine share one source of truth.
    let app_config_dir = outl_config::config_dir();
    std::fs::create_dir_all(&app_config_dir)?;

    let actor = load_or_create_actor(&app_config_dir)?;
    let hlc = HlcGenerator::new(actor);

    // Resolve the journal/clock timezone once, before any workspace opens.
    let global_cfg = outl_config::load();
    outl_actions::clock::init(global_cfg.calendar.timezone.as_deref());

    // The addon's workspace is fixed (the mounted volume), so record it as the
    // last-opened workspace. The frontend's "has a workspace configured" gate
    // reads `settings.last_workspace != null`; setting it means the workspace
    // picker never appears and the user lands straight in the app. Persist it
    // so the config file is written automatically on first boot.
    let mut settings = settings::load();
    if settings.last_workspace.as_ref() != Some(&workspace_root) {
        settings.last_workspace = Some(workspace_root.clone());
        if let Err(e) = settings::save(&settings) {
            tracing::warn!("failed to persist last_workspace: {e}");
        }
    }

    let workspace: Arc<Mutex<Option<Workspace>>> = Arc::new(Mutex::new(None));
    let storage_root: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
    let workspace_guards: Arc<Mutex<Option<outl_tauri_shared::workspace_open::WorkspaceGuards>>> =
        Arc::new(Mutex::new(None));
    let registry = Arc::new(RuntimeRegistry::with_builtins());
    let iroh_transport: Arc<Mutex<Option<Arc<dyn outl_actions::SyncTransport>>>> =
        Arc::new(Mutex::new(None));
    let events = Arc::new(EventHub::new());

    // Background projection writer: mutations queue their page here so the
    // `.md` + sidecar write happens off the request thread.
    let projection_writer = ProjectionWriter::spawn(
        workspace.clone(),
        storage_root.clone(),
        {
            let events = events.clone();
            move |failure| {
                events.emit_payload(
                    outl_tauri_shared::PROJECTION_WRITE_FAILED_EVENT,
                    &failure,
                );
            }
        },
    );

    // Plugin host on its own thread (the Boa `Context` is `!Send`).
    let plugins = PluginService::spawn(
        "server",
        client_capabilities,
        workspace.clone(),
        storage_root.clone(),
        hlc.clone(),
    );

    let mcp = std::sync::Arc::new(mcp::McpBridge::new(workspace_root.clone()));

    let state = Arc::new(AppState {
        workspace,
        storage_root,
        workspace_guards,
        hlc,
        settings: Arc::new(Mutex::new(settings)),
        registry,
        iroh_transport,
        history: Mutex::new(std::collections::HashMap::new()),
        backlink_index: Arc::new(Mutex::new(None)),
        projection_writer,
        events,
        mcp,
        plugins,
    });

    // Open the workspace in the background so the HTTP server is up before
    // the filesystem work completes.
    boot::spawn_workspace_opener(state.clone(), workspace_root.clone());

    // Build the router.
    let app = Router::new()
        // The command surface: the browser UI's `invoke` lands here.
        .route("/api/invoke/{cmd}", post(dispatch::invoke))
        // Live events over WebSocket.
        .route("/api/events", get(ws::events_ws))
        // MCP over HTTP (JSON-RPC).
        .route("/api/mcp", post(mcp::mcp_handler))
        // Read-only filesystem API for other HA processes.
        .route("/api/fs/list", get(fs::list))
        .route("/api/fs/read", get(fs::read))
        // Hostname for the Sync panel's device label.
        .route("/api/hostname", get(hostname_handler))
        // Static UI.
        .fallback_service(ServeDir::new(static_dir))
        .with_state(state);

    let addr = std::net::SocketAddr::from(([0, 0, 0, 0], port));
    info!("outl-server listening on {addr} (workspace: {})", workspace_root.display());
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn hostname_handler() -> axum::response::Response {
    let name = hostname::get()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|_| "home-assistant".to_string());
    axum::Json(serde_json::json!({ "hostname": name })).into_response()
}
