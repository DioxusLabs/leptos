#![allow(missing_docs)]

use super::{CastFrom, RemoveEventHandler};
use crate::view::{Mountable, ToTemplate};
use blitz_dom::{BaseDocument, QualName};
use blitz_traits::{events::DomEventData, node_id::NodeId};
use std::{
    borrow::Cow,
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Node(pub NodeId);
pub type Element = Node;
pub type Text = Node;
pub type Placeholder = Node;
pub type ClassList = Node;
pub type CssStyleDeclaration = Node;
pub type TemplateElement = Node;

thread_local! {
    static DOCUMENT: Cell<*mut BaseDocument> = const { Cell::new(std::ptr::null_mut()) };
    static ACCESSING: Cell<bool> = const { Cell::new(false) };
    static LISTENERS: RefCell<HashMap<(usize, NodeId, String), Vec<(usize, Rc<RefCell<Box<dyn FnMut(Event)>>>)>>> = RefCell::new(HashMap::new());
    static NEXT_LISTENER: Cell<usize> = const { Cell::new(0) };
}

/// Runs application code with access to this document. The pointer never escapes
/// this scope, and renderer operations cannot recursively borrow the document.
pub fn with_document<T>(doc: &mut BaseDocument, f: impl FnOnce() -> T) -> T {
    struct Reset(*mut BaseDocument);
    impl Drop for Reset {
        fn drop(&mut self) {
            DOCUMENT.set(self.0);
        }
    }
    assert!(
        !ACCESSING.get(),
        "native document is already borrowed by the renderer"
    );
    let _reset = Reset(DOCUMENT.replace(doc));
    f()
}

fn document<T>(f: impl FnOnce(&mut BaseDocument) -> T) -> T {
    struct Reset;
    impl Drop for Reset {
        fn drop(&mut self) {
            ACCESSING.set(false);
        }
    }
    let ptr = DOCUMENT.get();
    assert!(
        !ptr.is_null(),
        "native renderer operation requires a current document; use \
         with_document"
    );
    assert!(
        !ACCESSING.replace(true),
        "recursive native renderer document access"
    );
    let _reset = Reset;
    // SAFETY: with_document holds the unique borrow for the entire scope;
    // ACCESSING prevents overlapping borrows within renderer operations.
    f(unsafe { &mut *ptr })
}

#[derive(Debug, Clone)]
pub struct Event {
    pub data: DomEventData,
    target: Node,
    prevented: Rc<Cell<bool>>,
    stopped: Rc<Cell<bool>>,
}
impl Event {
    pub fn new(target: NodeId, data: DomEventData) -> Self {
        Self {
            data,
            target: Node(target),
            prevented: Default::default(),
            stopped: Default::default(),
        }
    }
    pub fn target(&self) -> Option<Element> {
        Some(self.target)
    }
    pub fn prevent_default(&self) {
        self.prevented.set(true);
    }
    pub fn default_prevented(&self) -> bool {
        self.prevented.get()
    }
    pub fn stop_propagation(&self) {
        self.stopped.set(true);
    }
    pub fn propagation_stopped(&self) -> bool {
        self.stopped.get()
    }
}

pub fn dispatch(chain: &[NodeId], event: &Event) {
    let doc = document(|doc| doc.id());
    for &node in chain {
        let callbacks = LISTENERS.with_borrow(|listeners| {
            listeners
                .get(&(doc, node, event.data.name().into()))
                .cloned()
                .unwrap_or_default()
        });
        for (_, callback) in callbacks {
            callback.borrow_mut()(event.clone());
        }
        if event.propagation_stopped() {
            break;
        }
    }
}

pub fn dispose_document(id: usize) {
    LISTENERS.with_borrow_mut(|listeners| {
        listeners.retain(|(doc, _, _), _| *doc != id)
    });
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyValue(Option<String>);
impl PropertyValue {
    pub const UNDEFINED: Self = Self(None);
    pub fn from_str(value: &str) -> Self {
        Self(Some(value.into()))
    }
}
impl<T: Into<PropertyValue>> From<Option<T>> for PropertyValue {
    fn from(value: Option<T>) -> Self {
        value.map(Into::into).unwrap_or(Self(None))
    }
}
macro_rules! values {
    ($($ty:ty),*) => { $(impl From<$ty> for PropertyValue {
        fn from(value: $ty) -> Self { Self(Some(value.to_string())) }
    })* };
}
values!(
    String, &String, &str, usize, u8, u16, u32, u64, u128, isize, i8, i16, i32,
    i64, i128, f32, f64, bool
);

impl AsRef<Node> for Node {
    fn as_ref(&self) -> &Node {
        self
    }
}
impl CastFrom<Node> for Node {
    fn cast_from(node: Node) -> Option<Self> {
        Some(node)
    }
}
impl Node {
    pub fn node_type(&self) -> u16 {
        document(|d| {
            let n = d.get_node(self.0).unwrap();
            if n.is_element() {
                1
            } else if n.is_text_node() {
                3
            } else {
                8
            }
        })
    }
    pub fn text_content(&self) -> Option<String> {
        document(|d| d.get_node(self.0).map(|n| n.text_content()))
    }
    pub fn get_attribute(&self, name: &str) -> Option<String> {
        document(|d| {
            d.get_node(self.0)
                .and_then(|n| n.attr(name.into()))
                .map(str::to_owned)
        })
    }
    pub fn value(&self) -> String {
        document(|d| {
            let node =
                d.get_node(self.0).expect("native value target is missing");
            node.element_data()
                .and_then(|element| element.text_input_data())
                .map(|input| input.editor.raw_text().to_string())
                .unwrap_or_else(|| {
                    node.attr("value".into()).unwrap_or_default().to_owned()
                })
        })
    }
    pub fn set_value(&self, value: &str) {
        NativeDom::set_property(self, "value", &PropertyValue::from_str(value));
    }
    pub fn tag_name(&self) -> String {
        document(|d| {
            d.mutate()
                .element_name(self.0)
                .map(|n| n.local.to_string().to_uppercase())
                .unwrap_or_default()
        })
    }
}
impl Mountable for Node {
    fn unmount(&mut self) {
        NativeDom::remove(self);
    }
    fn mount(&mut self, parent: &Element, marker: Option<&Node>) {
        NativeDom::insert_node(parent, self, marker);
    }
    fn insert_before_this(&self, child: &mut dyn Mountable) -> bool {
        if let Some(parent) = NativeDom::get_parent(self) {
            child.mount(&parent, Some(self));
            true
        } else {
            false
        }
    }
    fn elements(&self) -> Vec<Element> {
        if self.node_type() == 1 {
            vec![*self]
        } else {
            vec![]
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct NativeDom;
pub type Dom = NativeDom;
fn name(value: &str) -> QualName {
    QualName::new(None, "".into(), value.into())
}
impl NativeDom {
    pub fn intern(text: &str) -> &str {
        text
    }
    pub fn create_element(tag: &str, namespace: Option<&str>) -> Element {
        document(|d| {
            Node(d.mutate().create_element(
                QualName::new(
                    None,
                    namespace.unwrap_or("http://www.w3.org/1999/xhtml").into(),
                    tag.into(),
                ),
                vec![],
            ))
        })
    }
    pub fn create_text_node(text: &str) -> Text {
        document(|d| Node(d.mutate().create_text_node(text)))
    }
    pub fn create_placeholder() -> Placeholder {
        document(|d| Node(d.mutate().create_comment_node("")))
    }
    pub fn set_text(node: &Text, text: &str) {
        document(|d| d.mutate().set_node_text(node.0, text));
    }
    pub fn set_attribute(node: &Element, key: &str, value: &str) {
        document(|d| d.mutate().set_attribute(node.0, name(key), value));
    }
    pub fn remove_attribute(node: &Element, key: &str) {
        document(|d| d.mutate().clear_attribute(node.0, name(key)));
    }
    pub fn insert_node(parent: &Element, child: &Node, marker: Option<&Node>) {
        document(|d| {
            let mut m = d.mutate();
            if let Some(marker) = marker {
                m.insert_nodes_before(marker.0, &[child.0]);
            } else {
                m.append_children(parent.0, &[child.0]);
            }
        });
    }
    pub fn try_insert_node(
        parent: &Element,
        child: &Node,
        marker: Option<&Node>,
    ) -> bool {
        Self::insert_node(parent, child, marker);
        true
    }
    pub fn remove_node(parent: &Element, child: &Node) -> Option<Node> {
        if Self::get_parent(child) == Some(*parent) {
            Self::remove(child);
            Some(*child)
        } else {
            None
        }
    }
    pub fn remove(node: &Node) {
        document(|d| d.mutate().remove_node(node.0));
    }
    pub fn get_parent(node: &Node) -> Option<Node> {
        document(|d| d.mutate().parent_id(node.0).map(Node))
    }
    pub fn first_child(node: &Node) -> Option<Node> {
        document(|d| d.mutate().child_ids(node.0).first().copied().map(Node))
    }
    pub fn next_sibling(node: &Node) -> Option<Node> {
        document(|d| d.mutate().next_sibling_id(node.0).map(Node))
    }
    pub fn log_node(node: &Node) {
        eprintln!("{node:?}");
    }
    pub fn clear_children(parent: &Element) {
        document(|d| {
            let mut m = d.mutate();
            for child in m.child_ids(parent.0) {
                m.remove_node(child);
            }
        });
    }
    pub fn mount_before<M: Mountable>(child: &mut M, before: &Node) {
        assert!(
            Self::try_mount_before(child, before),
            "native mount marker has no parent"
        );
    }
    pub fn try_mount_before<M: Mountable>(
        child: &mut M,
        before: &Node,
    ) -> bool {
        before.insert_before_this(child)
    }
    pub fn set_property_or_value(
        el: &Element,
        key: &str,
        value: &PropertyValue,
    ) {
        Self::set_property(el, key, value);
    }
    pub fn set_property(el: &Element, key: &str, value: &PropertyValue) {
        if let Some(value) = &value.0 {
            Self::set_attribute(el, key, value);
        } else {
            Self::remove_attribute(el, key);
        }
    }
    pub fn add_event_listener(
        el: &Element,
        name: &str,
        cb: Box<dyn FnMut(Event)>,
    ) -> RemoveEventHandler<Element> {
        let key = (document(|d| d.id()), el.0, name.to_owned());
        let id = NEXT_LISTENER.get();
        NEXT_LISTENER.set(id + 1);
        LISTENERS.with_borrow_mut(|l| {
            l.entry(key.clone())
                .or_default()
                .push((id, Rc::new(RefCell::new(cb))))
        });
        RemoveEventHandler::new(move || {
            LISTENERS.with_borrow_mut(|l| {
                if let Some(list) = l.get_mut(&key) {
                    list.retain(|(i, _)| *i != id);
                    if list.is_empty() {
                        l.remove(&key);
                    }
                }
            });
        })
    }
    pub fn add_event_listener_use_capture(
        el: &Element,
        name: &str,
        cb: Box<dyn FnMut(Event)>,
    ) -> RemoveEventHandler<Element> {
        Self::add_event_listener(el, name, cb)
    }
    pub fn add_event_listener_delegated(
        el: &Element,
        name: Cow<'static, str>,
        _key: Cow<'static, str>,
        cb: Box<dyn FnMut(Event)>,
    ) -> RemoveEventHandler<Element> {
        Self::add_event_listener(el, &name, cb)
    }
    pub fn event_target<T: CastFrom<Element>>(event: &Event) -> T {
        T::cast_from(event.target).expect("native event target has wrong type")
    }
    pub fn class_list(el: &Element) -> ClassList {
        *el
    }
    pub fn add_class(el: &ClassList, class: &str) {
        let mut classes = el.get_attribute("class").unwrap_or_default();
        if !classes.split_whitespace().any(|c| c == class) {
            if !classes.is_empty() {
                classes.push(' ');
            }
            classes.push_str(class);
            Self::set_attribute(el, "class", &classes);
        }
    }
    pub fn remove_class(el: &ClassList, class: &str) {
        let classes = el
            .get_attribute("class")
            .unwrap_or_default()
            .split_whitespace()
            .filter(|c| *c != class)
            .collect::<Vec<_>>()
            .join(" ");
        Self::set_attribute(el, "class", &classes);
    }
    pub fn style(el: &Element) -> CssStyleDeclaration {
        *el
    }
    pub fn set_css_property(el: &CssStyleDeclaration, key: &str, value: &str) {
        document(|d| d.mutate().set_style_property(el.0, key, value));
    }
    pub fn remove_css_property(el: &CssStyleDeclaration, key: &str) {
        document(|d| d.mutate().remove_style_property(el.0, key));
    }
    pub fn set_inner_html(_el: &Element, _html: &str) {
        unimplemented!("native inner_html is not supported by this spike")
    }
    pub fn get_template<V: ToTemplate + 'static>() -> TemplateElement {
        unimplemented!("native templates are not supported by this spike")
    }
    pub fn clone_template(_tpl: &TemplateElement) -> Element {
        unimplemented!("native templates are not supported by this spike")
    }
    pub fn create_element_from_html(_html: Cow<'static, str>) -> Element {
        unimplemented!("native HTML templates are not supported by this spike")
    }
    pub fn create_svg_element_from_html(_html: Cow<'static, str>) -> Element {
        unimplemented!("native SVG templates are not supported by this spike")
    }
}

pub fn queue_microtask(task: impl FnOnce() + 'static) {
    any_spawner::Executor::spawn_local(async move {
        task();
    });
}
