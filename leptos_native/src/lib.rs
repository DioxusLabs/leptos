//! Experimental Leptos host for Blitz's native DOM and AnyRender backends.
#![cfg(feature = "native")]

mod renderer;
use any_spawner::{CustomExecutor, Executor, PinnedFuture, PinnedLocalFuture};
use blitz_dom::{
    BaseDocument, DocGuard, DocGuardMut, Document, DocumentConfig, EventDriver,
    EventHandler,
};
use blitz_traits::{
    events::{BlitzInputEvent, DomEvent, DomEventData, EventState, UiEvent},
    node_id::NodeId,
    shell::{ColorScheme, Viewport},
};
use leptos::{
    prelude::*,
    tachys::renderer::native::{self, Event, Node},
};
pub use renderer::{LeptosNativeWindowRenderer, RendererOptions};
use std::{
    cell::{Cell, RefCell},
    path::Path,
    rc::Rc,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    task::{Context, Wake, Waker},
};

#[derive(Default)]
struct TaskWake {
    ready: AtomicBool,
    shell: Mutex<Option<Waker>>,
}
impl Wake for TaskWake {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        self.ready.store(true, Ordering::Release);
        if let Some(waker) = &*self.shell.lock().unwrap() {
            waker.wake_by_ref();
        }
    }
}
#[derive(Default)]
struct Tasks {
    futures: RefCell<Vec<PinnedLocalFuture<()>>>,
    wake: Arc<TaskWake>,
}
thread_local! {
    static CURRENT_TASKS: RefCell<Option<Rc<Tasks>>> = const { RefCell::new(None) };
    static EXECUTOR_INITIALIZED: Cell<bool> = const { Cell::new(false) };
}
struct NativeExecutor;
impl CustomExecutor for NativeExecutor {
    fn spawn(&self, future: PinnedFuture<()>) {
        self.spawn_local(future);
    }
    fn spawn_local(&self, future: PinnedLocalFuture<()>) {
        CURRENT_TASKS.with_borrow(|tasks| {
            let tasks = tasks
                .as_ref()
                .expect("native executor requires a current LeptosDocument");
            tasks.futures.borrow_mut().push(future);
            tasks.wake.wake_by_ref();
        });
    }
    fn poll_local(&self) {}
}
fn scoped<T>(
    doc: &mut BaseDocument,
    tasks: &Rc<Tasks>,
    f: impl FnOnce() -> T,
) -> T {
    struct Reset(Option<Rc<Tasks>>);
    impl Drop for Reset {
        fn drop(&mut self) {
            CURRENT_TASKS.set(self.0.take());
        }
    }
    let _reset = Reset(CURRENT_TASKS.replace(Some(tasks.clone())));
    native::with_document(doc, f)
}

/// Owns the Blitz DOM, reactive owner, rendered view, and its local task queue.
pub struct LeptosDocument {
    pub dom: BaseDocument,
    pub root: NodeId,
    owner: Owner,
    state: Option<Box<dyn Mountable>>,
    tasks: Rc<Tasks>,
    event_dirty: bool,
}
impl LeptosDocument {
    pub fn new<V: IntoView + 'static>(app: impl FnOnce() -> V) -> Self {
        if !EXECUTOR_INITIALIZED.get() {
            Executor::init_local_custom_executor(NativeExecutor).expect(
                "another local executor is already installed on the native UI \
                 thread",
            );
            EXECUTOR_INITIALIZED.set(true);
        }
        let mut dom = blitz_html::HtmlDocument::from_html(
            "<!doctype html><html><head></head><body><main \
             id='app'></main></body></html>",
            DocumentConfig {
                html_parser_provider: Some(Arc::new(blitz_html::HtmlProvider)),
                net_provider: Some(blitz_shell::DataUriNetProvider::shared(None)),
                viewport: Some(Viewport::new(
                    640,
                    160,
                    1.0,
                    ColorScheme::Light,
                )),
                ..Default::default()
            },
        )
        .into_inner();
        let root = find_node(&dom, dom.root_node().id, &|node| {
            node.attr("id".into()) == Some("app")
        })
        .unwrap();
        let tasks = Rc::new(Tasks::default());
        let owner = Owner::new();
        let state = scoped(&mut dom, &tasks, || {
            owner.with(|| {
                let mut state = app().into_view().build();
                state.mount(&Node(root), None);
                Box::new(state) as Box<dyn Mountable>
            })
        });
        let mut document = Self {
            dom,
            root,
            owner,
            state: Some(state),
            tasks,
            event_dirty: false,
        };
        document.poll(None);
        document
    }

    /// Dispatches a real Blitz DOM click, including default actions and bubbling.
    pub fn click(&mut self, target: NodeId) {
        let data = self
            .dom
            .get_node(target)
            .expect("click target is missing")
            .synthetic_click_event(Default::default());
        let mut driver = EventDriver::new(self, LeptosEventHandler);
        driver.handle_dom_event(DomEvent::new(target, data));
    }
    /// Sets an input's live value and dispatches its native input event.
    pub fn input(&mut self, target: NodeId, value: &str) {
        scoped(&mut self.dom, &self.tasks, || Node(target).set_value(value));
        EventDriver::new(self, LeptosEventHandler).handle_dom_event(
            DomEvent::new(
                target,
                DomEventData::Input(BlitzInputEvent {
                    value: value.to_owned(),
                }),
            ),
        );
    }
    /// Dispatches a synthetic native DOM event with Blitz's default actions.
    pub fn dispatch_event(&mut self, target: NodeId, data: DomEventData) {
        EventDriver::new(self, LeptosEventHandler)
            .handle_dom_event(DomEvent::new(target, data));
    }
    pub fn find_by_text(&self, tag: &str, text: &str) -> Option<NodeId> {
        find_node(&self.dom, self.root, &|node| {
            node.data
                .is_element_with_tag_name(&blitz_dom::LocalName::from(tag))
                && node.text_content() == text
        })
    }
    pub fn text(&self) -> String {
        self.dom.get_node(self.root).unwrap().text_content()
    }

    /// Resolves styles/layout and paints to a PNG without opening a window or GPU.
    pub fn render_png(&mut self, path: impl AsRef<Path>) {
        use anyrender::PaintScene;
        self.poll(None);
        self.dom.resolve(0.0);
        // Data URI decoding is synchronous; ingest the responses queued by style resolution.
        self.dom.resolve(0.0);
        let (width, height) = self.dom.viewport().window_size;
        let scale = self.dom.viewport().scale_f64();
        let buffer = anyrender::render_to_buffer::<
            anyrender_vello_cpu::VelloCpuImageRenderer,
            _,
        >(
            |scene| {
                scene.fill(
                    peniko::Fill::NonZero,
                    Default::default(),
                    blitz_dom::util::Color::WHITE,
                    Default::default(),
                    &peniko::kurbo::Rect::new(
                        0.0,
                        0.0,
                        width as f64,
                        height as f64,
                    ),
                );
                blitz_paint::paint_scene(
                    scene,
                    &mut self.dom,
                    scale,
                    width,
                    height,
                    0,
                    0,
                );
            },
            width,
            height,
        );
        let mut encoder = png::Encoder::new(
            std::fs::File::create(path).unwrap(),
            width,
            height,
        );
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&buffer)
            .unwrap();
    }
}
fn find_node(
    dom: &BaseDocument,
    id: NodeId,
    predicate: &impl Fn(&blitz_dom::Node) -> bool,
) -> Option<NodeId> {
    let node = dom.get_node(id)?;
    if predicate(node) {
        return Some(id);
    }
    node.children
        .iter()
        .find_map(|child| find_node(dom, *child, predicate))
}
impl Document for LeptosDocument {
    fn inner(&self) -> DocGuard<'_> {
        DocGuard::Ref(&self.dom)
    }
    fn inner_mut(&mut self) -> DocGuardMut<'_> {
        DocGuardMut::Ref(&mut self.dom)
    }
    fn handle_ui_event(&mut self, event: UiEvent) {
        EventDriver::new(self, LeptosEventHandler).handle_ui_event(event);
        self.poll(None);
    }
    fn poll(&mut self, context: Option<Context<'_>>) -> bool {
        if let Some(context) = context {
            *self.tasks.wake.shell.lock().unwrap() =
                Some(context.waker().clone());
        }
        let waker = Waker::from(self.tasks.wake.clone());
        let mut context = Context::from_waker(&waker);
        let mut changed = std::mem::take(&mut self.event_dirty);
        scoped(&mut self.dom, &self.tasks, || {
            while self.tasks.wake.ready.swap(false, Ordering::AcqRel) {
                changed = true;
                let futures =
                    std::mem::take(&mut *self.tasks.futures.borrow_mut());
                let mut pending = Vec::new();
                for mut future in futures {
                    if future.as_mut().poll(&mut context).is_pending() {
                        pending.push(future);
                    }
                }
                self.tasks.futures.borrow_mut().extend(pending);
            }
        });
        if changed {
            self.dom.shell_provider.request_redraw();
        }
        changed
    }
}
impl Drop for LeptosDocument {
    fn drop(&mut self) {
        scoped(&mut self.dom, &self.tasks, || {
            if let Some(mut state) = self.state.take() {
                state.unmount();
                drop(state);
            }
            self.owner.cleanup();
            self.tasks.futures.borrow_mut().clear();
        });
        native::dispose_document(self.dom.id());
    }
}
struct LeptosEventHandler;
impl EventHandler for LeptosEventHandler {
    fn handle_event(
        &mut self,
        chain: &[NodeId],
        event: &mut DomEvent,
        doc: &mut dyn Document,
        state: &mut EventState,
    ) {
        let event = Event::new(event.target, event.data.clone());
        let doc = (doc as &mut dyn std::any::Any)
            .downcast_mut::<LeptosDocument>()
            .expect("Leptos event handler requires a LeptosDocument");
        scoped(&mut doc.dom, &doc.tasks, || {
            native::dispatch(chain, &event);
            if matches!(event.data, DomEventData::Input(_)) {
                let target = event.target().unwrap();
                if target.tag_name() == "SELECT"
                    || matches!(
                        target.get_attribute("type").as_deref(),
                        Some("checkbox" | "radio")
                    )
                {
                    native::dispatch(
                        chain,
                        &Event::new(target.0, event.data.clone())
                            .with_name("change"),
                    );
                }
            }
            if matches!(event.data, DomEventData::Click(_))
                && !event.default_prevented()
            {
                for &id in chain {
                    let node = Node(id);
                    if node.node_type() == 1 && node.tag_name() == "A" {
                        if let Some(href) = node
                            .get_attribute("href")
                            .filter(|href| href.starts_with('#'))
                        {
                            event.prevent_default();
                            native::navigate_hash(&href);
                        }
                        break;
                    }
                }
            }
        });
        if event.default_prevented() {
            state.prevent_default();
        }
        if event.propagation_stopped() {
            state.stop_propagation();
        }
        doc.event_dirty = true;
    }
}

#[cfg(feature = "auto-launch")]
inventory::submit! {
    leptos::mount::NativeLauncher { launch: launch_registered }
}

#[cfg(feature = "auto-launch")]
fn launch_registered(app: leptos::mount::NativeApp) {
    launch(app);
}

/// Opens a native Blitz window using the feature-selected renderer.
///
/// Without a renderer feature, uses Vello CPU with softbuffer. When multiple
/// backends are enabled, priority is Vello, Vello Hybrid, Skia GPU, Skia raster,
/// then Vello CPU. Pixels takes precedence over softbuffer for raster backends.
///
/// Set `LEPTOS_NATIVE_HEADLESS` to a PNG path to render one frame and exit
/// without creating a window or requiring a display server.
pub fn launch<V: IntoView + 'static>(app: impl FnOnce() -> V) {
    if let Some(path) = std::env::var_os("LEPTOS_NATIVE_HEADLESS") {
        let mut document = LeptosDocument::new(app);
        document.render_png(&path);
        println!("Native headless frame: {}", Path::new(&path).display());
        return;
    }
    let event_loop = blitz_shell::create_default_event_loop();
    let (proxy, receiver) =
        blitz_shell::BlitzShellProxy::new(event_loop.create_proxy());
    let mut application = blitz_shell::BlitzApplication::new(proxy, receiver);
    let document = LeptosDocument::new(app);
    let renderer = LeptosNativeWindowRenderer::new();
    application.add_window(blitz_shell::WindowConfig::new(
        Box::new(document),
        renderer,
    ));
    event_loop.run_app(application).unwrap();
}
