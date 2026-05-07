//! Rich widget components for the Fuse TUI.
//!
//! Provides GUI-like widgets: sidebar with tabs, title bar, chat area with
//! scrollbar, input area with cursor, status bar with FPS, command palette
//! overlay, and help modal.

use crate::tui::app::{AppState, SidebarTab, UiMode};
use crate::tui::chat::Role;
use crate::tui::render;

use ratatui::prelude::*;
use ratatui::widgets::*;

/// Draw the sidebar with navigation tabs.
pub fn draw_sidebar(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(state.theme.border_style())
        .style(state.theme.sidebar_style());

    let inner = block.inner(area);
    f.render_widget(block, area);

    // Tab items
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Logo
            Constraint::Length(1), // Spacer
            Constraint::Min(4),    // Tabs
            Constraint::Length(1), // Bottom
        ])
        .split(inner);

    // Logo
    let logo = Paragraph::new("  ⚡ Fuse").style(
        Style::default()
            .fg(state.theme.accent)
            .add_modifier(Modifier::BOLD),
    );
    f.render_widget(logo, chunks[0]);

    // Navigation tabs
    let mut tab_lines: Vec<Line> = Vec::new();
    for tab in SidebarTab::all() {
        let is_active = *tab == state.sidebar_tab;
        let indicator = if is_active { "▸ " } else { "  " };
        let style = if is_active {
            state.theme.accent_style().add_modifier(Modifier::BOLD)
        } else {
            state.theme.dim_style()
        };
        tab_lines.push(Line::from(Span::styled(
            format!("{indicator}{}", tab.label()),
            style,
        )));
    }
    let tabs_widget = Paragraph::new(tab_lines);
    f.render_widget(tabs_widget, chunks[2]);
}

/// Draw the title bar.
pub fn draw_title_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let mode_indicator = match state.mode {
        UiMode::Chat => "CHAT",
        UiMode::CommandPalette => "CMD",
        UiMode::Help => "HELP",
        UiMode::Scroll => "SCROLL",
    };

    let title = Line::from(vec![
        Span::styled(
            format!(" {} ", state.session.model),
            Style::default()
                .fg(state.theme.accent)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" │ ", state.theme.muted_style()),
        Span::styled(
            format!(" {mode_indicator} "),
            if state.mode == UiMode::Chat {
                state.theme.dim_style()
            } else {
                Style::default()
                    .fg(state.theme.warning_color)
                    .add_modifier(Modifier::BOLD)
            },
        ),
    ]);

    let bar = Paragraph::new(title).style(state.theme.title_style());
    f.render_widget(bar, area);
}

/// Draw the chat message area with scrollbar.
pub fn draw_chat_area(f: &mut Frame, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(if state.mode == UiMode::Scroll {
            state.theme.border_focused_style()
        } else {
            state.theme.border_style()
        })
        .title(" Messages ");

    let inner = block.inner(area);
    f.render_widget(block, area);

    let chat_width = inner.width.saturating_sub(2) as usize;
    let visible_height = inner.height as usize;

    let mut lines: Vec<Line> = Vec::new();

    for msg in &state.session.messages {
        let (prefix, style) = match msg.role {
            Role::User => ("▶ You", state.theme.user_style()),
            Role::Assistant => ("◆ AI ", state.theme.assistant_style()),
            Role::System => ("● Sys", state.theme.system_style()),
        };

        // Timestamp
        let time_str = if state.show_timestamps {
            format!(" {}", msg.timestamp.format("%H:%M"))
        } else {
            String::new()
        };

        // Token count
        let token_str = if state.show_token_count {
            msg.tokens.map(|t| format!(" [{t}t]")).unwrap_or_default()
        } else {
            String::new()
        };

        // Header line
        lines.push(Line::from(vec![
            Span::styled(prefix.to_string(), style.add_modifier(Modifier::BOLD)),
            Span::styled(time_str, state.theme.muted_style()),
            Span::styled(token_str, state.theme.muted_style()),
        ]));

        // Content with markdown rendering and word wrap
        let rendered = render::render_markdown(&msg.content);
        let wrapped = render::word_wrap(&rendered, chat_width.saturating_sub(2));
        for line in &wrapped {
            lines.push(Line::from(Span::styled(format!("  {line}"), style)));
        }

        // Blank separator
        lines.push(Line::from(""));
    }

    // Streaming indicator
    if state.session.is_streaming {
        lines.push(Line::from(vec![
            Span::styled(
                "◆ AI ",
                state.theme.streaming_style().add_modifier(Modifier::BOLD),
            ),
            Span::styled(" streaming...", state.theme.muted_style()),
        ]));
        lines.push(Line::from(Span::styled(
            format!("  {}▌", state.session.streaming_buffer),
            state.theme.streaming_style(),
        )));
    }

    // Calculate scroll
    let total_lines = lines.len();
    let skip = if total_lines > visible_height {
        total_lines
            .saturating_sub(visible_height)
            .saturating_sub(state.scroll_offset as usize)
    } else {
        0
    };

    let visible_lines: Vec<Line> = lines.into_iter().skip(skip).take(visible_height).collect();

    let paragraph = Paragraph::new(visible_lines);
    f.render_widget(paragraph, inner);

    // Scrollbar (right side)
    if total_lines > visible_height {
        let scrollbar = Scrollbar::default()
            .orientation(ScrollbarOrientation::VerticalRight)
            .begin_symbol(Some("↑"))
            .end_symbol(Some("↓"));

        let mut scrollbar_state = ScrollbarState::new(total_lines).position(skip);
        f.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
    }
}

/// Draw the input area with cursor.
pub fn draw_input_area(f: &mut Frame, area: Rect, state: &AppState) {
    let is_focused = state.mode == UiMode::Chat;
    let border_style = if is_focused {
        state.theme.border_focused_style()
    } else {
        state.theme.border_style()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(border_style)
        .title(if is_focused {
            " Input (/ commands, ? help) "
        } else {
            " Input "
        });

    let paragraph = Paragraph::new(state.session.input.as_str())
        .block(block)
        .style(Style::default().fg(state.theme.fg));
    f.render_widget(paragraph, area);

    // Show cursor only when in chat mode
    if is_focused {
        let cursor_x = area.x + 1 + state.session.input.len() as u16;
        let cursor_y = area.y + 1;
        f.set_cursor_position((cursor_x.min(area.x + area.width - 2), cursor_y));
    }
}

/// Draw the status bar with metrics.
pub fn draw_status_bar(f: &mut Frame, area: Rect, state: &AppState) {
    let left = format!(
        " {} msgs │ {} │ {}",
        state.session.message_count(),
        state.session.model,
        if state.session.is_streaming {
            "streaming..."
        } else {
            "ready"
        },
    );

    let right = format!("{:.0} FPS │ Ctrl+C quit ", state.fps,);

    // Pad to fill width
    let padding = area
        .width
        .saturating_sub(left.len() as u16 + right.len() as u16);

    let bar = Line::from(vec![
        Span::styled(&left, state.theme.status_style()),
        Span::styled(" ".repeat(padding as usize), state.theme.status_style()),
        Span::styled(&right, state.theme.status_style()),
    ]);

    let paragraph = Paragraph::new(bar).style(state.theme.status_style());
    f.render_widget(paragraph, area);
}

/// Draw the command palette overlay (centered modal).
pub fn draw_command_palette(f: &mut Frame, area: Rect, state: &AppState) {
    let width = 50.min(area.width.saturating_sub(4));
    let height = 12.min(area.height.saturating_sub(4));

    let popup_area = centered_rect(width, height, area);

    // Clear background
    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(state.theme.border_focused_style())
        .title(" Commands (Esc to close) ")
        .style(Style::default().bg(state.theme.bg));

    let inner = block.inner(popup_area);
    f.render_widget(block, popup_area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(1), Constraint::Min(1)])
        .split(inner);

    // Search input
    let search =
        Paragraph::new(format!("/{}", state.command_input)).style(state.theme.accent_style());
    f.render_widget(search, chunks[0]);

    // Filtered commands
    let items: Vec<Line> = state
        .command_matches
        .iter()
        .take(chunks[1].height as usize)
        .enumerate()
        .map(|(i, cmd)| {
            let style = if i == 0 {
                state.theme.selected_style()
            } else {
                state.theme.dim_style()
            };
            Line::from(Span::styled(format!("  {cmd}"), style))
        })
        .collect();

    let list = Paragraph::new(items);
    f.render_widget(list, chunks[1]);

    // Cursor in search
    let cursor_x = popup_area.x + 2 + state.command_input.len() as u16;
    let cursor_y = popup_area.y + 1;
    f.set_cursor_position((cursor_x, cursor_y));
}

/// Draw the help overlay.
pub fn draw_help_overlay(f: &mut Frame, area: Rect, state: &AppState) {
    let width = 60.min(area.width.saturating_sub(4));
    let height = 20.min(area.height.saturating_sub(4));
    let popup_area = centered_rect(width, height, area);

    f.render_widget(Clear, popup_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(state.theme.border_focused_style())
        .title(" Help (Esc to close) ")
        .style(Style::default().bg(state.theme.bg));

    let help_text = vec![
        Line::from(Span::styled(
            "  Fuse Terminal UI — Keyboard Shortcuts",
            Style::default()
                .fg(state.theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from("  Enter          Send message"),
        Line::from("  /              Open command palette"),
        Line::from("  ?              Toggle this help"),
        Line::from("  Ctrl+C         Quit"),
        Line::from("  Ctrl+B         Toggle sidebar"),
        Line::from("  Ctrl+L         Clear chat"),
        Line::from("  Tab / Shift+Tab  Cycle sidebar tabs"),
        Line::from("  ↑/↓ PgUp/PgDn Scroll messages"),
        Line::from("  Home/End       Scroll to top/bottom"),
        Line::from("  Mouse wheel    Scroll messages"),
        Line::from(""),
        Line::from(Span::styled(
            "  Commands",
            Style::default()
                .fg(state.theme.accent)
                .add_modifier(Modifier::BOLD),
        )),
        Line::from("  /quit  /clear  /theme  /sidebar"),
        Line::from("  /model /timestamps  /tokens  /help"),
    ];

    let paragraph = Paragraph::new(help_text).block(block);
    f.render_widget(paragraph, popup_area);
}

/// Create a centered rectangle within `area`.
fn centered_rect(width: u16, height: u16, area: Rect) -> Rect {
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    Rect::new(x, y, width.min(area.width), height.min(area.height))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_centered_rect() {
        let area = Rect::new(0, 0, 100, 50);
        let popup = centered_rect(40, 20, area);
        assert_eq!(popup.x, 30);
        assert_eq!(popup.y, 15);
        assert_eq!(popup.width, 40);
        assert_eq!(popup.height, 20);
    }

    #[test]
    fn test_centered_rect_too_large() {
        let area = Rect::new(0, 0, 20, 10);
        let popup = centered_rect(40, 20, area);
        // Should clamp to area size
        assert!(popup.width <= area.width);
        assert!(popup.height <= area.height);
    }

    #[test]
    fn test_centered_rect_small_area() {
        let area = Rect::new(5, 5, 10, 10);
        let popup = centered_rect(6, 6, area);
        assert_eq!(popup.x, 7); // 5 + (10-6)/2
        assert_eq!(popup.y, 7);
    }
}
