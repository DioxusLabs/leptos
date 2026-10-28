use blitz_dom::Document;
use blitz_traits::node_id::NodeId;
use leptos::prelude::*;
use leptos_native::LeptosDocument;

fn query(doc: &LeptosDocument, selector: &str) -> NodeId {
    doc.dom.query_selector(selector).unwrap().expect(selector)
}

fn summary(doc: &LeptosDocument, total: i32, count: usize, values: &[i32]) {
    let text = doc.dom.get_node(query(doc, "p")).unwrap().text_content();
    assert_eq!(text, format!("Total: {total} from {count} counters."));
    let rows = doc.dom.query_selector_all("li > span").unwrap();
    assert_eq!(rows.len(), count);
    assert_eq!(
        rows.iter()
            .map(|id| doc.dom.get_node(*id).unwrap().text_content())
            .collect::<Vec<_>>(),
        values.iter().map(ToString::to_string).collect::<Vec<_>>()
    );
    println!("{text} Values: {values:?}");
}

#[test]
fn counters_add_increment_remove_and_input() {
    let mut doc = LeptosDocument::new(|| view! { <counters::Counters /> });
    doc.dom.set_viewport(blitz_traits::shell::Viewport::new(
        640,
        260,
        1.0,
        Default::default(),
    ));
    summary(&doc, 0, 0, &[]);
    let add = doc.find_by_text("button", "Add Counter").unwrap();
    for _ in 0..3 {
        doc.click(add);
        doc.poll(None);
    }
    summary(&doc, 0, 3, &[0, 0, 0]);
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("output");
    std::fs::create_dir_all(&output).unwrap();
    doc.render_png(output.join("counters-before.png"));
    let first_increment = query(&doc, "li:nth-child(1) button:nth-of-type(2)");
    let second_increment = query(&doc, "li:nth-child(2) button:nth-of-type(2)");
    doc.click(first_increment);
    doc.click(first_increment);
    doc.click(second_increment);
    doc.poll(None);
    summary(&doc, 3, 3, &[2, 1, 0]);
    doc.click(query(&doc, "li:nth-child(2) button:last-child"));
    doc.poll(None);
    summary(&doc, 2, 2, &[2, 0]);
    let input = query(&doc, "li input");
    for (value, expected) in [("", 0), ("not a number", 0), ("7", 7)] {
        doc.input(input, value);
        doc.poll(None);
        summary(&doc, expected, 2, &[expected, 0]);
    }
    doc.render_png(output.join("counters-after.png"));
    println!("{}", output.join("counters-before.png").display());
    println!("{}", output.join("counters-after.png").display());
    doc.click(doc.find_by_text("button", "Clear Counters").unwrap());
    doc.poll(None);
    summary(&doc, 0, 0, &[]);
}
