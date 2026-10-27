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

#[test]
fn static_html_svg_and_inner_html() {
    let mut doc = LeptosDocument::new(|| ());
    native::with_document(&mut doc.dom, || {
        for tag in ["div", "td", "tr", "tbody"] {
            let element = Rndr::create_element_from_html(
                format!("<{tag}>hello &amp; goodbye</{tag}>").into(),
            );
            assert_eq!(element.tag_name(), tag.to_uppercase());
            if tag == "div" || tag == "td" {
                assert_eq!(
                    element.text_content().as_deref(),
                    Some("hello & goodbye")
                );
            }
        }
        let svg = Rndr::create_svg_element_from_html("<circle r='5'/>".into());
        assert_eq!(svg.tag_name(), "CIRCLE");
        assert_eq!(svg.get_attribute("r").as_deref(), Some("5"));
        let element = Rndr::create_element("div", None);
        Rndr::set_inner_html(&element, "<strong>hello &amp; goodbye</strong>");
        assert_eq!(element.text_content().as_deref(), Some("hello & goodbye"));
        let old_child = Rndr::first_child(&element).unwrap();
        Rndr::set_inner_html(&element, "");
        assert_eq!(Rndr::first_child(&element), None);
        assert_eq!(Rndr::get_parent(&old_child), None);
        assert!(
            std::panic::catch_unwind(|| Rndr::create_element_from_html(
                "".into()
            ))
            .is_err()
        );
        assert_eq!(Rndr::create_element("div", None).tag_name(), "DIV");
    });
}

#[test]
fn hash_links_focus_and_live_checked_properties() {
    let hashes = Rc::new(Cell::new(0));
    let changes = Rc::new(Cell::new(0));
    let blurs = Rc::new(Cell::new(0));
    let handle = Rc::new(std::cell::RefCell::new(None));
    let mut doc = LeptosDocument::new({
        let hashes = hashes.clone();
        let changes = changes.clone();
        let blurs = blurs.clone();
        let handle = handle.clone();
        move || {
            *handle.borrow_mut() =
                Some(window_event_listener(ev::hashchange, move |_| {
                    hashes.set(hashes.get() + 1);
                    assert_eq!(location_hash().as_deref(), Some("/active"));
                }));
            view! {
                <input id="first" on:focusout=move |_| blurs.set(blurs.get() + 1) />
                <input id="second" />
                <input id="checked" type="checkbox" prop:checked=false on:change=move |event| {
                    assert!(event_target_checked(&event));
                    changes.set(changes.get() + 1);
                } />
                <a href="#/active">"Active"</a>
                <a href="#/blocked" on:click=move |event| event.prevent_default()>"Blocked"</a>
            }
        }
    });
    let first = doc.dom.query_selector("#first").unwrap().unwrap();
    let second = doc.dom.query_selector("#second").unwrap().unwrap();
    native::with_document(&mut doc.dom, || {
        Node(first).focus().unwrap();
        Node(second).focus().unwrap();
        assert_eq!(blurs.get(), 1);
    });
    assert_eq!(doc.dom.get_focussed_node_id(), Some(second));
    let checked = doc.dom.query_selector("#checked").unwrap().unwrap();
    native::with_document(&mut doc.dom, || assert!(!Node(checked).checked()));
    doc.dom.resolve(0.0);
    native::with_document(&mut doc.dom, || assert!(!Node(checked).checked()));
    doc.click(checked);
    assert_eq!(changes.get(), 1);
    native::with_document(&mut doc.dom, || {
        assert!(Node(checked).checked());
        Rndr::set_property(&Node(checked), "checked", &false.into());
        assert!(!Node(checked).checked());
    });
    let active = doc.find_by_text("a", "Active").unwrap();
    doc.click(active);
    doc.click(active);
    doc.click(doc.find_by_text("a", "Blocked").unwrap());
    assert_eq!(hashes.get(), 1);
    assert_eq!(doc.dom.url().fragment(), Some("/active"));
    handle.borrow_mut().take().unwrap().remove();
    native::with_document(&mut doc.dom, || native::navigate_hash("/completed"));
    assert_eq!(hashes.get(), 1);
    let mut other = LeptosDocument::new(|| ());
    native::with_document(&mut other.dom, || {
        assert_eq!(location_hash().as_deref(), Some(""))
    });
}
