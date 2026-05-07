use crate::ui::state::{AppState, Message};
use crate::ui::utils::DomUtils;
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct ExportDialogProps {
    pub is_open: bool,
    pub messages: Vec<Message>,
    pub on_close: Callback<()>,
}

#[derive(Clone, PartialEq)]
pub enum ExportFormat {
    Markdown,
    Json,
    Html,
    Pdf,
}

impl ExportFormat {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportFormat::Markdown => "markdown",
            ExportFormat::Json => "json",
            ExportFormat::Html => "html",
            ExportFormat::Pdf => "pdf",
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ExportFormat::Markdown => "Markdown (.md)",
            ExportFormat::Json => "JSON (.json)",
            ExportFormat::Html => "HTML (.html)",
            ExportFormat::Pdf => "PDF (.pdf)",
        }
    }

    pub fn extension(&self) -> &'static str {
        match self {
            ExportFormat::Markdown => ".md",
            ExportFormat::Json => ".json",
            ExportFormat::Html => ".html",
            ExportFormat::Pdf => ".pdf",
        }
    }
}

#[function_component(ExportDialog)]
pub fn export_dialog(props: &ExportDialogProps) -> Html {
    if !props.is_open {
        return html! {};
    }

    let selected_format = use_state(|| ExportFormat::Markdown);
    let include_metadata = use_state(|| true);

    let close_dialog = {
        let on_close = props.on_close.clone();
        Callback::from(move |_| on_close.emit(()))
    };

    let select_format = {
        let selected_format = selected_format.clone();
        Callback::from(move |format: ExportFormat| selected_format.set(format))
    };

    let toggle_metadata = {
        let include_metadata = include_metadata.clone();
        Callback::from(move |_| include_metadata.set(!*include_metadata))
    };

    let export_conversation = {
        let messages = props.messages.clone();
        let selected_format = selected_format.clone();
        let include_metadata = include_metadata.clone();
        let on_close = props.on_close.clone();

        Callback::from(move |_| {
            let content = generate_export_content(&messages, *selected_format, *include_metadata);
            let filename = format!("conversation{}", selected_format.extension());

            // Use web_sys to trigger download
            if let Ok(blob) =
                web_sys::Blob::new_with_str_sequence(&js_sys::Array::of1(&content.into()))
            {
                let url = web_sys::Url::create_object_url_with_blob(&blob).unwrap();
                if let Ok(document) = web_sys::window().unwrap().document() {
                    if let Ok(anchor) = document.create_element("a") {
                        let _ = anchor.set_attribute("href", &url);
                        let _ = anchor.set_attribute("download", &filename);
                        let _ = anchor.set_attribute("style", "display: none");
                        let _ = document.body().unwrap().append_child(&anchor);
                        let _ = anchor.dyn_ref::<web_sys::HtmlElement>().unwrap().click();
                        let _ = document.body().unwrap().remove_child(&anchor);
                    }
                }
                web_sys::Url::revoke_object_url(&url).unwrap();
            }

            on_close.emit(());
        })
    };

    html! {
        <div
            class="export-dialog-overlay"
            style="
                position: fixed;
                top: 0;
                left: 0;
                right: 0;
                bottom: 0;
                background: rgba(0, 0, 0, 0.5);
                display: flex;
                align-items: center;
                justify-content: center;
                z-index: 1000;
            "
            onclick={close_dialog.clone()}
        >
            <div
                class="export-dialog"
                style="
                    background: white;
                    border-radius: 0.5rem;
                    box-shadow: 0 20px 25px -5px rgba(0, 0, 0, 0.1);
                    max-width: 500px;
                    width: 90%;
                    max-height: 80vh;
                    overflow-y: auto;
                "
                onclick={Callback::from(|e: MouseEvent| e.stop_propagation())}
            >
                <div
                    class="dialog-header"
                    style="
                        padding: 1.5rem;
                        border-bottom: 1px solid #e5e7eb;
                        display: flex;
                        justify-content: space-between;
                        align-items: center;
                    "
                >
                    <h2 style="margin: 0; font-size: 1.25rem; font-weight: 600;">
                        {"Export Conversation"}
                    </h2>
                    <button
                        onclick={close_dialog}
                        style="
                            background: none;
                            border: none;
                            font-size: 1.5rem;
                            cursor: pointer;
                            color: #6b7280;
                        "
                    >
                        {"×"}
                    </button>
                </div>

                <div class="dialog-body" style="padding: 1.5rem;">
                    <div class="format-selection" style="margin-bottom: 1.5rem;">
                        <label style="display: block; margin-bottom: 0.5rem; font-weight: 500;">
                            {"Export Format"}
                        </label>
                        <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 0.5rem;">
                            { [ExportFormat::Markdown, ExportFormat::Json, ExportFormat::Html, ExportFormat::Pdf]
                                .iter()
                                .map(|format| {
                                    let format_clone = format.clone();
                                    let select_format = select_format.clone();
                                    let is_selected = *selected_format == format_clone;
                                    html! {
                                        <button
                                            key={format.as_str()}
                                            onclick={Callback::from(move |_| select_format.emit(format_clone.clone()))}
                                            style={format!("
                                                padding: 0.75rem;
                                                border: 2px solid {};
                                                border-radius: 0.375rem;
                                                background: {};
                                                color: {};
                                                cursor: pointer;
                                                text-align: center;
                                                transition: all 0.2s;
                                            ",
                                                if is_selected { "#3b82f6" } else { "#d1d5db" },
                                                if is_selected { "#3b82f6" } else { "white" },
                                                if is_selected { "white" } else { "#374151" }
                                            )}
                                        >
                                            {format.display_name()}
                                        </button>
                                    }
                                })
                                .collect::<Html>()
                            }
                        </div>
                    </div>

                    <div class="options" style="margin-bottom: 1.5rem;">
                        <label style="display: flex; align-items: center; cursor: pointer;">
                            <input
                                type="checkbox"
                                checked={*include_metadata}
                                onchange={toggle_metadata}
                                style="margin-right: 0.5rem;"
                            />
                            {"Include metadata (timestamps, model info, token counts)"}
                        </label>
                    </div>

                    <div class="summary" style="margin-bottom: 1.5rem; padding: 1rem; background: #f9fafb; border-radius: 0.375rem;">
                        <div style="font-size: 0.875rem; color: #6b7280;">
                            {format!("Exporting {} messages in {} format", props.messages.len(), selected_format.display_name())}
                        </div>
                    </div>
                </div>

                <div
                    class="dialog-footer"
                    style="
                        padding: 1.5rem;
                        border-top: 1px solid #e5e7eb;
                        display: flex;
                        justify-content: flex-end;
                        gap: 0.75rem;
                    "
                >
                    <button
                        onclick={close_dialog}
                        style="
                            padding: 0.5rem 1rem;
                            border: 1px solid #d1d5db;
                            border-radius: 0.375rem;
                            background: white;
                            cursor: pointer;
                        "
                    >
                        {"Cancel"}
                    </button>
                    <button
                        onclick={export_conversation}
                        disabled={props.messages.is_empty()}
                        style={format!("
                            padding: 0.5rem 1rem;
                            border: none;
                            border-radius: 0.375rem;
                            background: {};
                            color: white;
                            cursor: {};
                        ",
                            if props.messages.is_empty() { "#d1d5db" } else { "#3b82f6" },
                            if props.messages.is_empty() { "not-allowed" } else { "pointer" }
                        )}
                    >
                        {"Export"}
                    </button>
                </div>
            </div>
        </div>
    }
}

fn generate_export_content(
    messages: &[Message],
    format: ExportFormat,
    include_metadata: bool,
) -> String {
    match format {
        ExportFormat::Markdown => generate_markdown_content(messages, include_metadata),
        ExportFormat::Json => generate_json_content(messages, include_metadata),
        ExportFormat::Html => generate_html_content(messages, include_metadata),
        ExportFormat::Pdf => generate_markdown_content(messages, include_metadata), // PDF will be handled by browser
    }
}

fn generate_markdown_content(messages: &[Message], include_metadata: bool) -> String {
    let mut content = String::from("# Conversation Export\n\n");

    for message in messages {
        let role = match message.role {
            crate::ui::state::MessageRole::User => "User",
            crate::ui::state::MessageRole::Assistant => "Assistant",
            crate::ui::state::MessageRole::System => "System",
        };

        content.push_str(&format!("## {}\n\n", role));

        if include_metadata {
            content.push_str(&format!(
                "**Timestamp:** {}\n",
                message.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            ));
            if let Some(model) = &message.model {
                content.push_str(&format!("**Model:** {}\n", model));
            }
            if let Some(tokens) = message.tokens {
                content.push_str(&format!("**Tokens:** {}\n", tokens));
            }
            content.push_str("\n");
        }

        content.push_str(&message.content);
        content.push_str("\n\n---\n\n");
    }

    content
}

fn generate_json_content(messages: &[Message], include_metadata: bool) -> String {
    use serde_json::json;

    let messages_json: Vec<serde_json::Value> = messages
        .iter()
        .map(|msg| {
            let mut obj = json!({
                "role": match msg.role {
                    crate::ui::state::MessageRole::User => "user",
                    crate::ui::state::MessageRole::Assistant => "assistant",
                    crate::ui::state::MessageRole::System => "system",
                },
                "content": msg.content,
            });

            if include_metadata {
                if let Some(obj) = obj.as_object_mut() {
                    obj.insert("timestamp".to_string(), json!(msg.timestamp.to_rfc3339()));
                    if let Some(model) = &msg.model {
                        obj.insert("model".to_string(), json!(model));
                    }
                    if let Some(tokens) = msg.tokens {
                        obj.insert("tokens".to_string(), json!(tokens));
                    }
                }
            }

            obj
        })
        .collect();

    let export_data = json!({
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "format": "conversation",
        "messages": messages_json
    });

    serde_json::to_string_pretty(&export_data).unwrap_or_default()
}

fn generate_html_content(messages: &[Message], include_metadata: bool) -> String {
    let mut content = r#"<!DOCTYPE html>
<html lang="en">
<head>
    <meta charset="UTF-8">
    <meta name="viewport" content="width=device-width, initial-scale=1.0">
    <title>Conversation Export</title>
    <style>
        body { font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, sans-serif; line-height: 1.6; max-width: 800px; margin: 0 auto; padding: 2rem; }
        .message { margin-bottom: 2rem; padding: 1rem; border-radius: 0.5rem; }
        .user { background: #3b82f6; color: white; }
        .assistant { background: #f3f4f6; color: #1f2937; }
        .system { background: #fef3c7; color: #92400e; }
        .metadata { font-size: 0.875rem; opacity: 0.8; margin-bottom: 0.5rem; }
        .timestamp { font-size: 0.75rem; }
    </style>
</head>
<body>
    <h1>Conversation Export</h1>
"#.to_string();

    for message in messages {
        let (role_class, role_name) = match message.role {
            crate::ui::state::MessageRole::User => ("user", "User"),
            crate::ui::state::MessageRole::Assistant => ("assistant", "Assistant"),
            crate::ui::state::MessageRole::System => ("system", "System"),
        };

        content.push_str(&format!("<div class=\"message {}\">\n", role_class));
        content.push_str(&format!("<h3>{}</h3>\n", role_name));

        if include_metadata {
            content.push_str("<div class=\"metadata\">\n");
            content.push_str(&format!(
                "<span class=\"timestamp\">{}</span><br>\n",
                message.timestamp.format("%Y-%m-%d %H:%M:%S UTC")
            ));
            if let Some(model) = &message.model {
                content.push_str(&format!("Model: {}<br>\n", model));
            }
            if let Some(tokens) = message.tokens {
                content.push_str(&format!("Tokens: {}\n", tokens));
            }
            content.push_str("</div>\n");
        }

        // Simple HTML escaping and formatting
        let escaped_content = message
            .content
            .replace("&", "&")
            .replace("<", "<")
            .replace(">", ">")
            .replace("\n", "<br>");

        content.push_str(&format!("<div>{}</div>\n", escaped_content));
        content.push_str("</div>\n");
    }

    content.push_str("</body>\n</html>");
    content
}
