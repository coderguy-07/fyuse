//! TUI application — high-performance interactive terminal UI powered by ratatui.
//!
//! Architecture:
//! - 120Hz render loop with diff-based rendering (ratatui double-buffer)
//! - Async event handling with non-blocking input
//! - State machine for UI modes (Chat, ModelSelect, Help, Command)
//! - Responsive layout with sidebar, tabs, status bar, scrollbar

use crate::config::FuseConfig;
use crate::error::{FuseError, Result};
use crate::tui::chat::{ChatMessage, ChatSession};
use crate::tui::theme::Theme;
use crate::tui::widgets::{
    draw_chat_area, draw_command_palette, draw_help_overlay, draw_input_area, draw_sidebar,
    draw_status_bar, draw_title_bar,
};

use crossterm::event::{
    self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    MouseEvent, MouseEventKind,
};
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use crossterm::ExecutableCommand;
use ratatui::prelude::*;
use std::io;
use std::time::{Duration, Instant};

/// Target frames per second for the render loop.
const TARGET_FPS: f64 = 120.0;
/// Frame budget in duration.
const FRAME_BUDGET: Duration = Duration::from_micros((1_000_000.0 / TARGET_FPS) as u64);

/// UI mode state machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    /// Normal chat interaction.
    Chat,
    /// Command palette overlay (triggered by /).
    CommandPalette,
    /// Help overlay (triggered by ?).
    Help,
    /// Scrolling through messages (triggered by arrow keys).
    Scroll,
}

/// Active sidebar tab.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarTab {
    Chat,
    Models,
    Sessions,
    Settings,
}

impl SidebarTab {
    pub fn label(&self) -> &'static str {
        match self {
            Self::Chat => "Chat",
            Self::Models => "Models",
            Self::Sessions => "Sessions",
            Self::Settings => "Settings",
        }
    }

    pub fn all() -> &'static [SidebarTab] {
        &[
            SidebarTab::Chat,
            SidebarTab::Models,
            SidebarTab::Sessions,
            SidebarTab::Settings,
        ]
    }

    pub fn next(&self) -> Self {
        match self {
            Self::Chat => Self::Models,
            Self::Models => Self::Sessions,
            Self::Sessions => Self::Settings,
            Self::Settings => Self::Chat,
        }
    }

    pub fn prev(&self) -> Self {
        match self {
            Self::Chat => Self::Settings,
            Self::Models => Self::Chat,
            Self::Sessions => Self::Models,
            Self::Settings => Self::Sessions,
        }
    }
}

/// Full application state.
pub struct AppState {
    pub session: ChatSession,
    pub mode: UiMode,
    pub sidebar_tab: SidebarTab,
    pub sidebar_visible: bool,
    pub scroll_offset: u16,
    pub command_input: String,
    pub command_matches: Vec<String>,
    pub theme: Theme,
    pub frame_count: u64,
    pub fps: f64,
    pub should_quit: bool,
    pub show_timestamps: bool,
    pub show_token_count: bool,
    last_frame_time: Instant,
    fps_samples: Vec<f64>,
}

impl AppState {
    pub fn new(model: &str, theme: Theme) -> Self {
        Self {
            session: ChatSession::new(model),
            mode: UiMode::Chat,
            sidebar_tab: SidebarTab::Chat,
            sidebar_visible: true,
            scroll_offset: 0,
            command_input: String::new(),
            command_matches: Vec::new(),
            theme,
            frame_count: 0,
            fps: 0.0,
            should_quit: false,
            show_timestamps: false,
            show_token_count: true,
            last_frame_time: Instant::now(),
            fps_samples: Vec::with_capacity(60),
        }
    }

    /// Update FPS counter.
    fn update_fps(&mut self) {
        let now = Instant::now();
        let elapsed = now.duration_since(self.last_frame_time);
        self.last_frame_time = now;

        if elapsed.as_secs_f64() > 0.0 {
            let sample = 1.0 / elapsed.as_secs_f64();
            self.fps_samples.push(sample);
            if self.fps_samples.len() > 60 {
                self.fps_samples.remove(0);
            }
            self.fps = self.fps_samples.iter().sum::<f64>() / self.fps_samples.len() as f64;
        }
        self.frame_count += 1;
    }

    /// Available commands for the command palette.
    fn available_commands() -> Vec<(&'static str, &'static str)> {
        vec![
            ("/quit", "Exit Fuse"),
            ("/clear", "Clear chat history"),
            ("/model", "Switch model"),
            ("/theme", "Toggle dark/light theme"),
            ("/sidebar", "Toggle sidebar"),
            ("/timestamps", "Toggle timestamp display"),
            ("/tokens", "Toggle token count display"),
            ("/help", "Show help"),
            ("/export", "Export conversation"),
            ("/system", "Set system prompt"),
        ]
    }

    /// Filter commands matching current input.
    fn update_command_matches(&mut self) {
        let query = self.command_input.to_lowercase();
        self.command_matches = Self::available_commands()
            .iter()
            .filter(|(cmd, desc)| cmd.contains(&query) || desc.to_lowercase().contains(&query))
            .map(|(cmd, desc)| format!("{cmd}  —  {desc}"))
            .collect();
    }
}

/// Run the interactive chat TUI.
pub async fn run_tui(model: &str, _config: &FuseConfig) -> Result<()> {
    // Set up terminal with mouse capture
    enable_raw_mode().map_err(|e| FuseError::InternalError(format!("Raw mode: {e}")))?;
    let mut stdout = io::stdout();
    stdout
        .execute(EnterAlternateScreen)
        .map_err(|e| FuseError::InternalError(format!("Alt screen: {e}")))?;
    stdout
        .execute(EnableMouseCapture)
        .map_err(|e| FuseError::InternalError(format!("Mouse capture: {e}")))?;

    let backend = CrosstermBackend::new(stdout);
    let mut terminal =
        Terminal::new(backend).map_err(|e| FuseError::InternalError(format!("Terminal: {e}")))?;

    let theme = Theme::dark();
    let mut state = AppState::new(model, theme);

    state.session.add_message(ChatMessage::system(format!(
        "Welcome to Fuse. Model: {model}. Press ? for help, / for commands."
    )));

    let result = run_event_loop(&mut terminal, &mut state).await;

    // Restore terminal
    disable_raw_mode().ok();
    terminal.backend_mut().execute(DisableMouseCapture).ok();
    terminal.backend_mut().execute(LeaveAlternateScreen).ok();
    terminal.show_cursor().ok();

    result
}

/// High-performance 120Hz event loop.
async fn run_event_loop<B: Backend>(
    terminal: &mut Terminal<B>,
    state: &mut AppState,
) -> Result<()> {
    loop {
        if state.should_quit {
            return Ok(());
        }

        // Render frame
        state.update_fps();
        terminal
            .draw(|f| draw_frame(f, state))
            .map_err(|e| FuseError::InternalError(format!("Draw: {e}")))?;

        // Poll events within frame budget (non-blocking for 120Hz)
        let poll_timeout = FRAME_BUDGET;
        if event::poll(poll_timeout).map_err(|e| FuseError::InternalError(format!("Poll: {e}")))? {
            let evt = event::read().map_err(|e| FuseError::InternalError(format!("Read: {e}")))?;
            match evt {
                Event::Key(key) => handle_key_event(state, key),
                Event::Mouse(mouse) => handle_mouse_event(state, mouse),
                Event::Resize(_, _) => {} // ratatui handles resize automatically
                _ => {}
            }
        }
    }
}

/// Route key events to the appropriate handler based on UI mode.
fn handle_key_event(state: &mut AppState, key: KeyEvent) {
    // Global: Ctrl+C / Ctrl+D always quits
    if key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char('c') | KeyCode::Char('d'))
    {
        state.should_quit = true;
        return;
    }

    match state.mode {
        UiMode::Chat => handle_chat_key(state, key),
        UiMode::CommandPalette => handle_command_key(state, key),
        UiMode::Help => handle_help_key(state, key),
        UiMode::Scroll => handle_scroll_key(state, key),
    }
}

fn handle_chat_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Enter => {
            if !state.session.input.is_empty() {
                let input = state.session.input.clone();
                state.session.input.clear();

                // Handle slash commands inline
                if input.starts_with('/') {
                    execute_command(state, &input);
                    return;
                }

                // Add user message and generate response
                state.session.add_message(ChatMessage::user(&input));
                state.session.start_streaming();
                state
                    .session
                    .append_token("[inference not yet wired — connect InferenceCoordinator]");
                state.session.finish_streaming();
                state.scroll_offset = 0;
            }
        }
        KeyCode::Backspace => {
            state.session.input.pop();
        }
        KeyCode::Char('/') if state.session.input.is_empty() => {
            state.mode = UiMode::CommandPalette;
            state.command_input.clear();
            state.update_command_matches();
        }
        KeyCode::Char('?') if state.session.input.is_empty() => {
            state.mode = UiMode::Help;
        }
        KeyCode::Up | KeyCode::PageUp => {
            state.mode = UiMode::Scroll;
            state.scroll_offset = state.scroll_offset.saturating_add(3);
        }
        KeyCode::Down | KeyCode::PageDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(3);
        }
        KeyCode::Tab => {
            state.sidebar_tab = state.sidebar_tab.next();
        }
        KeyCode::BackTab => {
            state.sidebar_tab = state.sidebar_tab.prev();
        }
        // Ctrl+B toggle sidebar
        KeyCode::Char('b') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.sidebar_visible = !state.sidebar_visible;
        }
        // Ctrl+L clear
        KeyCode::Char('l') if key.modifiers.contains(KeyModifiers::CONTROL) => {
            state.session.clear();
            state.scroll_offset = 0;
        }
        KeyCode::Home => {
            state.scroll_offset = u16::MAX; // Scroll to top
        }
        KeyCode::End => {
            state.scroll_offset = 0; // Scroll to bottom
        }
        KeyCode::Char(c) => {
            state.session.input.push(c);
        }
        _ => {}
    }
}

fn handle_command_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            state.mode = UiMode::Chat;
            state.command_input.clear();
        }
        KeyCode::Enter => {
            let cmd = if state.command_input.is_empty() {
                // Select first match
                if let Some(first) = state.command_matches.first() {
                    first.split("  —").next().unwrap_or("").trim().to_string()
                } else {
                    String::new()
                }
            } else {
                format!("/{}", state.command_input)
            };
            state.mode = UiMode::Chat;
            if !cmd.is_empty() {
                execute_command(state, &cmd);
            }
            state.command_input.clear();
        }
        KeyCode::Backspace => {
            state.command_input.pop();
            state.update_command_matches();
        }
        KeyCode::Char(c) => {
            state.command_input.push(c);
            state.update_command_matches();
        }
        _ => {}
    }
}

fn handle_help_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') => {
            state.mode = UiMode::Chat;
        }
        _ => {}
    }
}

fn handle_scroll_key(state: &mut AppState, key: KeyEvent) {
    match key.code {
        KeyCode::Up | KeyCode::PageUp => {
            state.scroll_offset = state.scroll_offset.saturating_add(3);
        }
        KeyCode::Down | KeyCode::PageDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(3);
            if state.scroll_offset == 0 {
                state.mode = UiMode::Chat;
            }
        }
        KeyCode::Home => {
            state.scroll_offset = u16::MAX;
        }
        KeyCode::End => {
            state.scroll_offset = 0;
            state.mode = UiMode::Chat;
        }
        KeyCode::Esc => {
            state.scroll_offset = 0;
            state.mode = UiMode::Chat;
        }
        // Any typing returns to chat mode
        KeyCode::Char(_) | KeyCode::Enter => {
            state.mode = UiMode::Chat;
            handle_chat_key(state, key);
        }
        _ => {}
    }
}

fn handle_mouse_event(state: &mut AppState, mouse: MouseEvent) {
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            state.scroll_offset = state.scroll_offset.saturating_add(3);
            if state.mode == UiMode::Chat {
                state.mode = UiMode::Scroll;
            }
        }
        MouseEventKind::ScrollDown => {
            state.scroll_offset = state.scroll_offset.saturating_sub(3);
            if state.scroll_offset == 0 && state.mode == UiMode::Scroll {
                state.mode = UiMode::Chat;
            }
        }
        _ => {}
    }
}

/// Execute a slash command.
fn execute_command(state: &mut AppState, cmd: &str) {
    match cmd {
        "/quit" | "/exit" => state.should_quit = true,
        "/clear" => {
            state.session.clear();
            state.scroll_offset = 0;
        }
        "/help" => state.mode = UiMode::Help,
        "/theme" => state.theme = state.theme.toggle(),
        "/sidebar" => state.sidebar_visible = !state.sidebar_visible,
        "/timestamps" => state.show_timestamps = !state.show_timestamps,
        "/tokens" => state.show_token_count = !state.show_token_count,
        _ => {
            state
                .session
                .add_message(ChatMessage::system(format!("Unknown command: {cmd}")));
        }
    }
}

/// Main frame rendering — composes all widgets.
fn draw_frame(f: &mut Frame, state: &AppState) {
    let area = f.area();

    // Main layout: optional sidebar | content
    let main_chunks = if state.sidebar_visible && area.width > 40 {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(20), Constraint::Min(30)])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Length(0), Constraint::Min(1)])
            .split(area)
    };

    // Draw sidebar
    if state.sidebar_visible && area.width > 40 {
        draw_sidebar(f, main_chunks[0], state);
    }

    // Content area: title | chat | input | status
    let content_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // Title bar
            Constraint::Min(5),    // Chat area
            Constraint::Length(3), // Input area
            Constraint::Length(1), // Status bar
        ])
        .split(main_chunks[1]);

    draw_title_bar(f, content_chunks[0], state);
    draw_chat_area(f, content_chunks[1], state);
    draw_input_area(f, content_chunks[2], state);
    draw_status_bar(f, content_chunks[3], state);

    // Overlays (rendered last, on top of everything)
    match state.mode {
        UiMode::CommandPalette => draw_command_palette(f, area, state),
        UiMode::Help => draw_help_overlay(f, area, state),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_app_state_new() {
        let state = AppState::new("llama3", Theme::dark());
        assert_eq!(state.mode, UiMode::Chat);
        assert_eq!(state.sidebar_tab, SidebarTab::Chat);
        assert!(state.sidebar_visible);
        assert_eq!(state.scroll_offset, 0);
        assert!(!state.should_quit);
    }

    #[test]
    fn test_sidebar_tab_cycle() {
        let tab = SidebarTab::Chat;
        assert_eq!(tab.next(), SidebarTab::Models);
        assert_eq!(tab.next().next(), SidebarTab::Sessions);
        assert_eq!(tab.next().next().next(), SidebarTab::Settings);
        assert_eq!(tab.next().next().next().next(), SidebarTab::Chat);
    }

    #[test]
    fn test_sidebar_tab_prev() {
        let tab = SidebarTab::Chat;
        assert_eq!(tab.prev(), SidebarTab::Settings);
        assert_eq!(SidebarTab::Models.prev(), SidebarTab::Chat);
    }

    #[test]
    fn test_execute_command_quit() {
        let mut state = AppState::new("test", Theme::dark());
        execute_command(&mut state, "/quit");
        assert!(state.should_quit);
    }

    #[test]
    fn test_execute_command_clear() {
        let mut state = AppState::new("test", Theme::dark());
        state.session.add_message(ChatMessage::user("hello"));
        execute_command(&mut state, "/clear");
        assert_eq!(state.session.message_count(), 0);
    }

    #[test]
    fn test_execute_command_theme_toggle() {
        let mut state = AppState::new("test", Theme::dark());
        assert!(state.theme.is_dark);
        execute_command(&mut state, "/theme");
        assert!(!state.theme.is_dark);
        execute_command(&mut state, "/theme");
        assert!(state.theme.is_dark);
    }

    #[test]
    fn test_execute_command_sidebar_toggle() {
        let mut state = AppState::new("test", Theme::dark());
        assert!(state.sidebar_visible);
        execute_command(&mut state, "/sidebar");
        assert!(!state.sidebar_visible);
    }

    #[test]
    fn test_execute_command_unknown() {
        let mut state = AppState::new("test", Theme::dark());
        execute_command(&mut state, "/nonexistent");
        assert_eq!(state.session.message_count(), 1);
        assert!(state
            .session
            .messages
            .back()
            .unwrap()
            .content
            .contains("Unknown command"));
    }

    #[test]
    fn test_handle_key_quit() {
        let mut state = AppState::new("test", Theme::dark());
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL),
        );
        assert!(state.should_quit);
    }

    #[test]
    fn test_handle_key_enter_sends_message() {
        let mut state = AppState::new("test", Theme::dark());
        state.session.input = "hello".to_string();
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE),
        );
        assert!(state.session.input.is_empty());
        assert!(state.session.message_count() >= 2); // user + assistant
    }

    #[test]
    fn test_handle_key_backspace() {
        let mut state = AppState::new("test", Theme::dark());
        state.session.input = "hello".to_string();
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Backspace, KeyModifiers::NONE),
        );
        assert_eq!(state.session.input, "hell");
    }

    #[test]
    fn test_handle_key_slash_opens_command_palette() {
        let mut state = AppState::new("test", Theme::dark());
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('/'), KeyModifiers::NONE),
        );
        assert_eq!(state.mode, UiMode::CommandPalette);
    }

    #[test]
    fn test_handle_key_question_opens_help() {
        let mut state = AppState::new("test", Theme::dark());
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('?'), KeyModifiers::NONE),
        );
        assert_eq!(state.mode, UiMode::Help);
    }

    #[test]
    fn test_handle_scroll_up_enters_scroll_mode() {
        let mut state = AppState::new("test", Theme::dark());
        handle_key_event(&mut state, KeyEvent::new(KeyCode::Up, KeyModifiers::NONE));
        assert_eq!(state.mode, UiMode::Scroll);
        assert_eq!(state.scroll_offset, 3);
    }

    #[test]
    fn test_command_palette_escape() {
        let mut state = AppState::new("test", Theme::dark());
        state.mode = UiMode::CommandPalette;
        handle_key_event(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(state.mode, UiMode::Chat);
    }

    #[test]
    fn test_help_escape() {
        let mut state = AppState::new("test", Theme::dark());
        state.mode = UiMode::Help;
        handle_key_event(&mut state, KeyEvent::new(KeyCode::Esc, KeyModifiers::NONE));
        assert_eq!(state.mode, UiMode::Chat);
    }

    #[test]
    fn test_scroll_end_returns_to_chat() {
        let mut state = AppState::new("test", Theme::dark());
        state.mode = UiMode::Scroll;
        state.scroll_offset = 10;
        handle_key_event(&mut state, KeyEvent::new(KeyCode::End, KeyModifiers::NONE));
        assert_eq!(state.mode, UiMode::Chat);
        assert_eq!(state.scroll_offset, 0);
    }

    #[test]
    fn test_command_matches_filter() {
        let mut state = AppState::new("test", Theme::dark());
        state.command_input = "cl".to_string();
        state.update_command_matches();
        assert!(state.command_matches.iter().any(|m| m.contains("/clear")));
    }

    #[test]
    fn test_fps_update() {
        let mut state = AppState::new("test", Theme::dark());
        state.update_fps();
        state.update_fps();
        assert!(state.frame_count >= 2);
    }

    #[test]
    fn test_ctrl_b_toggles_sidebar() {
        let mut state = AppState::new("test", Theme::dark());
        assert!(state.sidebar_visible);
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('b'), KeyModifiers::CONTROL),
        );
        assert!(!state.sidebar_visible);
    }

    #[test]
    fn test_ctrl_l_clears() {
        let mut state = AppState::new("test", Theme::dark());
        state.session.add_message(ChatMessage::user("hello"));
        handle_key_event(
            &mut state,
            KeyEvent::new(KeyCode::Char('l'), KeyModifiers::CONTROL),
        );
        assert_eq!(state.session.message_count(), 0);
    }

    #[test]
    fn test_tab_cycles_sidebar_tabs() {
        let mut state = AppState::new("test", Theme::dark());
        assert_eq!(state.sidebar_tab, SidebarTab::Chat);
        handle_key_event(&mut state, KeyEvent::new(KeyCode::Tab, KeyModifiers::NONE));
        assert_eq!(state.sidebar_tab, SidebarTab::Models);
    }

    #[test]
    fn test_mouse_scroll() {
        let mut state = AppState::new("test", Theme::dark());
        handle_mouse_event(
            &mut state,
            MouseEvent {
                kind: MouseEventKind::ScrollUp,
                column: 0,
                row: 0,
                modifiers: KeyModifiers::NONE,
            },
        );
        assert_eq!(state.scroll_offset, 3);
        assert_eq!(state.mode, UiMode::Scroll);
    }

    #[test]
    fn test_home_scrolls_to_top() {
        let mut state = AppState::new("test", Theme::dark());
        handle_key_event(&mut state, KeyEvent::new(KeyCode::Home, KeyModifiers::NONE));
        assert_eq!(state.scroll_offset, u16::MAX);
    }

    #[test]
    fn test_frame_budget() {
        // 120 FPS = ~8.3ms per frame
        assert!(FRAME_BUDGET.as_micros() > 8000);
        assert!(FRAME_BUDGET.as_micros() < 9000);
    }
}
