use blitz_dom::Document;
use blitz_traits::{
    events::{BlitzKeyEvent, DomEventData, MouseEventButton, UiEvent},
    node_id::NodeId,
    shell::Viewport,
};
use leptos::{
    prelude::*,
    tachys::renderer::native::{self, Node},
};
use leptos_native::LeptosDocument;

mod common;

fn query(doc: &LeptosDocument, selector: &str) -> NodeId {
    doc.dom.query_selector(selector).unwrap().expect(selector)
}

fn app() -> LeptosDocument {
    let mut doc = LeptosDocument::new(|| {
        view! {
            <style>{include_str!("../../examples/todomvc/node_modules/todomvc-common/base.css")}</style>
            <style>{include_str!("../../examples/todomvc/node_modules/todomvc-app-css/index.css")}</style>
            <todomvc::TodoMVC />
        }
    });
    doc.dom
        .set_viewport(Viewport::new(720, 620, 1.0, Default::default()));
    doc.dom.resolve(0.0);
    doc
}

fn key(doc: &mut LeptosDocument, target: NodeId, name: &str, down: bool) {
    let event = BlitzKeyEvent {
        key: name.parse().unwrap(),
        code: name.parse().unwrap(),
        modifiers: Default::default(),
        location: Default::default(),
        is_auto_repeating: false,
        is_composing: false,
        state: if down {
            blitz_traits::events::KeyState::Pressed
        } else {
            blitz_traits::events::KeyState::Released
        },
        text: None,
    };
    doc.dispatch_event(
        target,
        if down {
            DomEventData::KeyDown(event)
        } else {
            DomEventData::KeyUp(event)
        },
    );
    doc.poll(None);
}

fn add(doc: &mut LeptosDocument, title: &str) {
    let target = query(doc, ".new-todo");
    doc.input(target, title);
    key(doc, target, "Enter", true);
    native::with_document(&mut doc.dom, || {
        assert_eq!(Node(target).value(), "")
    });
}

fn titles(doc: &LeptosDocument) -> Vec<String> {
    doc.dom
        .query_selector_all(".todo-list > li label")
        .unwrap()
        .iter()
        .map(|id| doc.dom.get_node(*id).unwrap().text_content())
        .collect()
}

fn count(doc: &LeptosDocument, expected: &str) {
    assert_eq!(
        doc.dom
            .get_node(query(doc, ".todo-count"))
            .unwrap()
            .text_content(),
        expected
    );
    println!("{expected}: {:?}", titles(doc));
}

fn click(doc: &mut LeptosDocument, selector: &str) {
    doc.click(query(doc, selector));
    doc.poll(None);
}

fn assert_checkbox_images(doc: &LeptosDocument) {
    for id in doc.dom.query_selector_all(".todo-list > li label").unwrap() {
        let element = doc.dom.get_node(id).unwrap().element_data().unwrap();
        let image = element.background_images[0].as_ref().unwrap();
        assert_eq!(image.status, blitz_dom::node::Status::Ok);
        assert!(matches!(image.image, blitz_dom::node::ImageData::Svg(_)));
    }
}

fn edit(doc: &mut LeptosDocument, selector: &str) -> NodeId {
    let label = query(doc, selector);
    let data = doc
        .dom
        .get_node(label)
        .unwrap()
        .synthetic_click_event(Default::default());
    let DomEventData::Click(pointer) = data else {
        panic!("expected click data")
    };
    doc.dispatch_event(label, DomEventData::DoubleClick(pointer));
    doc.poll(None);
    let input = query(doc, ".edit");
    assert_eq!(doc.dom.get_focussed_node_id(), Some(input));
    input
}

fn pointer_double_click(doc: &mut LeptosDocument, label: NodeId) {
    doc.dom.resolve(0.0);
    let mut pointer = doc
        .dom
        .get_node(label)
        .unwrap()
        .synthetic_click_event_data(Default::default());
    let start = std::time::Instant::now();
    for _ in 0..2 {
        pointer.buttons = MouseEventButton::Main.into();
        doc.handle_ui_event(UiEvent::PointerDown(pointer.clone()));
        pointer.buttons = Default::default();
        doc.handle_ui_event(UiEvent::PointerUp(pointer.clone()));
    }
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
}

#[test]
fn pointer_double_clicks_edit_each_todo_without_another_input() {
    let mut doc = app();
    for title in ["First todo", "Second todo", "Third todo"] {
        add(&mut doc, title);
    }
    for index in 1..=3 {
        let label = query(&doc, &format!(".todo:nth-child({index}) label"));
        pointer_double_click(&mut doc, label);
        let input = query(&doc, ".edit");
        assert_eq!(doc.dom.get_focussed_node_id(), Some(input));
        key(&mut doc, input, "Escape", false);
    }
    println!(
        "two pointer down/up sequences deliver dblclick and focus edit on all \
         three todos"
    );
}

#[test]
fn rapid_repeated_double_click_at_same_position_is_a_blitz_gap() {
    let mut doc = app();
    add(&mut doc, "Same position");
    let start = std::time::Instant::now();
    let label = query(&doc, ".todo label");
    pointer_double_click(&mut doc, label);
    let input = query(&doc, ".edit");
    key(&mut doc, input, "Escape", false);
    let label = query(&doc, ".todo label");
    pointer_double_click(&mut doc, label);
    assert!(start.elapsed() < std::time::Duration::from_millis(500));
    assert!(doc.dom.query_selector(".edit").unwrap().is_none());
    println!(
        "Blitz gap: a second rapid double-click at the same position has \
         click counts 3/4, not 2; no dblclick"
    );
}

#[test]
fn focused_clear_completed_has_no_blitz_keyboard_activation() {
    let mut doc = app();
    add(&mut doc, "Completed todo");
    doc.dom.resolve(0.0);
    click(&mut doc, ".toggle");
    assert_eq!(doc.dom.query_selector_all(".todo.completed").unwrap().len(), 1);
    doc.dom.resolve(0.0);
    let button = query(&doc, ".clear-completed");
    let press = |doc: &mut LeptosDocument, name: &str, code: &str| {
        let mut event = BlitzKeyEvent {
            key: name.parse().unwrap(),
            code: code.parse().unwrap(),
            modifiers: Default::default(),
            location: Default::default(),
            is_auto_repeating: false,
            is_composing: false,
            state: blitz_traits::events::KeyState::Pressed,
            text: None,
        };
        doc.handle_ui_event(UiEvent::KeyDown(event.clone()));
        event.state = blitz_traits::events::KeyState::Released;
        doc.handle_ui_event(UiEvent::KeyUp(event));
    };
    for _ in 0..16 {
        if doc.dom.get_focussed_node_id() == Some(button) {
            break;
        }
        press(&mut doc, "Tab", "Tab");
    }
    assert_eq!(doc.dom.get_focussed_node_id(), Some(button));
    for (name, code) in [("Enter", "Enter"), (" ", "Space")] {
        press(&mut doc, name, code);
        assert_eq!(titles(&doc), ["Completed todo"]);
    }
    doc.click(button);
    doc.poll(None);
    assert!(titles(&doc).is_empty());
    println!(
        "Blitz gap: focused Clear completed receives Enter/Space but no \
         activation click; direct click clears"
    );
}

#[test]
fn todos_add_toggle_filter_edit_and_clear() {
    let mut doc = app();
    assert!(titles(&doc).is_empty());
    assert_eq!(
        doc.dom.get_focussed_node_id(),
        Some(query(&doc, ".new-todo"))
    );
    for title in ["Buy milk", "Write native tests", "Ship spike"] {
        add(&mut doc, title);
    }
    count(&doc, "3 items left");
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("output");
    std::fs::create_dir_all(&output).unwrap();
    common::snapshot(&mut doc, &output, "todomvc-before");
    assert_checkbox_images(&doc);

    click(&mut doc, ".todo:first-child .toggle");
    count(&doc, "2 items left");
    let toggle = query(&doc, ".todo:first-child .toggle");
    native::with_document(&mut doc.dom, || assert!(Node(toggle).checked()));
    click(&mut doc, ".todo:first-child .toggle");
    count(&doc, "3 items left");
    click(&mut doc, ".todo:first-child .toggle");

    click(&mut doc, "a[href='#/active']");
    assert_eq!(titles(&doc), ["Write native tests", "Ship spike"]);
    assert_eq!(doc.dom.url().fragment(), Some("/active"));
    count(&doc, "2 items left");
    click(&mut doc, "a[href='#/completed']");
    assert_eq!(titles(&doc), ["Buy milk"]);
    assert_eq!(doc.dom.url().fragment(), Some("/completed"));
    count(&doc, "2 items left");
    common::snapshot(&mut doc, &output, "todomvc-completed");
    assert_checkbox_images(&doc);
    click(&mut doc, "a[href='#/']");

    let input = edit(&mut doc, ".todo:nth-child(2) label");
    native::with_document(&mut doc.dom, || {
        assert_eq!(Node(input).value(), "Write native tests")
    });
    doc.input(input, "Write more native tests");
    key(&mut doc, input, "Enter", false);
    assert_eq!(
        titles(&doc),
        ["Buy milk", "Write more native tests", "Ship spike"]
    );
    assert!(doc.dom.query_selector(".edit").unwrap().is_none());
    click(&mut doc, ".clear-completed");
    assert_eq!(titles(&doc), ["Write more native tests", "Ship spike"]);
    count(&doc, "2 items left");
    common::snapshot(&mut doc, &output, "todomvc-after");
    println!("{}", output.join("todomvc-before.png").display());
    println!("{}", output.join("todomvc-after.png").display());
}

#[test]
fn empty_titles_cancel_and_empty_edit() {
    let mut doc = app();
    let target = query(&doc, ".new-todo");
    doc.input(target, "   ");
    key(&mut doc, target, "Enter", true);
    assert!(titles(&doc).is_empty());
    add(&mut doc, "Keep me");
    let input = edit(&mut doc, ".todo label");
    doc.input(input, "Discard this edit");
    key(&mut doc, input, "Escape", false);
    assert_eq!(titles(&doc), ["Keep me"]);
    let input = edit(&mut doc, ".todo label");
    doc.input(input, "  ");
    key(&mut doc, input, "Enter", false);
    assert!(titles(&doc).is_empty());
    assert!(titles(&app()).is_empty());
}
