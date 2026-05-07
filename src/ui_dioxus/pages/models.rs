//! Model management page — list, pull, delete, quantize models.

use dioxus::prelude::*;

use crate::ui_dioxus::app::Theme;
use crate::ui_dioxus::components::model_card::ModelCard;
use crate::ui_dioxus::ModelInfo;

/// Props for the ModelsPage.
#[derive(Props, Clone, PartialEq)]
pub struct ModelsPageProps {
    /// Current theme.
    pub theme: Theme,
}

/// Returns placeholder model data.
fn placeholder_models() -> Vec<ModelInfo> {
    vec![
        ModelInfo {
            id: "llama3-8b".into(),
            name: "LLaMA 3 8B".into(),
            size_bytes: 4_500_000_000,
            loaded: true,
            quantization: Some("Q4_K_M".into()),
        },
        ModelInfo {
            id: "mistral-7b".into(),
            name: "Mistral 7B".into(),
            size_bytes: 4_100_000_000,
            loaded: false,
            quantization: Some("Q5_K_M".into()),
        },
        ModelInfo {
            id: "phi-3-mini".into(),
            name: "Phi-3 Mini".into(),
            size_bytes: 2_300_000_000,
            loaded: true,
            quantization: Some("Q4_0".into()),
        },
        ModelInfo {
            id: "gemma-2b".into(),
            name: "Gemma 2B".into(),
            size_bytes: 1_400_000_000,
            loaded: false,
            quantization: None,
        },
    ]
}

/// Quantization options available.
const QUANT_OPTIONS: &[&str] = &["Q4_0", "Q4_K_M", "Q5_K_M", "Q8_0"];

/// Model management page with list, pull, delete, and quantize functionality.
#[component]
pub fn ModelsPage(props: ModelsPageProps) -> Element {
    let theme = props.theme;
    let mut models = use_signal(placeholder_models);
    let mut pull_input = use_signal(String::new);
    let mut selected_quant = use_signal(|| "Q4_K_M".to_string());
    let detail_model_id = use_signal(|| Option::<String>::None);
    let border_color = theme.border_color();

    let card_bg = match theme {
        Theme::Light => "#ffffff",
        Theme::Dark => "#1f2937",
    };

    let input_bg = match theme {
        Theme::Light => "#ffffff",
        Theme::Dark => "#374151",
    };

    let form_card_style = format!(
        "background: {card_bg}; border: 1px solid {border_color}; \
         border-radius: 0.5rem; padding: 1rem; margin-bottom: 1.5rem;",
    );

    let text_input_style = format!(
        "width: 100%; padding: 0.5rem 0.75rem; \
         border: 1px solid {border_color}; border-radius: 0.375rem; \
         background: {input_bg}; color: inherit; font-size: 0.9rem; \
         box-sizing: border-box;",
    );

    let select_style = format!(
        "padding: 0.5rem 0.75rem; border: 1px solid {border_color}; \
         border-radius: 0.375rem; background: {input_bg}; \
         color: inherit; font-size: 0.9rem;",
    );

    let model_count = models().len();

    rsx! {
        div {
            class: "models-page",

            h2 {
                style: "margin: 0 0 1rem; font-size: 1.5rem;",
                "Model Management"
            }

            // Pull new model form
            div {
                style: "{form_card_style}",

                h3 {
                    style: "margin: 0 0 0.75rem; font-size: 1.1rem;",
                    "Pull New Model"
                }

                div {
                    style: "display: flex; gap: 0.5rem; align-items: flex-end;",

                    div {
                        style: "flex: 1;",
                        label {
                            style: "display: block; font-size: 0.8rem; margin-bottom: 0.25rem; opacity: 0.7;",
                            "Model name or HuggingFace ID"
                        }
                        input {
                            r#type: "text",
                            placeholder: "e.g., llama3:8b or meta-llama/Meta-Llama-3-8B",
                            style: "{text_input_style}",
                            value: "{pull_input}",
                            oninput: move |evt: Event<FormData>| {
                                pull_input.set(evt.value().to_string());
                            },
                        }
                    }

                    div {
                        label {
                            style: "display: block; font-size: 0.8rem; margin-bottom: 0.25rem; opacity: 0.7;",
                            "Quantization"
                        }
                        select {
                            style: "{select_style}",
                            value: "{selected_quant}",
                            onchange: move |evt: Event<FormData>| {
                                selected_quant.set(evt.value().to_string());
                            },
                            for opt in QUANT_OPTIONS {
                                option {
                                    value: "{opt}",
                                    "{opt}"
                                }
                            }
                        }
                    }

                    button {
                        style: "padding: 0.5rem 1.25rem; background: #3b82f6; color: white; \
                                border: none; border-radius: 0.375rem; cursor: pointer; \
                                font-size: 0.9rem; font-weight: 600; white-space: nowrap;",
                        onclick: move |_| {
                            let name = pull_input().trim().to_string();
                            if name.is_empty() {
                                return;
                            }
                            let quant = selected_quant().clone();
                            let new_model = ModelInfo {
                                id: name.clone(),
                                name: name.clone(),
                                size_bytes: 0,
                                loaded: false,
                                quantization: Some(quant),
                            };
                            models.write().push(new_model);
                            pull_input.set(String::new());
                        },
                        "Pull"
                    }
                }
            }

            // Model list
            h3 {
                style: "margin: 0 0 0.75rem; font-size: 1.1rem;",
                "Installed Models ({model_count})"
            }

            if models().is_empty() {
                p {
                    style: "opacity: 0.5; text-align: center; padding: 2rem;",
                    "No models installed. Pull one above to get started."
                }
            } else {
                for model in models().iter() {
                    ModelCard {
                        key: "{model.id}",
                        model: model.clone(),
                        theme: theme,
                        on_delete: move |id: String| {
                            models.write().retain(|m| m.id != id);
                        },
                        show_details: detail_model_id() == Some(model.id.clone()),
                    }
                }
            }

            div {
                style: "margin-top: 1rem; font-size: 0.8rem; opacity: 0.5;",
                "Click a model card's name to view details. Use the delete button to remove models."
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_models_count() {
        let models = placeholder_models();
        assert_eq!(models.len(), 4);
    }

    #[test]
    fn test_placeholder_models_have_ids() {
        for model in placeholder_models() {
            assert!(!model.id.is_empty());
            assert!(!model.name.is_empty());
        }
    }

    #[test]
    fn test_quant_options() {
        assert_eq!(QUANT_OPTIONS.len(), 4);
        assert!(QUANT_OPTIONS.contains(&"Q4_K_M"));
    }

    #[test]
    fn test_models_page_props() {
        let props = ModelsPageProps { theme: Theme::Dark };
        assert_eq!(props.theme, Theme::Dark);
    }
}
