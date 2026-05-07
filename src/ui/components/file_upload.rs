use crate::ui::utils::DomUtils;
use web_sys::{File, FileList};
use yew::prelude::*;

#[derive(Properties, PartialEq)]
pub struct FileUploadProps {
    pub on_file_selected: Callback<FileData>,
    pub disabled: bool,
    pub accepted_types: Option<String>,
    pub max_size_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FileData {
    pub name: String,
    pub size: u64,
    pub data: Vec<u8>,
    pub mime_type: String,
}

#[function_component(FileUpload)]
pub fn file_upload(props: &FileUploadProps) -> Html {
    let drag_over = use_state(|| false);
    let selected_file = use_state(|| None::<FileData>);
    let error_message = use_state(|| None::<String>);

    let on_drag_over = {
        let drag_over = drag_over.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            drag_over.set(true);
        })
    };

    let on_drag_leave = {
        let drag_over = drag_over.clone();
        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            drag_over.set(false);
        })
    };

    let on_drop = {
        let drag_over = drag_over.clone();
        let on_file_selected = props.on_file_selected.clone();
        let error_message = error_message.clone();
        let selected_file = selected_file.clone();
        let max_size_bytes = props.max_size_bytes;

        Callback::from(move |e: DragEvent| {
            e.prevent_default();
            drag_over.set(false);

            if let Some(files) = e.data_transfer() {
                if let Some(file_list) = files.files() {
                    process_files(
                        file_list,
                        on_file_selected.clone(),
                        error_message.clone(),
                        selected_file.clone(),
                        max_size_bytes,
                    );
                }
            }
        })
    };

    let on_file_input = {
        let on_file_selected = props.on_file_selected.clone();
        let error_message = error_message.clone();
        let selected_file = selected_file.clone();
        let max_size_bytes = props.max_size_bytes;

        Callback::from(move |e: Event| {
            let input: web_sys::HtmlInputElement = e.target_unchecked_into();
            if let Some(files) = input.files() {
                process_files(
                    files,
                    on_file_selected.clone(),
                    error_message.clone(),
                    selected_file.clone(),
                    max_size_bytes,
                );
            }
        })
    };

    let clear_file = {
        let selected_file = selected_file.clone();
        let error_message = error_message.clone();
        Callback::from(move |_| {
            selected_file.set(None);
            error_message.set(None);
        })
    };

    let accepted_types = props.accepted_types.as_deref().unwrap_or("image/*");
    let max_size_display = props
        .max_size_bytes
        .map(|size| format!(" (max {} MB)", size / (1024 * 1024)))
        .unwrap_or_default();

    html! {
        <div class="file-upload">
            <div
                class="drop-zone"
                style={format!("
                    border: 2px dashed {};
                    border-radius: 0.5rem;
                    padding: 2rem;
                    text-align: center;
                    cursor: {};
                    background: {};
                    transition: all 0.2s;
                    position: relative;
                ",
                    if *drag_over { "#3b82f6" } else { "#d1d5db" },
                    if props.disabled { "not-allowed" } else { "pointer" },
                    if *drag_over { "#eff6ff" } else { "white" }
                )}
                ondragover={on_drag_over}
                ondragleave={on_drag_leave}
                ondrop={on_drop}
                onclick={Callback::from(move |_| {
                    // Trigger file input click
                    if let Ok(document) = web_sys::window().unwrap().document() {
                        if let Ok(input) = document.query_selector("input[type='file']") {
                            if let Some(input) = input {
                                let _ = input.dyn_ref::<web_sys::HtmlElement>().unwrap().click();
                            }
                        }
                    }
                })}
            >
                <input
                    type="file"
                    accept={accepted_types}
                    disabled={props.disabled}
                    onchange={on_file_input}
                    style="display: none;"
                />

                <div style="margin-bottom: 1rem;">
                    <span style="font-size: 2rem;">{"📁"}</span>
                </div>

                <div style="margin-bottom: 0.5rem;">
                    <strong>{"Drop files here or click to browse"}</strong>
                </div>

                <div style="font-size: 0.875rem; color: #6b7280;">
                    {format!("Supported formats: {}{}", accepted_types, max_size_display)}
                </div>

                { if let Some(file) = (*selected_file).as_ref() {
                    html! {
                        <div
                            class="file-preview"
                            style="
                                margin-top: 1rem;
                                padding: 1rem;
                                background: #f9fafb;
                                border-radius: 0.375rem;
                                display: flex;
                                align-items: center;
                                justify-content: space-between;
                            "
                        >
                            <div style="display: flex; align-items: center; gap: 0.75rem;">
                                <span>{"📄"}</span>
                                <div>
                                    <div style="font-weight: 500;">{&file.name}</div>
                                    <div style="font-size: 0.75rem; color: #6b7280;">
                                        {format_size(file.size)}
                                    </div>
                                </div>
                            </div>
                            <button
                                onclick={clear_file}
                                style="
                                    background: none;
                                    border: none;
                                    cursor: pointer;
                                    color: #6b7280;
                                    font-size: 1.25rem;
                                "
                            >
                                {"×"}
                            </button>
                        </div>
                    }
                } else {
                    html! {}
                }}

                { if let Some(error) = (*error_message).as_ref() {
                    html! {
                        <div
                            class="error-message"
                            style="
                                margin-top: 1rem;
                                padding: 0.75rem;
                                background: #fef2f2;
                                border: 1px solid #fecaca;
                                border-radius: 0.375rem;
                                color: #dc2626;
                                font-size: 0.875rem;
                            "
                        >
                            {error}
                        </div>
                    }
                } else {
                    html! {}
                }}
            </div>
        </div>
    }
}

fn process_files(
    files: FileList,
    on_file_selected: Callback<FileData>,
    error_message: UseStateHandle<Option<String>>,
    selected_file: UseStateHandle<Option<FileData>>,
    max_size_bytes: Option<u64>,
) {
    if files.length() == 0 {
        return;
    }

    let file = files.get(0).unwrap();

    // Check file size
    if let Some(max_size) = max_size_bytes {
        if file.size() as u64 > max_size {
            error_message.set(Some(format!(
                "File size exceeds maximum allowed size of {} MB",
                max_size / (1024 * 1024)
            )));
            return;
        }
    }

    // Read file as array buffer
    let file_reader = web_sys::FileReader::new().unwrap();
    let file_reader_clone = file_reader.clone();

    let onload = Closure::wrap(Box::new(move || {
        let array_buffer = file_reader_clone.result().unwrap();
        let uint8_array = js_sys::Uint8Array::new(&array_buffer);
        let mut data = vec![0; uint8_array.length() as usize];
        uint8_array.copy_to(&mut data);

        let file_data = FileData {
            name: file.name(),
            size: file.size() as u64,
            data,
            mime_type: file.type_(),
        };

        selected_file.set(Some(file_data.clone()));
        error_message.set(None);
        on_file_selected.emit(file_data);
    }) as Box<dyn Fn()>);

    file_reader.set_onload(Some(onload.as_ref().unchecked_ref()));
    file_reader.read_as_array_buffer(&file).unwrap();

    // Keep the closure alive
    onload.forget();
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
