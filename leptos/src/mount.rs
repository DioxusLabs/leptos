use crate::IntoView;
#[cfg(debug_assertions)]
use crate::logging;
use any_spawner::Executor;
use reactive_graph::owner::Owner;
#[cfg(debug_assertions)]
use std::cell::Cell;
#[cfg(feature = "native")]
use tachys::renderer::types::Element as HtmlElement;
#[cfg(feature = "native")]
use tachys::view::any_view::{AnyView, IntoAny};
#[cfg(any(not(feature = "native"), feature = "hydrate"))]
use tachys::dom::body;
use tachys::view::{Mountable, Render};
#[cfg(feature = "hydrate")]
use tachys::{
    hydration::Cursor,
    view::{PositionState, RenderHtml},
};
#[cfg(feature = "hydrate")]
use wasm_bindgen::JsCast;
#[cfg(not(feature = "native"))]
use web_sys::HtmlElement;

#[cfg(feature = "hydrate")]
/// Hydrates the app described by the provided function, starting at `<body>`.
pub fn hydrate_body<F, N>(f: F)
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    let owner = hydrate_from(body(), f);
    owner.forget();
}

#[cfg(feature = "hydrate")]
/// Hydrates the app described by the provided function, starting at `<body>`, with support
/// for lazy-loaded routes and components.
pub fn hydrate_lazy<F, N>(f: F)
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    // use wasm-bindgen-futures to drive the reactive system
    // we ignore the return value because an Err here just means the wasm-bindgen executor is
    // already initialized, which is not an issue
    _ = Executor::init_wasm_bindgen();

    crate::task::spawn_local(async move {
        let owner = hydrate_from_async(body(), f).await;
        owner.forget();
    })
}

#[cfg(debug_assertions)]
thread_local! {
    static FIRST_CALL: Cell<bool> = const { Cell::new(true) };
}

#[cfg(feature = "hydrate")]
/// Runs the provided closure and mounts the result to the provided element.
pub fn hydrate_from<F, N>(parent: HtmlElement, f: F) -> UnmountHandle<N::State>
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    use hydration_context::HydrateSharedContext;
    use std::sync::Arc;

    // use wasm-bindgen-futures to drive the reactive system
    // we ignore the return value because an Err here just means the wasm-bindgen executor is
    // already initialized, which is not an issue
    _ = Executor::init_wasm_bindgen();

    #[cfg(debug_assertions)]
    {
        if !cfg!(feature = "hydrate") && FIRST_CALL.get() {
            logging::warn!(
                "It seems like you're trying to use Leptos in hydration mode, \
                 but the `hydrate` feature is not enabled on the `leptos` \
                 crate. Add `features = [\"hydrate\"]` to your Cargo.toml for \
                 the crate to work properly.\n\nNote that hydration and \
                 client-side rendering now use separate functions from \
                 leptos::mount: you are calling a hydration function."
            );
        }
        FIRST_CALL.set(false);
    }

    // create a new reactive owner and use it as the root node to run the app
    let owner = Owner::new_root(Some(Arc::new(HydrateSharedContext::new())));
    let mountable = owner.with(move || {
        let view = f().into_view();
        view.hydrate::<true>(
            &Cursor::new(parent.unchecked_into()),
            &PositionState::default(),
        )
    });

    if let Some(sc) = Owner::current_shared_context() {
        sc.hydration_complete();
    }

    // returns a handle that owns the owner
    // when this is dropped, it will clean up the reactive system and unmount the view
    UnmountHandle { owner, mountable }
}

#[cfg(feature = "hydrate")]
/// Runs the provided closure and mounts the result to the provided element.
pub async fn hydrate_from_async<F, N>(
    parent: HtmlElement,
    f: F,
) -> UnmountHandle<N::State>
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    use hydration_context::HydrateSharedContext;
    use std::sync::Arc;

    // use wasm-bindgen-futures to drive the reactive system
    // we ignore the return value because an Err here just means the wasm-bindgen executor is
    // already initialized, which is not an issue
    _ = Executor::init_wasm_bindgen();

    #[cfg(debug_assertions)]
    {
        if !cfg!(feature = "hydrate") && FIRST_CALL.get() {
            logging::warn!(
                "It seems like you're trying to use Leptos in hydration mode, \
                 but the `hydrate` feature is not enabled on the `leptos` \
                 crate. Add `features = [\"hydrate\"]` to your Cargo.toml for \
                 the crate to work properly.\n\nNote that hydration and \
                 client-side rendering now use separate functions from \
                 leptos::mount: you are calling a hydration function."
            );
        }
        FIRST_CALL.set(false);
    }

    // create a new reactive owner and use it as the root node to run the app
    let owner = Owner::new_root(Some(Arc::new(HydrateSharedContext::new())));
    let mountable = owner
        .with(move || {
            use reactive_graph::computed::ScopedFuture;

            ScopedFuture::new(async move {
                let view = f().into_view();
                view.hydrate_async(
                    &Cursor::new(parent.unchecked_into()),
                    &PositionState::default(),
                )
                .await
            })
        })
        .await;

    if let Some(sc) = Owner::current_shared_context() {
        sc.hydration_complete();
    }

    // returns a handle that owns the owner
    // when this is dropped, it will clean up the reactive system and unmount the view
    UnmountHandle { owner, mountable }
}

/// Runs the provided closure and mounts the result to the `<body>`.
#[cfg(not(feature = "native"))]
pub fn mount_to_body<F, N>(f: F)
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    let owner = mount_to(body(), f);
    owner.forget();
}

#[cfg(feature = "native")]
/// An application factory evaluated inside the native host's document scope.
pub type NativeApp = Box<dyn FnOnce() -> AnyView>;

#[cfg(feature = "native")]
/// A native host submitted through `inventory` by a linked launcher crate.
pub struct NativeLauncher {
    /// Opens the host and evaluates the application in its renderer scope.
    pub launch: fn(NativeApp),
}

#[cfg(feature = "native")]
inventory::collect!(NativeLauncher);

#[cfg(feature = "native")]
fn native_launcher<'a>(
    mut launchers: impl Iterator<Item = &'a NativeLauncher>,
) -> fn(NativeApp) {
    let launcher = launchers.next().expect(
        "mount_to_body with leptos/native requires a linked native host; \
         enable leptos_native/auto-launch and link leptos_native",
    );
    assert!(
        launchers.next().is_none(),
        "multiple native launchers registered"
    );
    launcher.launch
}

#[cfg(feature = "native")]
/// Launches the linked native host instead of mounting into a browser body.
///
/// Link `leptos_native` with its `auto-launch` feature to provide the host.
/// Exactly one host must register a [`NativeLauncher`].
pub fn mount_to_body<F, N>(f: F)
where
    F: FnOnce() -> N + 'static,
    N: IntoView + 'static,
{
    native_launcher(inventory::iter::<NativeLauncher>.into_iter())(Box::new(
        move || f().into_any(),
    ));
}

#[cfg(all(test, feature = "native"))]
mod native_launcher_tests {
    use super::*;

    #[test]
    fn selects_one_launcher_without_evaluating_the_app() {
        let launcher = NativeLauncher { launch: |_| {} };
        native_launcher(std::iter::once(&launcher))(Box::new(|| {
            panic!("host decides when the app is evaluated")
        }));
    }

    #[test]
    #[should_panic(expected = "requires a linked native host")]
    fn missing_launcher_has_actionable_error() {
        native_launcher(std::iter::empty());
    }

    #[test]
    #[should_panic(expected = "multiple native launchers")]
    fn rejects_multiple_launchers() {
        let launcher = NativeLauncher { launch: |_| {} };
        native_launcher([&launcher, &launcher].into_iter());
    }
}

/// Runs the provided closure and mounts the result to the provided element.
pub fn mount_to<F, N>(parent: HtmlElement, f: F) -> UnmountHandle<N::State>
where
    F: FnOnce() -> N + 'static,
    N: IntoView,
{
    // use wasm-bindgen-futures to drive the reactive system
    // we ignore the return value because an Err here just means the wasm-bindgen executor is
    // already initialized, which is not an issue
    _ = Executor::init_wasm_bindgen();

    #[cfg(debug_assertions)]
    {
        if !cfg!(feature = "csr") && FIRST_CALL.get() {
            logging::warn!(
                "It seems like you're trying to use Leptos in client-side \
                 rendering mode, but the `csr` feature is not enabled on the \
                 `leptos` crate. Add `features = [\"csr\"]` to your \
                 Cargo.toml for the crate to work properly.\n\nNote that \
                 hydration and client-side rendering now use different \
                 functions from leptos::mount. You are using a client-side \
                 rendering mount function."
            );
        }
        FIRST_CALL.set(false);
    }

    // create a new reactive owner and use it as the root node to run the app
    let owner = Owner::new();
    let mountable = owner.with(move || {
        let view = f().into_view();
        let mut mountable = view.build();
        mountable.mount(&parent, None);
        mountable
    });

    // returns a handle that owns the owner
    // when this is dropped, it will clean up the reactive system and unmount the view
    UnmountHandle { owner, mountable }
}

/// Runs the provided closure and mounts the result to the provided element.
pub fn mount_to_renderer<F, N>(
    parent: &tachys::renderer::types::Element,
    f: F,
) -> UnmountHandle<N::State>
where
    F: FnOnce() -> N + 'static,
    N: Render,
{
    // use wasm-bindgen-futures to drive the reactive system
    // we ignore the return value because an Err here just means the wasm-bindgen executor is
    // already initialized, which is not an issue
    _ = Executor::init_wasm_bindgen();

    // create a new reactive owner and use it as the root node to run the app
    let owner = Owner::new();
    let mountable = owner.with(move || {
        let view = f();
        let mut mountable = view.build();
        mountable.mount(parent, None);
        mountable
    });

    // returns a handle that owns the owner
    // when this is dropped, it will clean up the reactive system and unmount the view
    UnmountHandle { owner, mountable }
}

/// Hydrates any islands that are currently present on the page.
#[cfg(feature = "hydrate")]
pub fn hydrate_islands() {
    use hydration_context::{HydrateSharedContext, SharedContext};
    use std::sync::Arc;

    // use wasm-bindgen-futures to drive the reactive system
    // we ignore the return value because an Err here just means the wasm-bindgen executor is
    // already initialized, which is not an issue
    _ = Executor::init_wasm_bindgen();

    #[cfg(debug_assertions)]
    FIRST_CALL.set(false);

    // create a new reactive owner and use it as the root node to run the app
    let sc = HydrateSharedContext::new();
    sc.set_is_hydrating(false); // islands mode starts in "not hydrating"
    let owner = Owner::new_root(Some(Arc::new(sc)));
    owner.set();
    std::mem::forget(owner);
}

/// On drop, this will clean up the reactive [`Owner`] and unmount the view created by
/// [`mount_to`].
///
/// If you are using it to create the root of an application, you should use
/// [`UnmountHandle::forget`] to leak it.
#[must_use = "Dropping an `UnmountHandle` will unmount the view and cancel the \
              reactive system. You should either call `.forget()` to keep the \
              view permanently mounted, or store the `UnmountHandle` somewhere \
              and drop it when you'd like to unmount the view."]
pub struct UnmountHandle<M>
where
    M: Mountable,
{
    #[allow(dead_code)]
    owner: Owner,
    mountable: M,
}

impl<M> UnmountHandle<M>
where
    M: Mountable,
{
    /// Leaks the handle, preventing the reactive system from being cleaned up and the view from
    /// being unmounted. This should always be called when [`mount_to`] is used for the root of an
    /// application that should live for the long term.
    pub fn forget(self) {
        std::mem::forget(self);
    }
}

impl<M> Drop for UnmountHandle<M>
where
    M: Mountable,
{
    fn drop(&mut self) {
        self.mountable.unmount();
    }
}
