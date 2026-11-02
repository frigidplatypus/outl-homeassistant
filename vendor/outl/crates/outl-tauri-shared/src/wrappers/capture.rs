//! Capture inbox command (RFC 0363). Its own macro because `catalog.rs`
//! sits at its size ceiling. **Both** GUI clients register it: mobile
//! for the iOS and Android surfaces, the desktop for what `outl capture`
//! leaves in the workspace inbox.
//!
//! `tests/command_parity.rs` is what keeps that true — it scans every
//! `src/wrappers/*.rs`, not just `catalog.rs`, so a client dropping this
//! module fails the build. Before that fix the test only read
//! `catalog.rs` and this macro was invisible to it.

/// `drain_capture_inbox`: land whatever is waiting in this device's
/// capture inboxes.
#[macro_export]
macro_rules! capture_commands {
    ($state:ty) => {
        $crate::tauri_commands! {
            state = $state;

            /// Land every capture waiting in the App Group inbox.
            fn drain_capture_inbox()
                -> ::std::result::Result<$crate::capture_inbox::CaptureDrainDto, String>
                => $crate::capture_inbox::drain_capture_inbox;
        }
    };
}
