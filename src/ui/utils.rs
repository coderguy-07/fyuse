//! UI utilities for Yew components
//! Provides common utilities for the UI layer

use wasm_bindgen::JsValue;
use web_sys::{window, Storage, Window};
use yew::prelude::*;

/// Local storage utilities
pub struct LocalStorage;

impl LocalStorage {
    /// Get the local storage
    fn get_storage() -> Result<Storage, JsValue> {
        let window = web_sys::window().unwrap();
        window
            .local_storage()?
            .ok_or_else(|| JsValue::from_str("No local storage available"))
    }

    /// Get a value from local storage
    pub fn get(key: &str) -> Option<String> {
        Self::get_storage().ok()?.get_item(key).ok()?
    }

    /// Set a value in local storage
    pub fn set(key: &str, value: &str) -> Result<(), JsValue> {
        Self::get_storage()?.set_item(key, value)
    }

    /// Remove a value from local storage
    pub fn remove(key: &str) -> Result<(), JsValue> {
        Self::get_storage()?.remove_item(key)
    }

    /// Clear all local storage
    pub fn clear() -> Result<(), JsValue> {
        Self::get_storage()?.clear()
    }
}

/// Session storage utilities
pub struct SessionStorage;

impl SessionStorage {
    /// Get the session storage
    fn get_storage() -> Result<Storage, JsValue> {
        let window = web_sys::window().unwrap();
        window
            .session_storage()?
            .ok_or_else(|| JsValue::from_str("No session storage available"))
    }

    /// Get a value from session storage
    pub fn get(key: &str) -> Option<String> {
        Self::get_storage().ok()?.get_item(key).ok()?
    }

    /// Set a value in session storage
    pub fn set(key: &str, value: &str) -> Result<(), JsValue> {
        Self::get_storage()?.set_item(key, value)
    }

    /// Remove a value from session storage
    pub fn remove(key: &str) -> Result<(), JsValue> {
        Self::get_storage()?.remove_item(key)
    }

    /// Clear all session storage
    pub fn clear() -> Result<(), JsValue> {
        Self::get_storage()?.clear()
    }
}

/// DOM utilities
pub struct DomUtils;

impl DomUtils {
    /// Get the window object
    pub fn window() -> Window {
        web_sys::window().unwrap()
    }

    /// Get the document object
    pub fn document() -> web_sys::Document {
        Self::window().document().unwrap()
    }

    /// Copy text to clipboard
    pub async fn copy_to_clipboard(text: &str) -> Result<(), JsValue> {
        let navigator = Self::window().navigator();
        let clipboard = navigator.clipboard();
        clipboard.write_text(text).await
    }

    /// Get current URL
    pub fn current_url() -> String {
        Self::window().location().href().unwrap_or_default()
    }

    /// Scroll to top of page
    pub fn scroll_to_top() {
        Self::window().scroll_to_with_x_and_y(0.0, 0.0);
    }

    /// Get viewport dimensions
    pub fn viewport_size() -> (f64, f64) {
        let window = Self::window();
        let width = window.inner_width().unwrap().as_f64().unwrap();
        let height = window.inner_height().unwrap().as_f64().unwrap();
        (width, height)
    }
}

/// Event utilities
pub struct EventUtils;

impl EventUtils {
    /// Prevent default event behavior
    pub fn prevent_default(event: &Event) {
        event.prevent_default();
    }

    /// Stop event propagation
    pub fn stop_propagation(event: &Event) {
        event.stop_propagation();
    }

    /// Get target element value
    pub fn target_value(event: &Event) -> String {
        let target = event.target().unwrap();
        let input: web_sys::HtmlInputElement = target.dyn_into().unwrap();
        input.value()
    }
}

/// Component utilities
pub struct ComponentUtils;

impl ComponentUtils {
    /// Generate a unique ID for components
    pub fn generate_id(prefix: &str) -> String {
        use js_sys::Date;
        let timestamp = Date::now() as u64;
        format!("{}-{}", prefix, timestamp)
    }

    /// Debounce a function call
    pub fn debounce<F>(f: F, delay_ms: u32) -> Box<dyn Fn()>
    where
        F: Fn() + 'static,
    {
        let f = std::rc::Rc::new(std::cell::RefCell::new(Some(f)));
        let timeout_handle = std::rc::Rc::new(std::cell::RefCell::new(
            None::<gloo_timers::callback::Timeout>,
        ));

        Box::new(move || {
            let f = f.clone();
            let timeout_handle = timeout_handle.clone();

            // Cancel previous timeout
            if let Some(handle) = timeout_handle.borrow_mut().take() {
                handle.cancel();
            }

            // Set new timeout
            let handle = gloo_timers::callback::Timeout::new(delay_ms, move || {
                if let Some(f) = f.borrow_mut().take() {
                    f();
                }
            });

            *timeout_handle.borrow_mut() = Some(handle);
        })
    }
}

/// CSS class utilities
pub struct ClassUtils;

impl ClassUtils {
    /// Combine multiple CSS classes
    pub fn combine(classes: &[&str]) -> String {
        classes
            .iter()
            .filter(|s| !s.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join(" ")
    }

    /// Add conditional classes
    pub fn conditional(base: &str, condition: bool, class: &str) -> String {
        if condition {
            format!("{} {}", base, class)
        } else {
            base.to_string()
        }
    }
}
