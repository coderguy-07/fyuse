use crate::ui::components::file_upload::FileData;
use crate::ui::components::{ChatWindow, ExportDialog, FileUpload, InputArea, ModelSelector};
use crate::ui::state::AppState;
use yew::prelude::*;
use yew_router::prelude::*;

#[derive(Clone, Routable, PartialEq)]
enum Route {
    #[at("/")]
    Home,
    #[at("/chat")]
    Chat,
}

fn switch(routes: Route) -> Html {
    match routes {
        Route::Home => html! { <Home /> },
        Route::Chat => html! { <Chat /> },
    }
}

#[function_component(App)]
pub fn app() -> Html {
    html! {
        <BrowserRouter>
            <Switch<Route> render={switch} />
        </BrowserRouter>
    }
}

#[function_component(Home)]
fn home() -> Html {
    html! {
        <div class="min-h-screen bg-gradient-to-r from-indigo-100 to-purple-100 p-8">
            <div class="container mx-auto text-center">
                <h1 class="text-5xl font-bold text-gray-800 mb-8">
                    {"🚀 Fuse AI Model Management"}
                </h1>
                <p class="text-xl text-gray-600 mb-12">
                    {"Advanced AI model management platform"}
                </p>
                <div class="space-x-4">
                    <Link<Route> to={Route::Chat} classes="inline-block bg-purple-600 text-white px-8 py-3 rounded-lg hover:bg-purple-700 transition duration-300">
                        {"Start Chat"}
                    </Link<Route>>
                </div>
            </div>
        </div>
    }
}

#[function_component(Chat)]
fn chat() -> Html {
    let state = use_state(|| AppState::default());
    let show_export_dialog = use_state(|| false);
    let attached_files = use_state(|| Vec::<FileData>::new());

    html! {
        <div class="app-container" style="display: flex; flex-direction: column; height: 100vh; font-family: 'Inter', sans-serif;">

            // Header with model selector and actions
            <header class="app-header" style="padding: 1rem; border-bottom: 1px solid #e5e7eb; background: white;">
                <div style="display: flex; justify-content: space-between; align-items: center;">
                    <div style="display: flex; align-items: center; gap: 1rem;">
                        <Link<Route> to={Route::Home} classes="text-gray-600 hover:text-gray-800">
                            {"← Home"}
                        </Link<Route>>
                        <h1 style="font-size: 1.5rem; font-weight: bold; margin: 0;">
                            {"Fuse AI Chat"}
                        </h1>
                    </div>

                    <div style="display: flex; align-items: center; gap: 1rem;">
                        <button
                            onclick={Callback::from(move |_| show_export_dialog.set(true))}
                            disabled={state.messages.is_empty()}
                            style={format!("
                                padding: 0.5rem 1rem;
                                border: 1px solid #d1d5db;
                                border-radius: 0.375rem;
                                background: white;
                                cursor: {};
                                font-size: 0.875rem;
                                color: {};
                            ",
                                if state.messages.is_empty() { "not-allowed" } else { "pointer" },
                                if state.messages.is_empty() { "#9ca3af" } else { "#374151" }
                            )}
                        >
                            {"📤 Export"}
                        </button>

                        <ModelSelector
                            models={state.available_models.clone()}
                            selected={state.selected_model.clone()}
                            on_select={Callback::from(move |model: String| {
                                // TODO: Update state
                            })}
                        />
                    </div>
                </div>
            </header>

            // Main chat area
            <main class="chat-area" style="flex: 1; overflow: hidden; display: flex; flex-direction: column;">
                <ChatWindow
                    messages={state.messages.clone()}
                    is_streaming={state.is_streaming}
                />

                <InputArea
                    value={state.current_input.clone()}
                    disabled={state.is_streaming}
                    on_input={Callback::from(move |value: String| {
                        // TODO: Update state
                    })}
                    on_submit={Callback::from(move |_| {
                        // TODO: Handle message submission
                    })}
                />

                // File upload area
                <div style="padding: 1rem; border-top: 1px solid #e5e7eb; background: white;">
                    <FileUpload
                        on_file_selected={Callback::from(move |file: FileData| {
                            attached_files.set(vec![file]);
                        })}
                        disabled={state.is_streaming}
                        accepted_types={Some("image/*".to_string())}
                        max_size_bytes={Some(10 * 1024 * 1024)} // 10MB
                    />
                </div>
            </main>

            // Export dialog
            <ExportDialog
                is_open={*show_export_dialog}
                messages={state.messages.iter().cloned().collect::<Vec<_>>()}
                on_close={Callback::from(move |_| show_export_dialog.set(false))}
            />
        </div>
    }
}
