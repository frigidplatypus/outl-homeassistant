//! Event hub: the server's replacement for Tauri's `AppHandle::emit`.
//!
//! The desktop backend pushes events (`workspace-ready`, `peer-ops-changed`,
//! `workspace-reconciled`, `sync-progress`, …) to the webview through Tauri.
//! Here the same events go to every connected browser over a single
//! WebSocket, fanned out by a tokio broadcast channel. The frontend's
//! `listen` shim (see `frontend/shims/event.ts`) subscribes per event name.

use std::sync::Arc;

use serde::Serialize;
use tokio::sync::broadcast;

/// One event frame, serialized as `{"event": name, "payload": …}`.
#[derive(Clone, Serialize)]
pub struct EventFrame {
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub payload: Option<serde_json::Value>,
}

/// A process-wide fan-out of backend events to all live WebSocket clients.
///
/// `broadcast` drops the oldest frame for a slow receiver rather than
/// blocking the emitter — correct for UI events, where a stale
/// `peer-ops-changed` is harmless (the next one, or the periodic reload,
/// catches up). The channel is sized generously so a burst of sync progress
/// ticks does not drop the `workspace-ready` that follows.
#[derive(Clone)]
pub struct EventHub {
    tx: broadcast::Sender<EventFrame>,
}

impl EventHub {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(1024);
        Self { tx }
    }

    /// Emit a unit event (no payload), e.g. `workspace-ready`.
    pub fn emit(&self, event: &str) {
        let _ = self.tx.send(EventFrame {
            event: event.to_string(),
            payload: None,
        });
    }

    /// Emit an event carrying a serializable payload.
    pub fn emit_payload<T: Serialize>(&self, event: &str, payload: &T) {
        let value = serde_json::to_value(payload).unwrap_or(serde_json::Value::Null);
        let _ = self.tx.send(EventFrame {
            event: event.to_string(),
            payload: Some(value),
        });
    }

    /// A receiver for the WebSocket handler. Each connection takes its own
    /// receiver; `broadcast` gives every receiver the full stream.
    pub fn subscribe(&self) -> broadcast::Receiver<EventFrame> {
        self.tx.subscribe()
    }
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

/// Shared handle so background threads (the iroh bridge, the file watcher,
/// the boot opener) can reach the hub without an `AppHandle`.
pub type Hub = Arc<EventHub>;
