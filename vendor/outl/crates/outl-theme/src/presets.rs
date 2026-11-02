//! Built-in palette presets.
//!
//! Each `fn` returns a [`Palette`] with every field populated. Hex
//! values are sourced from the TUI's pre-crate `theme.rs` so the
//! shipping look-and-feel is byte-identical after the refactor.
//!
//! When you add a preset, also extend
//! [`crate::PRESETS`] and [`crate::by_name`].
//!
//! This file holds outl's own brand palettes and the two generic
//! fallbacks (ANSI dark, plain light). The six ports of other
//! editors' schemes live in `ports` and are re-exported here, so
//! `presets::dracula()` keeps resolving.

mod ports;

pub use ports::*;

use crate::Palette;

/// outl brand palette — deep purple with lavender accent and lemon
/// highlight. Default for every client.
pub fn outl() -> Palette {
    let bg = "#0c0814";
    let bg_elev = "#15101f";
    let fg = "#f4f1fa";
    let fg_dim = "#b4adc7";
    let fg_dimmer = "#7b7390";
    let border = "#382c54";
    let accent = "#a78bfa";
    let accent_soft = "#c4b5fd";
    let accent_alt = "#d6ff47";
    let warn = "#fbbf24";
    let blue = "#7dd3fc";
    let magenta = "#f0abfc";

    Palette {
        name: "outl".into(),
        bg: bg.into(),
        bg_elev: bg_elev.into(),
        fg: fg.into(),
        fg_dim: fg_dim.into(),
        fg_dimmer: fg_dimmer.into(),
        border: border.into(),
        hint: fg_dim.into(),
        accent: accent.into(),
        accent_fg: bg.into(), // the canvas itself, 7.28:1
        accent_soft: accent_soft.into(),
        accent_alt: accent_alt.into(),
        warn: warn.into(),
        destructive: "#fb7185".into(), // rose-400, the value mobile already ships
        destructive_fg: "#0c0814".into(), // the canvas itself, 7.36:1
        ref_link_fg: accent.into(),
        tag_link_fg: magenta.into(),
        md_link_fg: blue.into(),
        bold_fg: fg.into(),
        italic_fg: accent_soft.into(),
        strike_fg: fg_dimmer.into(),
        highlight_bg: warn.into(),
        highlight_fg: bg.into(),
        code_fg: accent_alt.into(),
        todo_open_fg: warn.into(),
        todo_done_fg: accent_alt.into(),
        todo_done_body_fg: fg_dimmer.into(),
        property_key_fg: fg_dimmer.into(),
        property_value_fg: accent_soft.into(),
        heading_fg: fg.into(),
        dim_fg: fg_dimmer.into(),
        selected_bullet_bg: accent.into(),
        selected_bullet_fg: bg.into(),
        cursor_block_bg: fg.into(),
        cursor_block_fg: bg.into(),
        cursor_caret_fg: accent_soft.into(),
        status_normal_bg: accent.into(),
        status_normal_fg: bg.into(),
        status_insert_bg: accent_alt.into(),
        status_insert_fg: bg.into(),
        status_visual_bg: magenta.into(),
        status_visual_fg: bg.into(),
        status_message_fg: warn.into(),
        list_selected_bg: accent.into(),
        list_selected_fg: bg.into(),
        help_title_fg: accent_soft.into(),
    }
}

/// outl brand palette, light side — soft purple-tinted canvas with
/// the same brand violet accent as [`outl`], darkened where needed
/// for contrast on a light background.
///
/// This is the light palette mobile shipped before RFC 0022, as
/// `--color-ios-*` CSS custom properties in `outl-mobile/src/styles.css`
/// (`bg: #f6f4fb`, `card: #ffffff`, `divider: #d8d0e8`, `text: #0c0814`,
/// `accent: #7c3aed`, `destructive: #e11d48`) — recovered from git
/// history rather than deleted, per RFC 0022's "the migration moves
/// state in, it does not delete it". `fg_dim` / `fg_dimmer` were
/// alpha blends (`rgba(28, 21, 48, 0.62)` / `rgba(28, 21, 48, 0.42)`
/// over that `bg`); `Palette` fields are opaque hex, so they are
/// composited here into solid hex once (`#6f6a7d`, `#9a96a6`) rather
/// than carried as CSS `rgba()`.
///
/// The remaining fields (`ref_link_fg`, `code_fg`, `status_*`, …)
/// have no prior mobile-only value to recover — they mirror
/// [`outl`]'s structure with each hue darkened for a light canvas,
/// the way [`logseq_light`] darkens its terminal-flavoured fields
/// relative to its dark siblings. Paper white, cool grays, one violet.
pub fn outl_light() -> Palette {
    let bg = "#ffffff";
    let bg_elev = "#f7f7fa"; // a hair off white: sheets read as raised
    let fg = "#111827"; // 17.7:1 on `bg`
    let fg_dim = "#6b7280"; // 4.83:1 on `bg`, 4.6:1 on `bg_elev`
    let fg_dimmer = "#9ca3af"; // decoration and struck-through text only
    let border = "#e5e7eb";
    let accent = "#6d28d9"; // violet-700, 7.1:1 on `bg`
    let accent_soft = "#7c3aed";
    let accent_alt = "#4d7c0f"; // lime-700: 4.99:1 under the white canvas, so insert mode reads
    let warn = "#b45309"; // amber-700, 5.0:1
    let blue = "#0369a1"; // sky-700, 5.9:1
    let magenta = "#a21caf";

    Palette {
        name: "outl-light".into(),
        bg: bg.into(),
        bg_elev: bg_elev.into(),
        fg: fg.into(),
        fg_dim: fg_dim.into(),
        fg_dimmer: fg_dimmer.into(),
        border: border.into(),
        hint: fg_dim.into(),
        accent: accent.into(),
        accent_fg: bg.into(), // the canvas itself, 7.10:1
        accent_soft: accent_soft.into(),
        accent_alt: accent_alt.into(),
        warn: warn.into(),
        destructive: "#e11d48".into(), // rose-600, the value mobile already shipped
        destructive_fg: "#ffffff".into(), // 4.70:1, and the canvas is white too
        ref_link_fg: accent.into(),
        tag_link_fg: magenta.into(),
        md_link_fg: blue.into(),
        bold_fg: fg.into(),
        italic_fg: accent_soft.into(),
        strike_fg: fg_dimmer.into(),
        // Highlighter markers stay bright-bg/dark-fg regardless of
        // theme (a real highlighter pen doesn't get darker on light
        // paper) — mirrors `light()` / `logseq_light()`, not `outl()`.
        highlight_bg: "#fde68a".into(),
        highlight_fg: fg.into(),
        code_fg: accent_alt.into(),
        todo_open_fg: warn.into(),
        todo_done_fg: accent_alt.into(),
        todo_done_body_fg: fg_dimmer.into(),
        property_key_fg: fg_dimmer.into(),
        property_value_fg: accent_soft.into(),
        heading_fg: fg.into(),
        dim_fg: fg_dimmer.into(),
        selected_bullet_bg: accent.into(),
        selected_bullet_fg: bg.into(),
        cursor_block_bg: fg.into(),
        cursor_block_fg: bg.into(),
        cursor_caret_fg: accent_soft.into(),
        status_normal_bg: accent.into(),
        status_normal_fg: bg.into(),
        status_insert_bg: accent_alt.into(),
        status_insert_fg: bg.into(),
        status_visual_bg: magenta.into(),
        status_visual_fg: bg.into(),
        status_message_fg: warn.into(),
        list_selected_bg: accent.into(),
        list_selected_fg: bg.into(),
        help_title_fg: accent_soft.into(),
    }
}

/// Default dark — ANSI 16-color equivalents in hex so the desktop
/// has something to paint. The TUI rendering of this preset on a
/// real terminal continues to use the terminal's native ANSI
/// palette (see `outl-tui/src/theme.rs::default_dark` which builds
/// from explicit `Color::*` values, not from this palette).
pub fn default_dark() -> Palette {
    let bg = "#000000";
    let fg = "#cccccc";
    let dark_gray = "#555555";
    let gray = "#aaaaaa";
    let cyan = "#00aaaa";
    let magenta = "#aa00aa";
    let blue = "#0066ff";
    let green = "#00aa00";
    let yellow = "#ffff55";

    Palette {
        name: "default-dark".into(),
        bg: bg.into(),
        bg_elev: "#101010".into(),
        fg: fg.into(),
        fg_dim: gray.into(),
        fg_dimmer: dark_gray.into(),
        border: dark_gray.into(),
        hint: gray.into(),
        accent: cyan.into(),
        accent_fg: bg.into(), // the canvas itself, 7.33:1
        accent_soft: "#55ffff".into(),
        accent_alt: green.into(),
        warn: yellow.into(),
        destructive: "#ff5555".into(),    // ANSI bright red
        destructive_fg: "#000000".into(), // ANSI black, 6.68:1
        ref_link_fg: cyan.into(),
        tag_link_fg: magenta.into(),
        md_link_fg: blue.into(),
        bold_fg: fg.into(),
        italic_fg: fg.into(),
        strike_fg: dark_gray.into(),
        highlight_bg: yellow.into(),
        highlight_fg: bg.into(),
        code_fg: green.into(),
        todo_open_fg: yellow.into(),
        todo_done_fg: green.into(),
        todo_done_body_fg: dark_gray.into(),
        property_key_fg: dark_gray.into(),
        property_value_fg: gray.into(),
        heading_fg: fg.into(),
        dim_fg: dark_gray.into(),
        selected_bullet_bg: cyan.into(),
        selected_bullet_fg: bg.into(),
        cursor_block_bg: "#ffffff".into(),
        cursor_block_fg: bg.into(),
        cursor_caret_fg: "#ffffff".into(),
        status_normal_bg: cyan.into(),
        status_normal_fg: bg.into(),
        status_insert_bg: green.into(),
        status_insert_fg: bg.into(),
        status_visual_bg: magenta.into(),
        status_visual_fg: bg.into(),
        status_message_fg: yellow.into(),
        list_selected_bg: cyan.into(),
        list_selected_fg: bg.into(),
        help_title_fg: yellow.into(),
    }
}

/// Light palette for high-brightness environments.
pub fn light() -> Palette {
    let bg = "#ffffff";
    let fg = "#1a1a1a";
    let dark_gray = "#555555";
    let gray = "#888888";
    let blue = "#0066ff";
    let magenta = "#aa00aa";
    let red = "#cc0033";
    let green = "#007733";
    let yellow = "#aa6600";

    Palette {
        name: "light".into(),
        bg: bg.into(),
        bg_elev: "#f4f4f4".into(),
        fg: fg.into(),
        fg_dim: "#6e6e6e".into(), // 5.1:1 on bg, 4.6:1 on bg_elev
        fg_dimmer: "#bbbbbb".into(),
        border: gray.into(),
        hint: dark_gray.into(),
        accent: blue.into(),
        accent_fg: bg.into(), // the canvas itself, 4.83:1
        accent_soft: "#3399ff".into(),
        accent_alt: green.into(),
        warn: yellow.into(),
        destructive: "#dc2626".into(),    // red-600, readable on white
        destructive_fg: "#ffffff".into(), // the canvas itself, 4.83:1
        ref_link_fg: blue.into(),
        tag_link_fg: red.into(),
        md_link_fg: blue.into(),
        bold_fg: fg.into(),
        italic_fg: fg.into(),
        strike_fg: gray.into(),
        highlight_bg: "#fff59d".into(),
        highlight_fg: fg.into(),
        code_fg: magenta.into(),
        todo_open_fg: red.into(),
        todo_done_fg: green.into(),
        todo_done_body_fg: gray.into(),
        property_key_fg: gray.into(),
        property_value_fg: dark_gray.into(),
        heading_fg: fg.into(),
        dim_fg: gray.into(),
        selected_bullet_bg: blue.into(),
        selected_bullet_fg: bg.into(),
        cursor_block_bg: fg.into(),
        cursor_block_fg: bg.into(),
        cursor_caret_fg: fg.into(),
        status_normal_bg: blue.into(),
        status_normal_fg: bg.into(),
        status_insert_bg: green.into(),
        status_insert_fg: bg.into(),
        status_visual_bg: magenta.into(),
        status_visual_fg: bg.into(),
        status_message_fg: yellow.into(),
        list_selected_bg: blue.into(),
        list_selected_fg: bg.into(),
        help_title_fg: blue.into(),
    }
}
