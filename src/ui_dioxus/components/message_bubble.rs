//! Single chat message display component.

use dioxus::prelude::*;

use crate::ui_dioxus::app::Theme;
use crate::ui_dioxus::{Message, MessageRole};

/// Props for the MessageBubble component.
#[derive(Props, Clone, PartialEq)]
pub struct MessageBubbleProps {
    /// The message to display.
    pub message: Message,
    /// Current theme.
    pub theme: Theme,
}

/// Renders a single chat message with role-based styling.
///
/// User messages are styled blue and right-aligned.
/// Assistant messages are styled green and left-aligned.
/// System messages are styled gray and centered.
#[component]
pub fn MessageBubble(props: MessageBubbleProps) -> Element {
    let msg = &props.message;
    let theme = props.theme;

    let (bg, align, text_color) = match msg.role {
        MessageRole::User => ("#3b82f6", "flex-end", "#ffffff"),
        MessageRole::Assistant => ("#10b981", "flex-start", "#ffffff"),
        MessageRole::System => {
            let bg = match theme {
                Theme::Light => "#e5e7eb",
                Theme::Dark => "#374151",
            };
            (bg, "center", theme.text_color())
        }
    };

    let role_label = match msg.role {
        MessageRole::User => "You",
        MessageRole::Assistant => "Assistant",
        MessageRole::System => "System",
    };

    let container_style =
        format!("display: flex; justify-content: {align}; margin-bottom: 0.75rem;");

    let bubble_style = format!(
        "background: {bg}; color: {text_color}; padding: 0.75rem 1rem; \
         border-radius: 0.75rem; max-width: 70%; word-wrap: break-word;",
    );

    // Simple code block detection: if content has ```, render in a <pre> block
    let has_code = msg.content.contains("```");

    rsx! {
        div {
            class: "message-container",
            style: "{container_style}",

            div {
                class: "message-bubble",
                style: "{bubble_style}",

                div {
                    style: "font-size: 0.7rem; opacity: 0.8; margin-bottom: 0.25rem; font-weight: 600;",
                    "{role_label}"
                    if let Some(model) = &msg.model {
                        span {
                            style: "margin-left: 0.5rem; font-weight: 400;",
                            "({model})"
                        }
                    }
                }

                if has_code {
                    {render_content_with_code(&msg.content)}
                } else {
                    p {
                        style: "margin: 0; white-space: pre-wrap; line-height: 1.5;",
                        "{msg.content}"
                    }
                }

                if let Some(tokens) = msg.tokens {
                    div {
                        style: "font-size: 0.65rem; opacity: 0.6; margin-top: 0.25rem; text-align: right;",
                        "{tokens} tokens"
                    }
                }
            }
        }
    }
}

/// Render content that may contain markdown code blocks.
/// Splits on triple-backtick fences and renders code in <pre><code> blocks.
fn render_content_with_code(content: &str) -> Element {
    let parts: Vec<&str> = content.split("```").collect();

    rsx! {
        div {
            for (i, part) in parts.iter().enumerate() {
                if i % 2 == 1 {
                    // Inside a code fence
                    pre {
                        style: "background: rgba(0,0,0,0.2); padding: 0.5rem; border-radius: 0.375rem; \
                                overflow-x: auto; margin: 0.5rem 0; font-size: 0.85rem;",
                        code {
                            "{part.trim()}"
                        }
                    }
                } else if !part.is_empty() {
                    p {
                        style: "margin: 0; white-space: pre-wrap; line-height: 1.5;",
                        "{part}"
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
    fn test_message_bubble_props_equality() {
        let msg = Message {
            role: MessageRole::User,
            content: "Hello".into(),
            timestamp: "now".into(),
            model: None,
            tokens: None,
        };
        let props1 = MessageBubbleProps {
            message: msg.clone(),
            theme: Theme::Light,
        };
        let props2 = MessageBubbleProps {
            message: msg,
            theme: Theme::Light,
        };
        assert!(props1 == props2);
    }

    #[test]
    fn test_code_block_detection() {
        let content = "Hello ```code``` world";
        assert!(content.contains("```"));

        let plain = "Hello world";
        assert!(!plain.contains("```"));
    }
}
