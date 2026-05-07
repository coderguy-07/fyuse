//! Status indicator badge component (online/offline/loading).

use dioxus::prelude::*;

/// Status variants for the badge.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Online,
    Offline,
    Loading,
    Error,
}

impl Status {
    /// Color for this status.
    pub fn color(self) -> &'static str {
        match self {
            Status::Online => "#10b981",
            Status::Offline => "#6b7280",
            Status::Loading => "#f59e0b",
            Status::Error => "#ef4444",
        }
    }

    /// Label text for this status.
    pub fn label(self) -> &'static str {
        match self {
            Status::Online => "Online",
            Status::Offline => "Offline",
            Status::Loading => "Loading",
            Status::Error => "Error",
        }
    }
}

/// Props for the StatusBadge component.
#[derive(Props, Clone, PartialEq)]
pub struct StatusBadgeProps {
    /// Current status to display.
    pub status: Status,
    /// Optional custom label (overrides default).
    #[props(default)]
    pub label: Option<String>,
}

/// Renders a small status indicator badge with a colored dot and label.
#[component]
pub fn StatusBadge(props: StatusBadgeProps) -> Element {
    let color = props.status.color();
    let label = props
        .label
        .as_deref()
        .unwrap_or_else(|| props.status.label());

    rsx! {
        span {
            class: "status-badge",
            style: "display: inline-flex; align-items: center; gap: 0.375rem; \
                    font-size: 0.8rem; font-weight: 500; color: {color};",

            span {
                style: "width: 8px; height: 8px; border-radius: 50%; background: {color}; \
                        display: inline-block;",
            }
            "{label}"
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_status_colors() {
        assert_eq!(Status::Online.color(), "#10b981");
        assert_eq!(Status::Offline.color(), "#6b7280");
        assert_eq!(Status::Loading.color(), "#f59e0b");
        assert_eq!(Status::Error.color(), "#ef4444");
    }

    #[test]
    fn test_status_labels() {
        assert_eq!(Status::Online.label(), "Online");
        assert_eq!(Status::Offline.label(), "Offline");
        assert_eq!(Status::Loading.label(), "Loading");
        assert_eq!(Status::Error.label(), "Error");
    }

    #[test]
    fn test_status_equality() {
        assert_eq!(Status::Online, Status::Online);
        assert_ne!(Status::Online, Status::Offline);
    }
}
