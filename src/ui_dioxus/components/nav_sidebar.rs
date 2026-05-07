//! Navigation sidebar component.

use dioxus::prelude::*;

use crate::ui_dioxus::app::{Page, Theme};

/// Props for the NavSidebar component.
#[derive(Props, Clone, PartialEq)]
pub struct NavSidebarProps {
    /// Currently active page.
    pub current_page: Page,
    /// Current theme.
    pub theme: Theme,
    /// Called when a navigation item is clicked.
    pub on_navigate: EventHandler<Page>,
    /// Called when the theme toggle is clicked.
    pub on_toggle_theme: EventHandler<()>,
}

/// Navigation item definition.
struct NavItem {
    page: Page,
    label: &'static str,
    icon: &'static str,
}

const NAV_ITEMS: &[NavItem] = &[
    NavItem {
        page: Page::Dashboard,
        label: "Dashboard",
        icon: "[D]",
    },
    NavItem {
        page: Page::Chat,
        label: "Chat",
        icon: "[C]",
    },
    NavItem {
        page: Page::Models,
        label: "Models",
        icon: "[M]",
    },
    NavItem {
        page: Page::Channels,
        label: "Channels",
        icon: "[#]",
    },
];

/// Renders the navigation sidebar with page links and theme toggle.
#[component]
pub fn NavSidebar(props: NavSidebarProps) -> Element {
    let theme = props.theme;

    let sidebar_style = format!(
        "width: 220px; background: {}; border-right: 1px solid {}; \
         display: flex; flex-direction: column; padding: 1rem 0;",
        theme.sidebar_bg(),
        theme.border_color(),
    );

    let theme_btn_label = match theme {
        Theme::Light => "Dark Mode",
        Theme::Dark => "Light Mode",
    };

    let border_color = theme.border_color();
    let header_style = format!(
        "padding: 0 1rem 1rem; border-bottom: 1px solid {border_color}; margin-bottom: 0.5rem;",
    );
    let footer_style =
        format!("padding: 0.5rem 1rem; border-top: 1px solid {border_color}; margin-top: auto;",);
    let theme_btn_style = format!(
        "width: 100%; padding: 0.5rem; border: 1px solid {border_color}; \
         border-radius: 0.375rem; background: transparent; \
         color: inherit; cursor: pointer; font-size: 0.85rem;",
    );

    rsx! {
        nav {
            class: "nav-sidebar",
            style: "{sidebar_style}",

            div {
                style: "{header_style}",

                h1 {
                    style: "font-size: 1.5rem; margin: 0; font-weight: 700;",
                    "Fuse"
                }
                p {
                    style: "font-size: 0.75rem; margin: 0.25rem 0 0; opacity: 0.6;",
                    "AI System Manager"
                }
            }

            div {
                style: "flex: 1;",

                for item in NAV_ITEMS {
                    {
                        let is_active = item.page == props.current_page;
                        let active_bg = match theme {
                            Theme::Light => "rgba(59, 130, 246, 0.1)",
                            Theme::Dark => "rgba(59, 130, 246, 0.2)",
                        };
                        let bg = if is_active { active_bg } else { "transparent" };
                        let font_weight = if is_active { "600" } else { "400" };
                        let color = if is_active { "#3b82f6" } else { theme.text_color() };
                        let style = format!(
                            "display: flex; align-items: center; gap: 0.5rem; padding: 0.6rem 1rem; \
                             cursor: pointer; background: {bg}; color: {color}; font-weight: {font_weight}; \
                             border: none; width: 100%; text-align: left; font-size: 0.9rem; \
                             font-family: inherit;",
                        );
                        let page = item.page;
                        let on_nav = props.on_navigate;
                        rsx! {
                            button {
                                class: "nav-item",
                                style: "{style}",
                                onclick: move |_| {
                                    on_nav.call(page);
                                },
                                span { "{item.icon}" }
                                span { "{item.label}" }
                            }
                        }
                    }
                }
            }

            div {
                style: "{footer_style}",

                button {
                    style: "{theme_btn_style}",
                    onclick: move |_| {
                        props.on_toggle_theme.call(());
                    },
                    "{theme_btn_label}"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nav_items_count() {
        assert_eq!(NAV_ITEMS.len(), 4);
    }

    #[test]
    fn test_nav_items_pages() {
        assert_eq!(NAV_ITEMS[0].page, Page::Dashboard);
        assert_eq!(NAV_ITEMS[1].page, Page::Chat);
        assert_eq!(NAV_ITEMS[2].page, Page::Models);
        assert_eq!(NAV_ITEMS[3].page, Page::Channels);
    }

    #[test]
    fn test_nav_items_labels() {
        assert_eq!(NAV_ITEMS[0].label, "Dashboard");
        assert_eq!(NAV_ITEMS[1].label, "Chat");
        assert_eq!(NAV_ITEMS[2].label, "Models");
        assert_eq!(NAV_ITEMS[3].label, "Channels");
    }
}
