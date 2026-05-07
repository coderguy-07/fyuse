use crate::ui::state::{Message, MessageRole};
use std::collections::VecDeque;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ChatWindowProps {
    pub messages: VecDeque<Message>,
    pub is_streaming: bool,
}

#[function_component(ChatWindow)]
pub fn chat_window(props: &ChatWindowProps) -> Html {
    html! {
        <div
            class="chat-window"
            style="
                flex: 1;
                overflow-y: auto;
                padding: 1rem;
                background: #f9fafb;
            "
        >
            { if props.messages.is_empty() {
                html! {
                    <div
                        class="empty-state"
                        style="
                            display: flex;
                            flex-direction: column;
                            align-items: center;
                            justify-content: center;
                            height: 100%;
                            color: #6b7280;
                        "
                    >
                        <div style="font-size: 3rem; margin-bottom: 1rem;">
                            {"💬"}
                        </div>
                        <h2 style="font-size: 1.5rem; font-weight: 600; margin-bottom: 0.5rem;">
                            {"Start a conversation"}
                        </h2>
                        <p style="text-align: center; max-width: 400px;">
                            {"Select a model and type your message below to begin"}
                        </p>
                    </div>
                }
            } else {
                html! {
                    <div class="messages-container" style="max-width: 800px; margin: 0 auto;">
                        { for props.messages.iter().map(|message| {
                            html! { <MessageBubble message={message.clone()} /> }
                        }) }

                        { if props.is_streaming {
                            html! {
                                <div
                                    class="streaming-indicator"
                                    style="
                                        padding: 1rem;
                                        margin: 0.5rem 0;
                                        background: white;
                                        border-radius: 0.5rem;
                                        box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
                                    "
                                >
                                    <div class="typing-dots" style="display: flex; gap: 0.25rem;">
                                        <span style="width: 8px; height: 8px; background: #3b82f6; border-radius: 50%; animation: bounce 1.4s infinite ease-in-out;"></span>
                                        <span style="width: 8px; height: 8px; background: #3b82f6; border-radius: 50%; animation: bounce 1.4s infinite ease-in-out 0.2s;"></span>
                                        <span style="width: 8px; height: 8px; background: #3b82f6; border-radius: 50%; animation: bounce 1.4s infinite ease-in-out 0.4s;"></span>
                                    </div>
                                </div>
                            }
                        } else {
                            html! {}
                        }}
                    </div>
                }
            }}
        </div>
    }
}

#[derive(Properties, PartialEq)]
struct MessageBubbleProps {
    message: Message,
}

#[function_component(MessageBubble)]
fn message_bubble(props: &MessageBubbleProps) -> Html {
    let is_user = matches!(props.message.role, MessageRole::User);

    html! {
        <div
            class="message-bubble"
            style={format!("
                display: flex;
                margin-bottom: 1rem;
                {}
            ", if is_user { "justify-content: flex-end;" } else { "justify-content: flex-start;" })}
        >
            <div
                class="message-content"
                style={format!("
                    max-width: 70%;
                    padding: 1rem;
                    border-radius: 0.5rem;
                    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.1);
                    {}
                ", if is_user {
                    "background: #3b82f6; color: white;"
                } else {
                    "background: white; color: #1f2937;"
                })}
            >
                <div
                    class="message-header"
                    style="
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                        margin-bottom: 0.5rem;
                        font-size: 0.75rem;
                        opacity: 0.8;
                    "
                >
                    <span>{format_role(&props.message.role)}</span>
                    <span>{format_time(&props.message.timestamp)}</span>
                </div>

                <div
                    class="message-text"
                    style="white-space: pre-wrap; word-wrap: break-word;"
                >
                    {render_markdown(&props.message.content)}
                </div>

                { if let Some(tokens) = props.message.tokens {
                    html! {
                        <div
                            class="message-footer"
                            style="
                                margin-top: 0.5rem;
                                font-size: 0.75rem;
                                opacity: 0.7;
                            "
                        >
                            {format!("{} tokens", tokens)}
                        </div>
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}

fn format_role(role: &MessageRole) -> &'static str {
    match role {
        MessageRole::User => "You",
        MessageRole::Assistant => "Assistant",
        MessageRole::System => "System",
    }
}

fn format_time(timestamp: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(*timestamp);

    if duration.num_seconds() < 60 {
        "Just now".to_string()
    } else if duration.num_minutes() < 60 {
        format!("{}m ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h ago", duration.num_hours())
    } else {
        timestamp.format("%b %d, %H:%M").to_string()
    }
}

fn render_markdown(content: &str) -> String {
    // Simple markdown rendering (in production, use a proper markdown library)
    let mut html = content
        .replace("&", "&amp;")
        .replace("<", "&lt;")
        .replace(">", "&gt;");

    // Bold
    html = html.replace("**", "<strong>").replace("**", "</strong>");

    // Italic
    html = html.replace("*", "<em>").replace("*", "</em>");

    // Code blocks
    if html.contains("```") {
        let parts: Vec<&str> = html.split("```").collect();
        let mut result = String::new();
        for (i, part) in parts.iter().enumerate() {
            if i % 2 == 0 {
                result.push_str(part);
            } else {
                result.push_str("<pre><code>");
                result.push_str(part);
                result.push_str("</code></pre>");
            }
        }
        html = result;
    }

    // Inline code
    html = html.replace("`", "<code>").replace("`", "</code>");

    html
}
