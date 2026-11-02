//! Capture inbox (RFC 0363).
//!
//! The desktop has one inbox, the workspace's own `.outl/captures/`,
//! which is where `outl capture` writes. That is what gives a macOS
//! Service or Shortcut, Raycast or a hotkey daemon a way to add a line
//! without the app being on screen — none of them can boot a workspace,
//! and the ones that could should not pay for it per line.
outl_tauri_shared::capture_commands!(crate::state::AppState);
