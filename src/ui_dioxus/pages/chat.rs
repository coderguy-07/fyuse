//! Chat interface page — ported from the Yew ChatWindow and InputArea components.

use dioxus::prelude::*;

use crate::ui_dioxus::app::Theme;
use crate::ui_dioxus::components::message_bubble::MessageBubble;
use crate::ui_dioxus::{Message, MessageRole, ModelInfo};

/// Props for the ChatPage.
#[derive(Props, Clone, PartialEq)]
pub struct ChatPageProps {
    /// Current theme.
    pub theme: Theme,
}

/// Returns placeholder models for demonstration.
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
    ]
}

/// Chat page with message display, input area, model selector, and streaming indicator.
#[component]
pub fn ChatPage(props: ChatPageProps) -> Element {
    let theme = props.theme;
    let mut messages = use_signal(Vec::<Message>::new);
    let mut input = use_signal(String::new);
    let mut is_streaming = use_signal(|| false);
    let mut selected_model = use_signal(|| "llama3-8b".to_string());
    let models = placeholder_models();
    let border_color = theme.border_color();

    let card_bg = match theme {
        Theme::Light => "#ffffff",
        Theme::Dark => "#1f2937",
    };

    let input_bg = match theme {
        Theme::Light => "#ffffff",
        Theme::Dark => "#374151",
    };

    let select_style = format!(
        "padding: 0.4rem 0.75rem; border: 1px solid {border_color}; \
         border-radius: 0.375rem; background: {input_bg}; \
         color: inherit; font-size: 0.85rem;",
    );

    let msg_area_style = format!(
        "flex: 1; overflow-y: auto; background: {card_bg}; \
         border: 1px solid {border_color}; border-radius: 0.5rem; \
         padding: 1rem; margin-bottom: 1rem;",
    );

    let input_style = format!(
        "flex: 1; padding: 0.75rem 1rem; border: 1px solid {border_color}; \
         border-radius: 0.5rem; background: {input_bg}; color: inherit; \
         font-size: 1rem; outline: none;",
    );

    let mut do_send = move || {
        let text = input().trim().to_string();
        if text.is_empty() || is_streaming() {
            return;
        }

        let user_msg = Message {
            role: MessageRole::User,
            content: text.clone(),
            timestamp: "just now".into(),
            model: None,
            tokens: Some(text.split_whitespace().count()),
        };
        messages.write().push(user_msg);
        input.set(String::new());

        is_streaming.set(true);
        let model = selected_model().clone();
        let response = Message {
            role: MessageRole::Assistant,
            content: format!("This is a placeholder response to: \"{text}\""),
            timestamp: "just now".into(),
            model: Some(model),
            tokens: Some(12),
        };
        messages.write().push(response);
        is_streaming.set(false);
    };

    rsx! {
        div {
            class: "chat-page",
            style: "display: flex; flex-direction: column; height: 100%;",

            // Header with model selector
            div {
                style: "display: flex; justify-content: space-between; align-items: center; \
                        margin-bottom: 1rem;",

                h2 {
                    style: "margin: 0; font-size: 1.5rem;",
                    "Chat"
                }

                div {
                    style: "display: flex; align-items: center; gap: 0.75rem;",

                    label {
                        style: "font-size: 0.85rem; opacity: 0.7;",
                        "Model:"
                    }

                    select {
                        style: "{select_style}",
                        value: "{selected_model}",
                        onchange: move |evt: Event<FormData>| {
                            selected_model.set(evt.value().to_string());
                        },
                        for model in &models {
                            option {
                                value: "{model.id}",
                                "{model.name}"
                            }
                        }
                    }
                }
            }

            // Message area
            div {
                style: "{msg_area_style}",

                if messages().is_empty() {
                    div {
                        style: "display: flex; flex-direction: column; align-items: center; \
                                justify-content: center; height: 100%; opacity: 0.5;",

                        div {
                            style: "font-size: 2.5rem; margin-bottom: 0.5rem;",
                            "[Chat]"
                        }
                        h3 { "Start a conversation" }
                        p {
                            style: "font-size: 0.9rem;",
                            "Select a model and type your message below"
                        }
                    }
                } else {
                    for msg in messages().iter() {
                        MessageBubble {
                            key: "{msg.timestamp}-{msg.content}",
                            message: msg.clone(),
                            theme: theme,
                        }
                    }
                }

                // Streaming indicator
                if is_streaming() {
                    div {
                        style: "display: flex; align-items: center; gap: 0.5rem; \
                                padding: 0.5rem; opacity: 0.7;",
                        span {
                            style: "display: inline-block;",
                            "..."
                        }
                        span {
                            style: "font-size: 0.85rem;",
                            "Generating response..."
                        }
                    }
                }
            }

            // Input area
            div {
                style: "display: flex; gap: 0.5rem;",

                input {
                    r#type: "text",
                    placeholder: "Type your message...",
                    style: "{input_style}",
                    value: "{input}",
                    disabled: is_streaming(),
                    oninput: move |evt: Event<FormData>| {
                        input.set(evt.value().to_string());
                    },
                    onkeypress: move |evt: Event<KeyboardData>| {
                        if evt.data().key() == Key::Enter {
                            do_send();
                        }
                    },
                }

                button {
                    style: "padding: 0.75rem 1.5rem; background: #3b82f6; color: white; \
                            border: none; border-radius: 0.5rem; cursor: pointer; \
                            font-size: 1rem; font-weight: 600;",
                    disabled: is_streaming(),
                    onclick: move |_| do_send(),
                    "Send"
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_placeholder_models() {
        let models = placeholder_models();
        assert_eq!(models.len(), 3);
        assert_eq!(models[0].id, "llama3-8b");
        assert!(models[0].loaded);
        assert!(!models[1].loaded);
    }

    #[test]
    fn test_chat_page_props() {
        let props = ChatPageProps {
            theme: Theme::Light,
        };
        assert_eq!(props.theme, Theme::Light);
    }
}
