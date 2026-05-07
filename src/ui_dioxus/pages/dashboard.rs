//! System dashboard page — CPU, RAM, GPU stats, loaded models, queue status.

use dioxus::prelude::*;

use crate::ui_dioxus::app::Theme;
use crate::ui_dioxus::components::status_badge::{Status, StatusBadge};
use crate::ui_dioxus::format_bytes;

/// Props for the DashboardPage.
#[derive(Props, Clone, PartialEq)]
pub struct DashboardPageProps {
    /// Current theme.
    pub theme: Theme,
}

/// Placeholder system metrics.
#[derive(Debug, Clone, PartialEq)]
pub struct SystemMetrics {
    pub cpu_usage_percent: f32,
    pub ram_used_bytes: u64,
    pub ram_total_bytes: u64,
    pub gpu_name: Option<String>,
    pub gpu_usage_percent: Option<f32>,
    pub gpu_memory_used: Option<u64>,
    pub gpu_memory_total: Option<u64>,
    pub loaded_models: Vec<String>,
    pub request_queue_size: usize,
    pub active_connections: u32,
    pub total_requests_served: u64,
    pub uptime_seconds: u64,
}

impl Default for SystemMetrics {
    fn default() -> Self {
        Self {
            cpu_usage_percent: 23.5,
            ram_used_bytes: 8_500_000_000,
            ram_total_bytes: 16_000_000_000,
            gpu_name: Some("Apple M2 Pro".into()),
            gpu_usage_percent: Some(15.0),
            gpu_memory_used: Some(2_000_000_000),
            gpu_memory_total: Some(16_000_000_000),
            loaded_models: vec!["llama3-8b".into(), "phi-3-mini".into()],
            request_queue_size: 3,
            active_connections: 7,
            total_requests_served: 1_247,
            uptime_seconds: 86_400,
        }
    }
}

/// Format seconds into a human-readable uptime string.
fn format_uptime(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let mins = (seconds % 3_600) / 60;
    if days > 0 {
        format!("{days}d {hours}h {mins}m")
    } else if hours > 0 {
        format!("{hours}h {mins}m")
    } else {
        format!("{mins}m")
    }
}

/// System dashboard displaying metrics, loaded models, and status.
#[component]
pub fn DashboardPage(props: DashboardPageProps) -> Element {
    let theme = props.theme;
    let metrics = SystemMetrics::default();
    let border_color = theme.border_color();

    let card_bg = match theme {
        Theme::Light => "#ffffff",
        Theme::Dark => "#1f2937",
    };

    let card_style = format!(
        "background: {card_bg}; border: 1px solid {border_color}; border-radius: 0.5rem; padding: 1rem;",
    );

    let bar_bg_style = format!(
        "height: 6px; background: {border_color}; border-radius: 3px; margin-top: 0.5rem;",
    );

    let ram_percent =
        (metrics.ram_used_bytes as f64 / metrics.ram_total_bytes as f64 * 100.0) as u32;

    let uptime_str = format_uptime(metrics.uptime_seconds);
    let ram_used_str = format_bytes(metrics.ram_used_bytes);
    let ram_total_str = format_bytes(metrics.ram_total_bytes);

    let gpu_vram_str = match (metrics.gpu_memory_used, metrics.gpu_memory_total) {
        (Some(used), Some(total)) => {
            format!("VRAM: {} / {}", format_bytes(used), format_bytes(total))
        }
        _ => String::new(),
    };

    let model_item_style = format!(
        "display: flex; align-items: center; gap: 0.5rem; padding: 0.4rem 0; \
         border-bottom: 1px solid {border_color};",
    );

    rsx! {
        div {
            class: "dashboard-page",

            h2 {
                style: "margin: 0 0 1rem; font-size: 1.5rem;",
                "Dashboard"
            }

            // Status bar
            div {
                style: "display: flex; gap: 0.75rem; margin-bottom: 1.5rem; align-items: center;",

                StatusBadge { status: Status::Online, label: Some("Fuse Running".into()) }

                span {
                    style: "font-size: 0.85rem; opacity: 0.6;",
                    "Uptime: {uptime_str}"
                }

                span {
                    style: "font-size: 0.85rem; opacity: 0.6;",
                    "Requests served: {metrics.total_requests_served}"
                }
            }

            // Metrics grid
            div {
                style: "display: grid; grid-template-columns: repeat(auto-fit, minmax(250px, 1fr)); \
                        gap: 1rem; margin-bottom: 1.5rem;",

                // CPU card
                div {
                    style: "{card_style}",

                    h3 {
                        style: "margin: 0 0 0.5rem; font-size: 1rem; opacity: 0.7;",
                        "CPU"
                    }
                    div {
                        style: "font-size: 2rem; font-weight: 700;",
                        "{metrics.cpu_usage_percent:.1}%"
                    }
                    div {
                        style: "{bar_bg_style}",
                        div {
                            style: "height: 100%; width: {metrics.cpu_usage_percent}%; \
                                    background: #3b82f6; border-radius: 3px;",
                        }
                    }
                }

                // RAM card
                div {
                    style: "{card_style}",

                    h3 {
                        style: "margin: 0 0 0.5rem; font-size: 1rem; opacity: 0.7;",
                        "RAM"
                    }
                    div {
                        style: "font-size: 2rem; font-weight: 700;",
                        "{ram_used_str} / {ram_total_str}"
                    }
                    div {
                        style: "{bar_bg_style}",
                        div {
                            style: "height: 100%; width: {ram_percent}%; \
                                    background: #10b981; border-radius: 3px;",
                        }
                    }
                }

                // GPU card
                div {
                    style: "{card_style}",

                    h3 {
                        style: "margin: 0 0 0.5rem; font-size: 1rem; opacity: 0.7;",
                        "GPU"
                    }
                    if let Some(ref gpu_name) = metrics.gpu_name {
                        div {
                            style: "font-size: 0.85rem; opacity: 0.6; margin-bottom: 0.25rem;",
                            "{gpu_name}"
                        }
                        if let Some(usage) = metrics.gpu_usage_percent {
                            div {
                                style: "font-size: 2rem; font-weight: 700;",
                                "{usage:.1}%"
                            }
                        }
                        if !gpu_vram_str.is_empty() {
                            div {
                                style: "font-size: 0.85rem; opacity: 0.7; margin-top: 0.25rem;",
                                "{gpu_vram_str}"
                            }
                        }
                    } else {
                        div {
                            style: "font-size: 1.25rem; opacity: 0.5;",
                            "No GPU detected"
                        }
                    }
                }

                // Queue card
                div {
                    style: "{card_style}",

                    h3 {
                        style: "margin: 0 0 0.5rem; font-size: 1rem; opacity: 0.7;",
                        "Request Queue"
                    }
                    div {
                        style: "font-size: 2rem; font-weight: 700;",
                        "{metrics.request_queue_size}"
                    }
                    div {
                        style: "font-size: 0.85rem; opacity: 0.6; margin-top: 0.25rem;",
                        "pending requests"
                    }
                }
            }

            // Bottom row
            div {
                style: "display: grid; grid-template-columns: 1fr 1fr; gap: 1rem;",

                // Loaded models
                div {
                    style: "{card_style}",

                    h3 {
                        style: "margin: 0 0 0.75rem; font-size: 1rem;",
                        "Loaded Models ({metrics.loaded_models.len()})"
                    }

                    if metrics.loaded_models.is_empty() {
                        p {
                            style: "opacity: 0.5;",
                            "No models currently loaded"
                        }
                    } else {
                        for model_name in &metrics.loaded_models {
                            div {
                                style: "{model_item_style}",
                                StatusBadge { status: Status::Online }
                                span { "{model_name}" }
                            }
                        }
                    }
                }

                // Active connections
                div {
                    style: "{card_style}",

                    h3 {
                        style: "margin: 0 0 0.75rem; font-size: 1rem;",
                        "Connections"
                    }
                    div {
                        style: "font-size: 2.5rem; font-weight: 700; text-align: center; \
                                margin: 1rem 0;",
                        "{metrics.active_connections}"
                    }
                    div {
                        style: "text-align: center; font-size: 0.85rem; opacity: 0.6;",
                        "active connections"
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
    fn test_system_metrics_default() {
        let m = SystemMetrics::default();
        assert!(m.cpu_usage_percent > 0.0);
        assert!(m.ram_used_bytes > 0);
        assert!(m.ram_total_bytes > m.ram_used_bytes);
        assert!(!m.loaded_models.is_empty());
    }

    #[test]
    fn test_format_uptime() {
        assert_eq!(format_uptime(90), "1m");
        assert_eq!(format_uptime(3_661), "1h 1m");
        assert_eq!(format_uptime(86_400), "1d 0h 0m");
        assert_eq!(format_uptime(90_061), "1d 1h 1m");
    }

    #[test]
    fn test_dashboard_props() {
        let props = DashboardPageProps {
            theme: Theme::Light,
        };
        assert_eq!(props.theme, Theme::Light);
    }
}
