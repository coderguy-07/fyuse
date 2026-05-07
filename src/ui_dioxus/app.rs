//! Root Dioxus application component with signal-based routing and theme support.

use dioxus::prelude::*;

use crate::ui_dioxus::components::nav_sidebar::NavSidebar;
use crate::ui_dioxus::pages::{channels, chat, dashboard, models};

/// Available pages in the application.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Dashboard,
    Chat,
    Models,
    Channels,
}

/// Theme variants.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Light,
    Dark,
}

impl Theme {
    /// CSS class for the theme.
    pub fn class(self) -> &'static str {
        match self {
            Theme::Light => "theme-light",
            Theme::Dark => "theme-dark",
        }
    }

    /// Toggle to the other theme.
    pub fn toggle(self) -> Self {
        match self {
            Theme::Light => Theme::Dark,
            Theme::Dark => Theme::Light,
        }
    }

    /// Background color for the theme.
    pub fn bg_color(self) -> &'static str {
        match self {
            Theme::Light => "#f9fafb",
            Theme::Dark => "#111827",
        }
    }

    /// Text color for the theme.
    pub fn text_color(self) -> &'static str {
        match self {
            Theme::Light => "#111827",
            Theme::Dark => "#f9fafb",
        }
    }

    /// Sidebar background color.
    pub fn sidebar_bg(self) -> &'static str {
        match self {
            Theme::Light => "#ffffff",
            Theme::Dark => "#1f2937",
        }
    }

    /// Border color.
    pub fn border_color(self) -> &'static str {
        match self {
            Theme::Light => "#e5e7eb",
            Theme::Dark => "#374151",
        }
    }
}

/// Root application component.
#[component]
pub fn App() -> Element {
    let mut current_page = use_signal(|| Page::Dashboard);
    let mut theme = use_signal(|| Theme::Light);

    let theme_val = theme();
    let page_val = current_page();

    let app_style = format!(
        "display: flex; height: 100vh; font-family: 'Inter', -apple-system, sans-serif; \
         background: {}; color: {};",
        theme_val.bg_color(),
        theme_val.text_color(),
    );

    rsx! {
        div {
            class: "fuse-app {theme_val.class()}",
            style: "{app_style}",

            NavSidebar {
                current_page: page_val,
                theme: theme_val,
                on_navigate: move |page: Page| {
                    current_page.set(page);
                },
                on_toggle_theme: move |_: ()| {
                    theme.set(theme_val.toggle());
                },
            }

            main {
                style: "flex: 1; overflow-y: auto; padding: 1.5rem;",

                match page_val {
                    Page::Dashboard => rsx! { dashboard::DashboardPage { theme: theme_val } },
                    Page::Chat => rsx! { chat::ChatPage { theme: theme_val } },
                    Page::Models => rsx! { models::ModelsPage { theme: theme_val } },
                    Page::Channels => rsx! { channels::ChannelsPage { theme: theme_val } },
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_page_variants() {
        assert_ne!(Page::Dashboard, Page::Chat);
        assert_ne!(Page::Models, Page::Channels);
        assert_eq!(Page::Dashboard, Page::Dashboard);
    }

    #[test]
    fn test_theme_toggle() {
        assert_eq!(Theme::Light.toggle(), Theme::Dark);
        assert_eq!(Theme::Dark.toggle(), Theme::Light);
    }

    #[test]
    fn test_theme_class() {
        assert_eq!(Theme::Light.class(), "theme-light");
        assert_eq!(Theme::Dark.class(), "theme-dark");
    }

    #[test]
    fn test_theme_colors() {
        assert_eq!(Theme::Light.bg_color(), "#f9fafb");
        assert_eq!(Theme::Dark.bg_color(), "#111827");
        assert_eq!(Theme::Light.text_color(), "#111827");
        assert_eq!(Theme::Dark.text_color(), "#f9fafb");
    }

    #[test]
    fn test_theme_sidebar_bg() {
        assert_eq!(Theme::Light.sidebar_bg(), "#ffffff");
        assert_eq!(Theme::Dark.sidebar_bg(), "#1f2937");
    }

    #[test]
    fn test_theme_border_color() {
        assert_eq!(Theme::Light.border_color(), "#e5e7eb");
        assert_eq!(Theme::Dark.border_color(), "#374151");
    }
}
