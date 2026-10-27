#[cfg(not(feature = "native"))]
use wasm_bindgen::JsCast;
#[cfg(not(feature = "native"))]
use web_sys::HtmlElement;
use web_sys::{Document, Window};

thread_local! {
    pub(crate) static WINDOW: web_sys::Window = web_sys::window().unwrap();

    pub(crate) static DOCUMENT: web_sys::Document = web_sys::window().unwrap().document().unwrap();
}

/// Returns the [`Window`](https://developer.mozilla.org/en-US/docs/Web/API/Window).
///
/// This is cached as a thread-local variable, so calling `window()` multiple times
/// requires only one call out to JavaScript.
pub fn window() -> Window {
    WINDOW.with(Clone::clone)
}

/// Returns the [`Document`](https://developer.mozilla.org/en-US/docs/Web/API/Document).
///
/// This is cached as a thread-local variable, so calling `document()` multiple times
/// requires only one call out to JavaScript.
///
/// ## Panics
/// Panics if called outside a browser environment.
pub fn document() -> Document {
    DOCUMENT.with(Clone::clone)
}

/// The `<body>` element.
///
/// ## Panics
/// Panics if there is no `<body>` in the current document, or if it is called outside a browser
/// environment.
#[cfg(not(feature = "native"))]
pub fn body() -> HtmlElement {
    document().body().unwrap()
}

/// Helper function to extract [`Event.target`](https://developer.mozilla.org/en-US/docs/Web/API/Event/target)
/// from any event.
#[cfg(not(feature = "native"))]
pub fn event_target<T>(event: &web_sys::Event) -> T
where
    T: JsCast,
{
    event.target().unwrap().unchecked_into::<T>()
}

/// Helper function to extract `event.target.value` from an event.
///
/// This is useful in the `on:input` or `on:change` listeners for an `<input>` element.
#[cfg(not(feature = "native"))]
pub fn event_target_value<T>(event: &T) -> String
where
    T: JsCast,
{
    event
        .unchecked_ref::<web_sys::Event>()
        .target()
        .unwrap()
        .unchecked_into::<web_sys::HtmlInputElement>()
        .value()
}

/// Helper function to extract `event.target.checked` from an event.
///
/// This is useful in the `on:change` listeners for an `<input type="checkbox">` element.
#[cfg(not(feature = "native"))]
pub fn event_target_checked(ev: &web_sys::Event) -> bool {
    ev.target()
        .unwrap()
        .unchecked_into::<web_sys::HtmlInputElement>()
        .checked()
}

#[cfg(feature = "native")]
/// Native applications must supply a mount root explicitly.
pub fn body() -> crate::renderer::types::Element {
    unimplemented!("native apps must mount to an explicit root element")
}
#[cfg(feature = "native")]
/// Extracts a native event target.
pub fn event_target<
    T: crate::renderer::CastFrom<crate::renderer::types::Element>,
>(
    event: &crate::renderer::native::Event,
) -> T {
    crate::renderer::Rndr::event_target(event)
}
#[cfg(feature = "native")]
/// Reads the target's value attribute.
pub fn event_target_value(event: &crate::renderer::native::Event) -> String {
    event
        .target()
        .unwrap()
        .get_attribute("value")
        .unwrap_or_default()
}
#[cfg(feature = "native")]
/// Reads the target's checked attribute.
pub fn event_target_checked(event: &crate::renderer::native::Event) -> bool {
    event
        .target()
        .unwrap()
        .get_attribute("checked")
        .is_some_and(|v| v != "false")
}
