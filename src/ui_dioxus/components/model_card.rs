//! Model info card component.

use dioxus::prelude::*;

use crate::ui_dioxus::app::Theme;
use crate::ui_dioxus::{format_bytes, ModelInfo};

/// Props for the ModelCard component.
#[derive(Props, Clone, PartialEq)]
pub struct ModelCardProps {
    /// Model information to display.
    pub model: ModelInfo,
    /// Current theme.
    pub theme: Theme,
    /// Callback when delete is clicked.
    pub on_delete: EventHandler<String>,
    /// Whether to show detailed view.
    #[props(default = false)]
    pub show_details: bool,
}

/// Displays a model's information as a card.
#[component]
pub fn ModelCard(props: ModelCardProps) -> Element {
    let model = &props.model;
    let theme = props.theme;

    let card_bg = match theme {
        Theme::Light => "#ffffff",
        Theme::Dark => "#1f2937",
    };

    let card_style = format!(
        "background: {card_bg}; border: 1px solid {}; border-radius: 0.5rem; \
         padding: 1rem; margin-bottom: 0.75rem;",
        theme.border_color(),
    );

    let size_display = format_bytes(model.size_bytes);
    let status_color = if model.loaded { "#10b981" } else { "#6b7280" };
    let status_text = if model.loaded { "Loaded" } else { "Unloaded" };

    let model_id = model.id.clone();

    rsx! {
        div {
            class: "model-card",
            style: "{card_style}",

            div {
                style: "display: flex; justify-content: space-between; align-items: center;",

                div {
                    h3 {
                        style: "margin: 0 0 0.25rem 0; font-size: 1.1rem;",
                        "{model.name}"
                    }
                    span {
                        style: "font-size: 0.8rem; opacity: 0.7;",
                        "{model.id}"
                    }
                }

                div {
                    style: "display: flex; align-items: center; gap: 0.5rem;",

                    span {
                        style: "display: inline-flex; align-items: center; gap: 0.25rem; \
                                font-size: 0.8rem; color: {status_color}; font-weight: 600;",
                        span {
                            style: "width: 8px; height: 8px; border-radius: 50%; \
                                    background: {status_color}; display: inline-block;",
                        }
                        "{status_text}"
                    }

                    button {
                        style: "background: #ef4444; color: white; border: none; \
                                border-radius: 0.25rem; padding: 0.25rem 0.5rem; \
                                cursor: pointer; font-size: 0.75rem;",
                        onclick: move |_| {
                            props.on_delete.call(model_id.clone());
                        },
                        "Delete"
                    }
                }
            }

            div {
                style: "display: flex; gap: 1rem; margin-top: 0.5rem; font-size: 0.85rem; opacity: 0.8;",

                span { "Size: {size_display}" }

                if let Some(quant) = &model.quantization {
                    span { "Quant: {quant}" }
                }
            }

            if props.show_details {
                {
                    let border_c = theme.border_color();
                    let details_style = format!(
                        "margin-top: 0.75rem; padding-top: 0.75rem; \
                         border-top: 1px solid {border_c}; font-size: 0.85rem;",
                    );
                    let size_str = format_bytes(model.size_bytes);
                    rsx! {
                        div {
                            style: "{details_style}",
                            p { "Model ID: {model.id}" }
                            p { "Size: {size_str}" }
                            p { "Status: {status_text}" }
                            if let Some(quant) = &model.quantization {
                                p { "Quantization: {quant}" }
                            }
                        }
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
    fn test_model_info_display() {
        let model = ModelInfo {
            id: "test".into(),
            name: "Test".into(),
            size_bytes: 1000,
            loaded: false,
            quantization: None,
        };
        assert_eq!(model.id, "test");
        assert_eq!(format_bytes(model.size_bytes), "1000 B");
    }

    #[test]
    fn test_status_display() {
        let loaded = ModelInfo {
            id: "m".into(),
            name: "M".into(),
            size_bytes: 0,
            loaded: true,
            quantization: None,
        };
        let unloaded = ModelInfo {
            id: "m".into(),
            name: "M".into(),
            size_bytes: 0,
            loaded: false,
            quantization: None,
        };
        assert!(loaded.loaded);
        assert!(!unloaded.loaded);
    }
}
