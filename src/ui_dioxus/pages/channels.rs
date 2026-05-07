//! Channel management page — list, enable/disable, configure channels.

use dioxus::prelude::*;

use crate::ui_dioxus::app::Theme;
use crate::ui_dioxus::components::status_badge::{Status, StatusBadge};
use crate::ui_dioxus::ChannelInfo;

/// Props for the ChannelsPage.
#[derive(Props, Clone, PartialEq)]
pub struct ChannelsPageProps {
    /// Current theme.
    pub theme: Theme,
}

/// Returns placeholder channel data.
fn placeholder_channels() -> Vec<ChannelInfo> {
    vec![
        ChannelInfo {
            id: "telegram-1".into(),
            name: "Telegram Bot".into(),
            channel_type: "telegram".into(),
            enabled: true,
            connected_users: 156,
        },
        ChannelInfo {
            id: "discord-1".into(),
            name: "Discord Server".into(),
            channel_type: "discord".into(),
            enabled: true,
            connected_users: 89,
        },
        ChannelInfo {
            id: "slack-1".into(),
            name: "Slack Workspace".into(),
            channel_type: "slack".into(),
            enabled: false,
            connected_users: 0,
        },
        ChannelInfo {
            id: "matrix-1".into(),
            name: "Matrix Room".into(),
            channel_type: "matrix".into(),
            enabled: false,
            connected_users: 0,
        },
    ]
}

/// Channel management page with list, enable/disable toggles, and configuration.
#[component]
pub fn ChannelsPage(props: ChannelsPageProps) -> Element {
    let theme = props.theme;
    let mut channels = use_signal(placeholder_channels);
    let mut config_channel_id = use_signal(|| Option::<String>::None);
    let border_color = theme.border_color();

    let card_bg = match theme {
        Theme::Light => "#ffffff",
        Theme::Dark => "#1f2937",
    };

    let total_users: u32 = channels().iter().map(|c| c.connected_users).sum();
    let channel_list = channels();

    let card_style = format!(
        "background: {card_bg}; border: 1px solid {border_color}; border-radius: 0.5rem; \
         padding: 1rem; margin-bottom: 0.75rem;",
    );

    let btn_style = format!(
        "padding: 0.375rem 0.75rem; border: 1px solid {border_color}; \
         border-radius: 0.25rem; background: transparent; \
         color: inherit; cursor: pointer; font-size: 0.8rem;",
    );

    let config_panel_style = format!(
        "margin-top: 0.75rem; padding-top: 0.75rem; \
         border-top: 1px solid {border_color}; font-size: 0.9rem;",
    );

    rsx! {
        div {
            class: "channels-page",

            div {
                style: "display: flex; justify-content: space-between; align-items: center; \
                        margin-bottom: 1rem;",

                h2 {
                    style: "margin: 0; font-size: 1.5rem;",
                    "Channel Management"
                }

                span {
                    style: "font-size: 0.9rem; opacity: 0.6;",
                    "Total connected users: {total_users}"
                }
            }

            for channel in channel_list.iter() {
                {
                    let status = if channel.enabled { Status::Online } else { Status::Offline };
                    let ch_id = channel.id.clone();
                    let ch_id_toggle = channel.id.clone();
                    let ch_id_config = channel.id.clone();
                    let ch_id_config2 = channel.id.clone();
                    let is_config_open = config_channel_id() == Some(channel.id.clone());

                    rsx! {
                        div {
                            class: "channel-card",
                            style: "{card_style}",

                            div {
                                style: "display: flex; justify-content: space-between; align-items: center;",

                                div {
                                    style: "display: flex; align-items: center; gap: 0.75rem;",
                                    StatusBadge { status: status }
                                    div {
                                        h3 {
                                            style: "margin: 0; font-size: 1.05rem;",
                                            "{channel.name}"
                                        }
                                        span {
                                            style: "font-size: 0.8rem; opacity: 0.6;",
                                            "{channel.channel_type} | {channel.connected_users} users"
                                        }
                                    }
                                }

                                div {
                                    style: "display: flex; align-items: center; gap: 0.75rem;",

                                    label {
                                        style: "display: flex; align-items: center; gap: 0.375rem; \
                                                cursor: pointer; font-size: 0.85rem;",
                                        input {
                                            r#type: "checkbox",
                                            checked: channel.enabled,
                                            onchange: move |_| {
                                                let id = ch_id_toggle.clone();
                                                let mut list = channels();
                                                if let Some(ch) = list.iter_mut().find(|c| c.id == id) {
                                                    ch.enabled = !ch.enabled;
                                                    if !ch.enabled {
                                                        ch.connected_users = 0;
                                                    }
                                                }
                                                channels.set(list);
                                            },
                                        }
                                        "Enabled"
                                    }

                                    button {
                                        style: "{btn_style}",
                                        onclick: move |_| {
                                            let current = config_channel_id();
                                            if current == Some(ch_id_config.clone()) {
                                                config_channel_id.set(None);
                                            } else {
                                                config_channel_id.set(Some(ch_id_config.clone()));
                                            }
                                        },
                                        if is_config_open { "Hide Config" } else { "Configure" }
                                    }
                                }
                            }

                            if is_config_open {
                                div {
                                    style: "{config_panel_style}",

                                    p {
                                        style: "margin: 0 0 0.5rem; opacity: 0.7;",
                                        "Configuration for {ch_id}"
                                    }
                                    p {
                                        style: "font-size: 0.8rem; opacity: 0.5;",
                                        "Channel configuration is managed in fuse.toml. \
                                         Edit the [channels.{ch_id_config2}] section to update settings."
                                    }
                                }
                            }
                        }
                    }
                }
            }

            if channel_list.is_empty() {
                div {
                    style: "text-align: center; padding: 3rem; opacity: 0.5;",
                    p { "No channels configured." }
                    p {
                        style: "font-size: 0.85rem;",
                        "Add channel configurations in fuse.toml to get started."
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_channels() {
        let channels = placeholder_channels();
        assert_eq!(channels.len(), 4);
        assert!(channels[0].enabled);
        assert!(!channels[2].enabled);
    }

    #[test]
    fn test_total_connected_users() {
        let channels = placeholder_channels();
        let total: u32 = channels.iter().map(|c| c.connected_users).sum();
        assert_eq!(total, 245);
    }

    #[test]
    fn test_channels_page_props() {
        let props = ChannelsPageProps { theme: Theme::Dark };
        assert_eq!(props.theme, Theme::Dark);
    }

    #[test]
    fn test_channel_types() {
        let channels = placeholder_channels();
        let types: Vec<&str> = channels.iter().map(|c| c.channel_type.as_str()).collect();
        assert!(types.contains(&"telegram"));
        assert!(types.contains(&"discord"));
        assert!(types.contains(&"slack"));
        assert!(types.contains(&"matrix"));
    }
}
