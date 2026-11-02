//! The OS-wide quick capture chord.
//!
//! `Cmd/Ctrl+Shift+Space` works with outl in the background: the OS
//! hands it to us, we show and focus the main window and emit
//! `quick-capture://open`, and the frontend opens the same capture box
//! the in-app chord (`Action::QuickCapture`) opens. Nothing is written
//! here; the line becomes an op only when the user submits it.
//!
//! Another app may already own the chord. Registration failing is
//! logged and the app boots anyway, because the in-app chord, the
//! palette and `outl://capture` still reach the box.

use tauri::{AppHandle, Emitter, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// The chord, in the plugin's accelerator syntax. Kept equal to the
/// `QuickCapture` row in `outl_shortcuts::defaults::capture`.
pub const QUICK_CAPTURE_ACCELERATOR: &str = "CommandOrControl+Shift+Space";

/// Event the frontend listens to (`onQuickCaptureHotkey`).
pub const QUICK_CAPTURE_EVENT: &str = "quick-capture://open";

/// The plugin, with one handler that reacts to the press (not the
/// release, or every capture would open twice).
pub fn plugin<R: Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                open_capture(app);
            }
        })
        .build()
}

/// Register the chord with the OS. Called from `setup`, after the
/// plugin is in place. Never fails the boot.
pub fn register<R: Runtime>(app: &AppHandle<R>) {
    let shortcut: Shortcut = match QUICK_CAPTURE_ACCELERATOR.parse() {
        Ok(s) => s,
        Err(err) => {
            tracing::warn!("quick capture: bad accelerator: {err}");
            return;
        }
    };
    if let Err(err) = app.global_shortcut().register(shortcut) {
        tracing::warn!("quick capture: {QUICK_CAPTURE_ACCELERATOR} not registered: {err}");
    }
}

fn open_capture<R: Runtime>(app: &AppHandle<R>) {
    if let Some(win) = app.get_webview_window("main") {
        let _ = win.unminimize();
        let _ = win.show();
        let _ = win.set_focus();
    }
    if let Err(err) = app.emit(QUICK_CAPTURE_EVENT, ()) {
        tracing::warn!("quick capture: failed to emit {QUICK_CAPTURE_EVENT}: {err}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_accelerator_parses() {
        let parsed: Result<Shortcut, _> = QUICK_CAPTURE_ACCELERATOR.parse();
        assert!(parsed.is_ok(), "{parsed:?}");
    }

    #[test]
    fn the_event_name_matches_the_frontend_listener() {
        let events = include_str!("../../src/lib/events.ts");
        assert!(events.contains(&format!("\"{QUICK_CAPTURE_EVENT}\"")));
    }
}
