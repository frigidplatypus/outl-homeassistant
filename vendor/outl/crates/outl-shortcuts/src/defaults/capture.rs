//! Quick capture rows. Same owner as `defaults.rs`, split out only
//! because that file sits at its size ceiling.

use super::chords::{shift_ctrl_key, shift_meta_key};
use crate::action::Action;
use crate::binding::{Binding, Mode};
use crate::chord::Key;

/// `Cmd/Ctrl+Shift+Space` opens the capture box. The desktop also
/// registers the same chord with the OS, so it works with the window
/// in the background; this row is what the palette and help list.
pub(super) fn bindings() -> Vec<Binding> {
    let label = "Quick capture to today";
    vec![
        Binding::new(
            shift_meta_key(Key::Space),
            Mode::Global,
            Action::QuickCapture,
            label,
        ),
        Binding::new(
            shift_ctrl_key(Key::Space),
            Mode::Global,
            Action::QuickCapture,
            label,
        ),
    ]
}
