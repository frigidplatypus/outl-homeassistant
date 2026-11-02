//! WebSocket endpoint that streams backend events to the browser.
//!
//! The frontend's `listen` shim opens one WebSocket to `/api/events` and
//! dispatches `{event, payload}` frames to per-name handlers. This handler
//! upgrades the connection, subscribes to the [`EventHub`], and forwards
//! every frame. A connection that drops is simply removed from the
//! broadcast; the hub keeps running for the other clients.

use std::sync::Arc;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use tokio::sync::broadcast::error::RecvError;

use crate::events::EventHub;
use crate::state::AppState;

/// `GET /api/events` — upgrade to WebSocket and stream events.
pub async fn events_ws(
    ws: WebSocketUpgrade,
    State(state): State<Arc<AppState>>,
) -> Response {
    let hub = state.events.clone();
    ws.on_upgrade(move |socket| handle_socket(socket, hub))
}

async fn handle_socket(mut socket: WebSocket, hub: Arc<EventHub>) {
    let mut rx = hub.subscribe();
    loop {
        tokio::select! {
            frame = rx.recv() => match frame {
                Ok(frame) => {
                    let json = match serde_json::to_string(&frame) {
                        Ok(j) => j,
                        Err(_) => continue,
                    };
                    if socket.send(Message::Text(json.into())).await.is_err() {
                        break; // client gone
                    }
                }
                Err(RecvError::Lagged(_)) => continue, // dropped a frame; keep going
                Err(RecvError::Closed) => break,
            },
            inbound = socket.recv() => match inbound {
                // The frontend sends no inbound protocol; reading keeps the
                // connection healthy and detects a close.
                Some(Ok(_)) => {}
                Some(Err(_)) | None => break,
            },
        }
    }
}
