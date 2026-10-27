use blitz_dom::Document;
use leptos::{
    ev, html,
    prelude::*,
    tachys::renderer::{
        native::{self, Node},
        Rndr,
    },
};
use leptos_native::LeptosDocument;
use std::{cell::Cell, rc::Rc};

#[test]
fn empty_document_and_scoped_access() {
    let mut doc = LeptosDocument::new(|| ());
    assert_eq!(doc.text(), "");
    assert!(!doc.poll(None));
    native::with_document(&mut doc.dom, || {
        let root = Node(doc.root);
        Rndr::clear_children(&root);
        let text = Rndr::create_text_node("hello");
        Rndr::insert_node(&root, &text, None);
        assert_eq!(Rndr::first_child(&root), Some(text));
        Rndr::clear_children(&root);
        assert_eq!(Rndr::first_child(&root), None);
        assert_eq!(Rndr::get_parent(&text), None);
        Rndr::insert_node(&root, &text, None);
        assert_eq!(Rndr::remove_node(&root, &text), Some(text));
        assert_eq!(Rndr::remove_node(&root, &text), None);
    });
    let result =
        std::panic::catch_unwind(|| Rndr::create_text_node("outside scope"));
    assert!(result.is_err());
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        native::with_document(&mut doc.dom, || panic!("unwind document scope"));
    }));
    assert!(result.is_err());
    assert!(std::panic::catch_unwind(|| Rndr::create_placeholder()).is_err());
}

#[test]
fn native_aliases_node_refs_properties_and_event_control() {
    let button_ref = NodeRef::<html::Button>::new();
    let called = Rc::new(Cell::new(0));
    let parent_called = Rc::new(Cell::new(0));
    let mut doc = LeptosDocument::new({
        let called = called.clone();
        let parent_called = parent_called.clone();
        move || {
            view! {
                <div on:click=move |_| parent_called.set(parent_called.get() + 1)>
                    <button node_ref=button_ref prop:value="native" on:click=move |event: ev::MouseEvent| {
                        assert!(event.target().is_some());
                        event.prevent_default();
                        event.stop_propagation();
                        called.set(called.get() + 1);
                    }>"click"</button>
                </div>
            }
        }
    });
    let button = button_ref.get_untracked().expect("native node ref");
    native::with_document(&mut doc.dom, || {
        assert_eq!(button.get_attribute("value").as_deref(), Some("native"));
        Rndr::add_class(&Rndr::class_list(&button), "active");
        Rndr::add_class(&Rndr::class_list(&button), "active");
        assert_eq!(button.get_attribute("class").as_deref(), Some("active"));
        Rndr::remove_class(&Rndr::class_list(&button), "active");
        assert_eq!(button.get_attribute("class").as_deref(), Some(""));
        Rndr::set_css_property(&Rndr::style(&button), "color", "red");
        Rndr::remove_css_property(&Rndr::style(&button), "color");
    });
    doc.click(button.0);
    assert_eq!(called.get(), 1);
    assert_eq!(parent_called.get(), 0);
}

#[test]
fn documents_have_independent_tasks_and_listeners() {
    let app = || view! { <counter::SimpleCounter initial_value=0 step=1 /> };
    let mut first = LeptosDocument::new(app);
    let mut second = LeptosDocument::new(app);
    first.click(first.find_by_text("button", "+1").unwrap());
    second.poll(None);
    first.poll(None);
    assert!(first.text().contains("Value: 1!"));
    assert!(second.text().contains("Value: 0!"));
    drop(first);
    second.click(second.find_by_text("button", "-1").unwrap());
    second.poll(None);
    assert!(second.text().contains("Value: -1!"));
    second.click(second.find_by_text("button", "Clear").unwrap());
    second.poll(None);
    assert!(second.text().contains("Value: 0!"));
}
