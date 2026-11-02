//! Capture inbox (RFC 0363).
//!
//! Mobile has both inboxes: the one the shell exports in
//! `OUTL_CAPTURE_INBOX` (the iOS App Group container, Android's
//! `filesDir`), written by the widget, the App Intent and the share
//! targets, and the workspace's own `.outl/captures/`.
outl_tauri_shared::capture_commands!(crate::state::AppState);
