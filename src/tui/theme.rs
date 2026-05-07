//! Theme system for the TUI — dark/light with configurable colors.

use ratatui::style::{Color, Modifier, Style};

/// Color theme for the TUI.
#[derive(Debug, Clone)]
pub struct Theme {
    pub is_dark: bool,
    // Base
    pub bg: Color,
    pub fg: Color,
    pub fg_dim: Color,
    pub fg_muted: Color,
    // Accents
    pub accent: Color,
    pub accent_dim: Color,
    // Roles
    pub user_color: Color,
    pub assistant_color: Color,
    pub system_color: Color,
    // UI elements
    pub border: Color,
    pub border_focused: Color,
    pub selection_bg: Color,
    pub status_bg: Color,
    pub status_fg: Color,
    pub sidebar_bg: Color,
    pub title_bg: Color,
    pub title_fg: Color,
    // Code
    pub code_bg: Color,
    pub code_fg: Color,
    // Streaming
    pub streaming_color: Color,
    // Error / warning
    pub error_color: Color,
    pub warning_color: Color,
    pub success_color: Color,
}

impl Theme {
    /// Dark theme — default.
    pub fn dark() -> Self {
        Self {
            is_dark: true,
            bg: Color::Reset,
            fg: Color::White,
            fg_dim: Color::Gray,
            fg_muted: Color::DarkGray,
            accent: Color::Cyan,
            accent_dim: Color::Blue,
            user_color: Color::Cyan,
            assistant_color: Color::Green,
            system_color: Color::DarkGray,
            border: Color::DarkGray,
            border_focused: Color::Cyan,
            selection_bg: Color::Rgb(40, 40, 60),
            status_bg: Color::Rgb(30, 30, 40),
            status_fg: Color::Gray,
            sidebar_bg: Color::Rgb(20, 20, 30),
            title_bg: Color::Rgb(25, 25, 45),
            title_fg: Color::White,
            code_bg: Color::Rgb(30, 30, 40),
            code_fg: Color::Rgb(180, 220, 180),
            streaming_color: Color::Yellow,
            error_color: Color::Red,
            warning_color: Color::Yellow,
            success_color: Color::Green,
        }
    }

    /// Light theme.
    pub fn light() -> Self {
        Self {
            is_dark: false,
            bg: Color::White,
            fg: Color::Black,
            fg_dim: Color::DarkGray,
            fg_muted: Color::Gray,
            accent: Color::Blue,
            accent_dim: Color::Cyan,
            user_color: Color::Blue,
            assistant_color: Color::Rgb(0, 128, 0),
            system_color: Color::Gray,
            border: Color::Gray,
            border_focused: Color::Blue,
            selection_bg: Color::Rgb(220, 220, 240),
            status_bg: Color::Rgb(230, 230, 240),
            status_fg: Color::DarkGray,
            sidebar_bg: Color::Rgb(240, 240, 245),
            title_bg: Color::Rgb(220, 220, 240),
            title_fg: Color::Black,
            code_bg: Color::Rgb(240, 240, 240),
            code_fg: Color::Rgb(60, 100, 60),
            streaming_color: Color::Rgb(200, 150, 0),
            error_color: Color::Red,
            warning_color: Color::Rgb(200, 150, 0),
            success_color: Color::Rgb(0, 128, 0),
        }
    }

    /// Toggle between dark and light.
    pub fn toggle(&self) -> Self {
        if self.is_dark {
            Self::light()
        } else {
            Self::dark()
        }
    }

    // Style helpers

    pub fn base_style(&self) -> Style {
        Style::default().fg(self.fg).bg(self.bg)
    }

    pub fn dim_style(&self) -> Style {
        Style::default().fg(self.fg_dim)
    }

    pub fn muted_style(&self) -> Style {
        Style::default().fg(self.fg_muted)
    }

    pub fn accent_style(&self) -> Style {
        Style::default().fg(self.accent)
    }

    pub fn border_style(&self) -> Style {
        Style::default().fg(self.border)
    }

    pub fn border_focused_style(&self) -> Style {
        Style::default().fg(self.border_focused)
    }

    pub fn title_style(&self) -> Style {
        Style::default().fg(self.title_fg).bg(self.title_bg)
    }

    pub fn status_style(&self) -> Style {
        Style::default().fg(self.status_fg).bg(self.status_bg)
    }

    pub fn sidebar_style(&self) -> Style {
        Style::default().bg(self.sidebar_bg).fg(self.fg)
    }

    pub fn user_style(&self) -> Style {
        Style::default().fg(self.user_color)
    }

    pub fn assistant_style(&self) -> Style {
        Style::default().fg(self.assistant_color)
    }

    pub fn system_style(&self) -> Style {
        Style::default()
            .fg(self.system_color)
            .add_modifier(Modifier::ITALIC)
    }

    pub fn code_style(&self) -> Style {
        Style::default().fg(self.code_fg).bg(self.code_bg)
    }

    pub fn streaming_style(&self) -> Style {
        Style::default().fg(self.streaming_color)
    }

    pub fn selected_style(&self) -> Style {
        Style::default().bg(self.selection_bg).fg(self.fg)
    }

    pub fn error_style(&self) -> Style {
        Style::default()
            .fg(self.error_color)
            .add_modifier(Modifier::BOLD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dark_theme() {
        let t = Theme::dark();
        assert!(t.is_dark);
        assert_eq!(t.fg, Color::White);
    }

    #[test]
    fn test_light_theme() {
        let t = Theme::light();
        assert!(!t.is_dark);
        assert_eq!(t.fg, Color::Black);
    }

    #[test]
    fn test_toggle() {
        let dark = Theme::dark();
        let light = dark.toggle();
        assert!(!light.is_dark);
        let dark2 = light.toggle();
        assert!(dark2.is_dark);
    }

    #[test]
    fn test_style_helpers() {
        let t = Theme::dark();
        let s = t.base_style();
        assert_eq!(s.fg.unwrap(), Color::White);

        let s = t.user_style();
        assert_eq!(s.fg.unwrap(), Color::Cyan);

        let s = t.assistant_style();
        assert_eq!(s.fg.unwrap(), Color::Green);
    }
}
