//! Focus mode row. Same owner as `defaults.rs`, split out only
//! because that file sits at its size ceiling.

use super::chords::{pair, shift_ctrl_ch, shift_meta_ch};
use crate::action::Action;
use crate::binding::{Binding, Mode};

/// `Cmd/Ctrl+Shift+F` toggles focus mode, the same switch as the
/// button in the top bar. Spelled for both OSes because the desktop
/// adapter never rewrites `Cmd` to `Ctrl`.
///
/// The terminal takes `z f` in Normal mode. A terminal reports
/// `Ctrl+Shift+F` as `Ctrl+F` and plain `F` is find-backward, so the
/// GUI chord can't carry over; `z` is already the view family (`zz`,
/// `zi`, `zo`), and outl has no vim `zf` fold to collide with.
pub(super) fn bindings() -> Vec<Binding> {
    let label = "Toggle focus mode";
    vec![
        Binding::new(
            shift_meta_ch('f'),
            Mode::Global,
            Action::ToggleFocusMode,
            label,
        ),
        Binding::new(
            shift_ctrl_ch('f'),
            Mode::Global,
            Action::ToggleFocusMode,
            label,
        ),
        Binding::new(pair('z', 'f'), Mode::Normal, Action::ToggleFocusMode, label),
    ]
}

#[cfg(test)]
mod tests {
    use crate::action::Action;
    use crate::binding::Mode;
    use crate::{lookup, Chord, ChordSequence};

    #[test]
    fn focus_mode_has_a_chord_on_both_oses_and_the_terminal() {
        let rows: Vec<_> = crate::default_bindings()
            .into_iter()
            .filter(|b| b.action == Action::ToggleFocusMode)
            .collect();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn the_terminal_chord_leaves_find_backward_alone() {
        let zf = ChordSequence::pair(Chord::ch('z'), Chord::ch('f'));
        assert_eq!(lookup(Mode::Normal, &zf), Some(Action::ToggleFocusMode));
        let shift_f =
            ChordSequence::chord(Chord::new(crate::Modifiers::SHIFT, crate::Key::char('f')));
        assert_ne!(
            lookup(Mode::Normal, &shift_f),
            Some(Action::ToggleFocusMode)
        );
    }
}
