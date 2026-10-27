use blitz_dom::Document;
use leptos::prelude::*;
use leptos_native::LeptosDocument;

#[test]
fn simple_counter_clicks_and_paints() {
    let mut doc = LeptosDocument::new(
        || view! { <counter::SimpleCounter initial_value=0 step=1 /> },
    );
    assert!(doc.text().contains("Value: 0!"), "{}", doc.text());
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("output");
    std::fs::create_dir_all(&output).unwrap();
    doc.render_png(output.join("counter-before.png"));
    let increment = doc.find_by_text("button", "+1").expect("+1 button");
    for _ in 0..3 {
        doc.click(increment);
    }
    doc.poll(None);
    assert!(doc.text().contains("Value: 3!"), "{}", doc.text());
    doc.render_png(output.join("counter-after.png"));
    println!("Value: 3!");
    println!("{}", output.join("counter-before.png").display());
    println!("{}", output.join("counter-after.png").display());
}
