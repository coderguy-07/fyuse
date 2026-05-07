use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct InputAreaProps {
    pub value: String,
    pub disabled: bool,
    pub on_input: Callback<String>,
    pub on_submit: Callback<()>,
}

#[function_component(InputArea)]
pub fn input_area(props: &InputAreaProps) -> Html {
    let on_input = {
        let on_input = props.on_input.clone();
        Callback::from(move |e: InputEvent| {
            let input: web_sys::HtmlTextAreaElement = e.target_unchecked_into();
            on_input.emit(input.value());
        })
    };

    let on_keydown = {
        let on_submit = props.on_submit.clone();
        let value = props.value.clone();
        let disabled = props.disabled;
        Callback::from(move |e: KeyboardEvent| {
            if e.key() == "Enter" && !e.shift_key() {
                e.prevent_default();
                if !value.is_empty() && !disabled {
                    on_submit.emit(());
                }
            }
        })
    };

    let on_submit_click = {
        let on_submit = props.on_submit.clone();
        let value = props.value.clone();
        let disabled = props.disabled;
        Callback::from(move |_| {
            if !disabled && !value.is_empty() {
                on_submit.emit(());
            }
        })
    };

    html! {
        <div
            class="input-area"
            style="
                padding: 1rem;
                border-top: 1px solid #e5e7eb;
                background: white;
            "
        >
            <div
                class="input-container"
                style="
                    max-width: 800px;
                    margin: 0 auto;
                    display: flex;
                    gap: 0.5rem;
                    align-items: flex-end;
                "
            >
                <div class="textarea-wrapper" style="flex: 1; position: relative;">
                    <textarea
                        class="message-input"
                        style="
                            width: 100%;
                            min-height: 60px;
                            max-height: 200px;
                            padding: 0.75rem;
                            border: 1px solid #d1d5db;
                            border-radius: 0.5rem;
                            resize: vertical;
                            font-family: inherit;
                            font-size: 1rem;
                            outline: none;
                            transition: border-color 0.2s;
                        "
                        placeholder="Type your message... (Shift+Enter for new line)"
                        disabled={props.disabled}
                        value={props.value.clone()}
                        oninput={on_input}
                        onkeydown={on_keydown}
                    />

                    <div
                        class="input-hint"
                        style="
                            position: absolute;
                            bottom: -1.5rem;
                            right: 0;
                            font-size: 0.75rem;
                            color: #6b7280;
                        "
                    >
                        {"Press Enter to send, Shift+Enter for new line"}
                    </div>
                </div>

                <button
                    class="send-button"
                    style={format!("
                        padding: 0.75rem 1.5rem;
                        background: {};
                        color: white;
                        border: none;
                        border-radius: 0.5rem;
                        cursor: {};
                        font-weight: 500;
                        transition: background-color 0.2s;
                        min-width: 80px;
                    ",
                        if props.disabled || props.value.is_empty() { "#d1d5db" } else { "#3b82f6" },
                        if props.disabled || props.value.is_empty() { "not-allowed" } else { "pointer" }
                    )}
                    disabled={props.disabled || props.value.is_empty()}
                    onclick={on_submit_click}
                >
                    {if props.disabled {
                        "Sending..."
                    } else {
                        "Send"
                    }}
                </button>
            </div>

            <div
                class="file-upload-area"
                style="
                    max-width: 800px;
                    margin: 1rem auto 0;
                    display: flex;
                    gap: 0.5rem;
                "
            >
                <button
                    class="attach-button"
                    style="
                        padding: 0.5rem 1rem;
                        background: white;
                        border: 1px solid #d1d5db;
                        border-radius: 0.375rem;
                        cursor: pointer;
                        display: flex;
                        align-items: center;
                        gap: 0.5rem;
                        font-size: 0.875rem;
                        color: #6b7280;
                    "
                    disabled={props.disabled}
                >
                    <span>{"📎"}</span>
                    <span>{"Attach Image"}</span>
                </button>

                <div
                    class="file-info"
                    style="
                        flex: 1;
                        font-size: 0.875rem;
                        color: #6b7280;
                        display: flex;
                        align-items: center;
                    "
                >
                    {"Supports PNG, JPG, GIF, WebP (max 10MB)"}
                </div>
            </div>
        </div>
    }
}
