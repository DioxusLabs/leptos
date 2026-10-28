use blitz_dom::Document;
use blitz_traits::{node_id::NodeId, shell::Viewport};
use leptos::prelude::*;
use leptos_native::LeptosDocument;
use std::time::{Duration, Instant};

mod common;

#[derive(Debug, Clone, PartialEq)]
struct Row {
    node: NodeId,
    id: usize,
    label: String,
    selected: bool,
}

fn rows(doc: &LeptosDocument) -> Vec<Row> {
    doc.dom
        .query_selector_all("tbody > tr")
        .unwrap()
        .iter()
        .map(|id| {
            let node = doc.dom.get_node(*id).unwrap();
            let cells = node
                .children
                .iter()
                .filter_map(|child| {
                    let node = doc.dom.get_node(*child).unwrap();
                    node.is_element().then_some(node)
                })
                .collect::<Vec<_>>();
            assert_eq!(cells.len(), 4);
            Row {
                node: *id,
                id: cells[0].text_content().parse().unwrap(),
                label: cells[1].text_content(),
                selected: node
                    .attr("class".into())
                    .unwrap_or_default()
                    .split_whitespace()
                    .any(|class| class == "danger"),
            }
        })
        .collect()
}

fn query(doc: &LeptosDocument, selector: &str) -> NodeId {
    doc.dom.query_selector(selector).unwrap().expect(selector)
}

fn frame(doc: &mut LeptosDocument, target: NodeId) -> Duration {
    let start = Instant::now();
    doc.click(target);
    doc.poll(None);
    doc.dom.resolve(0.0);
    start.elapsed()
}

fn button(doc: &mut LeptosDocument, id: &str) -> Duration {
    frame(doc, query(doc, &format!("#{id}")))
}

fn assert_new_rows(rows: &[Row], count: usize) {
    assert_eq!(rows.len(), count);
    assert!(
        rows.iter()
            .all(|row| !row.selected
                && row.label.split_whitespace().count() == 3)
    );
    assert!(rows.windows(2).all(|pair| pair[0].id + 1 == pair[1].id));
}

#[test]
fn standard_operations_and_frame_timings() {
    let mut doc = LeptosDocument::new(|| {
        view! {
            <style>{include_str!("../examples/bootstrap.min.css")}</style>
            <style>".danger { background: gold }"</style>
            <js_framework_benchmark_leptos::App />
        }
    });
    doc.dom
        .set_viewport(Viewport::new(1000, 700, 1.0, Default::default()));
    doc.dom.resolve(0.0);
    assert!(rows(&doc).is_empty());
    for id in ["update", "swaprows", "clear"] {
        button(&mut doc, id);
        assert!(rows(&doc).is_empty());
    }
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("output");
    std::fs::create_dir_all(&output).unwrap();
    common::snapshot(&mut doc, &output, "benchmark-before");
    let mut timings = Vec::new();

    timings.push(("Create 1,000", button(&mut doc, "run"), 1000));
    let initial = rows(&doc);
    assert_new_rows(&initial, 1000);

    timings.push(("Replace all (1,000)", button(&mut doc, "run"), 1000));
    let replaced = rows(&doc);
    assert_new_rows(&replaced, 1000);
    assert!(replaced[0].id > initial.last().unwrap().id);
    assert!(
        replaced
            .iter()
            .all(|row| !initial.iter().any(|old| old.node == row.node))
    );

    timings.push(("Update every 10th", button(&mut doc, "update"), 1000));
    let updated = rows(&doc);
    assert_eq!(updated.len(), 1000);
    for (index, (before, after)) in replaced.iter().zip(&updated).enumerate() {
        assert_eq!(after.id, before.id);
        assert_eq!(after.node, before.node);
        assert_eq!(
            after.label,
            if index % 10 == 0 {
                format!("{} !!!", before.label)
            } else {
                before.label.clone()
            }
        );
    }

    let target = query(&doc, "tbody tr:nth-child(2) td:nth-child(2) a");
    timings.push(("Select row", frame(&mut doc, target), 1000));
    let selected = rows(&doc);
    let mut expected = updated;
    expected[1].selected = true;
    assert_eq!(selected, expected);
    common::snapshot(&mut doc, &output, "benchmark-selected");

    timings.push(("Swap rows (2 / 999)", button(&mut doc, "swaprows"), 1000));
    expected.swap(1, 998);
    assert_eq!(rows(&doc), expected);

    let target = query(&doc, "tbody tr:nth-child(2) td:nth-child(3) span");
    timings.push(("Remove row", frame(&mut doc, target), 999));
    expected.remove(1);
    assert_eq!(rows(&doc), expected);

    timings.push(("Create 10,000", button(&mut doc, "runlots"), 10000));
    let lots = rows(&doc);
    assert_new_rows(&lots, 10000);
    assert!(lots[0].id > replaced.last().unwrap().id);

    timings.push(("Append 1,000", button(&mut doc, "add"), 11000));
    let appended = rows(&doc);
    assert_eq!(appended.len(), 11000);
    assert_eq!(&appended[..10000], &lots);
    assert_new_rows(&appended[10000..], 1000);
    assert!(appended[10000].id > lots.last().unwrap().id);
    common::snapshot(&mut doc, &output, "benchmark-after");

    timings.push(("Clear", button(&mut doc, "clear"), 0));
    assert!(rows(&doc).is_empty());
    assert!(doc.dom.query_selector_all("tbody a").unwrap().is_empty());

    println!(
        "Profile: {}",
        if cfg!(debug_assertions) {
            "debug (not benchmark numbers)"
        } else {
            "release"
        }
    );
    println!(
        "Single sample per operation; click + poll + style/layout resolve; \
         excludes paint, selectors and assertions."
    );
    println!(
        "Native ViewTemplate builds nodes directly; vendored Bootstrap 5.2.3, \
         no network."
    );
    println!("{:<24} | {:>7} | {:>12}", "Operation", "Rows", "Leptos ms");
    for (name, elapsed, count) in timings {
        println!(
            "{name:<24} | {count:>7} | {:>12.3}",
            elapsed.as_secs_f64() * 1000.0
        );
    }
    for file in [
        "benchmark-before.png",
        "benchmark-selected.png",
        "benchmark-after.png",
    ] {
        println!("{}", output.join(file).display());
    }
}
