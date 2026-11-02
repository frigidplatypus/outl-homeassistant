//! Renderer for the sync panel (`:sync`). Pure projection of
//! [`SyncPanelState`]; the data and undo live in `actions/sync_panel.rs`.

use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};

use super::overlays::centered_rect;
use crate::actions::sync_panel::SyncPanelState;
use crate::state::App;

/// Device and actor ids are ULIDs; the tail is what tells them apart.
fn short(id: &str) -> &str {
    &id[id.len().saturating_sub(6)..]
}

pub(crate) fn lines(app: &App, state: &SyncPanelState) -> Vec<Line<'static>> {
    let t = &app.theme;
    let mut out = vec![Line::from(Span::styled(" Waiting to sync", t.help_title))];
    if state.pending.is_empty() {
        out.push(Line::from(Span::styled(
            "   every paired device is up to date",
            t.dim,
        )));
    }
    for (peer, n) in &state.pending {
        let what = if *n == 0 {
            "up to date".to_string()
        } else {
            format!("{n} pending")
        };
        out.push(Line::from(format!("   device …{}  {what}", short(peer))));
    }
    out.push(Line::from(""));
    out.push(Line::from(Span::styled(" Merged edits", t.help_title)));
    if state.merged.is_empty() {
        out.push(Line::from(Span::styled(
            "   no concurrent edits were merged",
            t.dim,
        )));
    }
    for (i, e) in state.merged.iter().enumerate() {
        let head = format!(
            " {} {}  from …{}",
            if i == state.selected { ">" } else { " " },
            e.page_title,
            short(&e.peer_actor)
        );
        let style = if i == state.selected {
            t.help_title
        } else {
            t.dim
        };
        out.push(Line::from(Span::styled(head, style)));
        out.push(Line::from(format!("     yours:  {}", e.ours)));
        out.push(Line::from(format!("     merged: {}", e.merged)));
    }
    out
}

pub(crate) fn render_sync_panel(
    f: &mut ratatui::Frame<'_>,
    full: Rect,
    app: &App,
    state: &SyncPanelState,
) {
    let area = centered_rect(full, 76, 70);
    f.render_widget(Clear, area);
    // Each merged entry is three lines plus the pending header.
    let top = (state.pending.len().max(1) + 3 + state.selected * 3) as u16;
    let scroll = top.saturating_sub(area.height.saturating_sub(6));
    let body = Paragraph::new(lines(app, state))
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0))
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(app.theme.border)
                .title(Span::styled(
                    " Sync · j/k move · u undo merge · Enter open · Esc close ",
                    app.theme.help_title,
                ))
                .style(app.theme.popup_style()),
        );
    f.render_widget(body, area);
}
