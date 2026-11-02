//! Overlay rows (picker / palette). Same owner as `defaults.rs`,
//! split out only because that file sits at its size ceiling.

use super::chords::key;
use crate::action::Action;
use crate::binding::{Binding, Mode::Overlay};
use crate::chord::Key;

pub(super) fn bindings() -> Vec<Binding> {
    vec![
        // Picker / palette have their own keys; arrow + Enter + Esc.
        Binding::new(key(Key::Esc), Overlay, Action::ExitInsert, "Close overlay"),
        Binding::new(
            key(Key::Down),
            Overlay,
            Action::SelectionDown,
            "Highlight next",
        ),
        Binding::new(
            key(Key::Up),
            Overlay,
            Action::SelectionUp,
            "Highlight previous",
        ),
    ]
}
