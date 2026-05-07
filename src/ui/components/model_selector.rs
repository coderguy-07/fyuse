use crate::ui::state::ModelInfo;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ModelSelectorProps {
    pub models: Vec<ModelInfo>,
    pub selected: Option<String>,
    pub on_select: Callback<String>,
}

#[function_component(ModelSelector)]
pub fn model_selector(props: &ModelSelectorProps) -> Html {
    let show_dropdown = use_state(|| false);

    let toggle_dropdown = {
        let show_dropdown = show_dropdown.clone();
        Callback::from(move |_| {
            show_dropdown.set(!*show_dropdown);
        })
    };

    let select_model = {
        let on_select = props.on_select.clone();
        let show_dropdown = show_dropdown.clone();
        Callback::from(move |model_name: String| {
            on_select.emit(model_name);
            show_dropdown.set(false);
        })
    };

    html! {
        <div class="model-selector" style="position: relative;">

            <button
                class="selector-button"
                style="
                    padding: 0.5rem 1rem;
                    border: 1px solid #d1d5db;
                    border-radius: 0.375rem;
                    background: white;
                    cursor: pointer;
                    display: flex;
                    align-items: center;
                    gap: 0.5rem;
                "
                onclick={toggle_dropdown}
            >
                <span>
                    { if let Some(ref model) = props.selected {
                        model.clone()
                    } else {
                        "Select Model".to_string()
                    }}
                </span>
                <span style="margin-left: 0.5rem;">
                    {"▼"}
                </span>
            </button>

            if *show_dropdown {
                <div
                    class="dropdown-menu"
                    style="
                        position: absolute;
                        top: 100%;
                        right: 0;
                        margin-top: 0.25rem;
                        background: white;
                        border: 1px solid #d1d5db;
                        border-radius: 0.375rem;
                        box-shadow: 0 4px 6px rgba(0, 0, 0, 0.1);
                        min-width: 200px;
                        max-height: 300px;
                        overflow-y: auto;
                        z-index: 50;
                    "
                >
                    { for props.models.iter().map(|model| {
                        let model_name = model.name.clone();
                        let select_model = select_model.clone();
                        html! {
                            <div
                                key={model.id.clone()}
                                class="dropdown-item"
                                style="
                                    padding: 0.75rem 1rem;
                                    cursor: pointer;
                                    border-bottom: 1px solid #f3f4f6;
                                "
                                onmouseover="this.style.backgroundColor = '#f9fafb'"
                                onmouseout="this.style.backgroundColor = 'transparent'"
                                onclick={Callback::from(move |_| select_model.emit(model_name.clone()))}
                            >
                                <div style="font-weight: 500;">
                                    {model.name.clone()}
                                </div>
                                <div style="font-size: 0.75rem; color: #6b7280; margin-top: 0.25rem;">
                                    {format_size(model.size_bytes)}
                                    { if model.loaded {
                                        html! { <span style="margin-left: 0.5rem; color: #10b981;">{"● Loaded"}</span> }
                                    } else {
                                        html! {}
                                    }}
                                </div>
                            </div>
                        }
                    }) }

                    { if props.models.is_empty() {
                        html! {
                            <div style="padding: 1rem; text-align: center; color: #6b7280;">
                                {"No models available"}
                            </div>
                        }
                    } else {
                        html! {}
                    }}
                </div>
            }
        </div>
    }
}

fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;

    if bytes >= GB {
        format!("{:.2} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.2} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.2} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}
