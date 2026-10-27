//! Experimental Leptos host for Blitz's native DOM and CPU renderer.

use any_spawner::{CustomExecutor, Executor, PinnedFuture, PinnedLocalFuture};
use blitz_dom::{
    BaseDocument, DocGuard, DocGuardMut, Document, DocumentConfig, EventDriver,
    EventHandler,
};
use blitz_traits::{
    events::{DomEvent, EventState, UiEvent},
    node_id::NodeId,
    shell::{ColorScheme, Viewport},
};
use leptos::{
    prelude::*,
    tachys::renderer::native::{self, Event, Node},
};
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
        self.dom.resolve(0.0);
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
                    &peniko::kurbo::Rect::new(0.0, 0.0, 640.0, 160.0),
                );
                blitz_paint::paint_scene(
                    scene,
                    &mut self.dom,
                    1.0,
                    640,
                    160,
                    0,
                    0,
                );
            },
            640,
            160,
        );
        let mut encoder =
            png::Encoder::new(std::fs::File::create(path).unwrap(), 640, 160);
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
    }
    fn poll(&mut self, context: Option<Context<'_>>) -> bool {
        if let Some(context) = context {
            *self.tasks.wake.shell.lock().unwrap() =
                Some(context.waker().clone());
        }
        let waker = Waker::from(self.tasks.wake.clone());
        let mut context = Context::from_waker(&waker);
        let mut changed = false;
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
        scoped(&mut doc.dom, &doc.tasks, || native::dispatch(chain, &event));
        if event.default_prevented() {
            state.prevent_default();
        }
        if event.propagation_stopped() {
            state.stop_propagation();
        }
    }
}

/// Opens a native Blitz window using its CPU softbuffer renderer.
pub fn launch<V: IntoView + 'static>(app: impl FnOnce() -> V) {
    let event_loop = blitz_shell::create_default_event_loop();
    let (proxy, receiver) =
        blitz_shell::BlitzShellProxy::new(event_loop.create_proxy());
    let mut application = blitz_shell::BlitzApplication::new(proxy, receiver);
    let document = LeptosDocument::new(app);
    let renderer = anyrender_vello_cpu::VelloCpuWindowRenderer::new();
    application.add_window(blitz_shell::WindowConfig::new(
        Box::new(document),
        renderer,
    ));
    event_loop.run_app(application).unwrap();
}
