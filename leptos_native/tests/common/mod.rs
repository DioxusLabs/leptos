use leptos_native::LeptosDocument;
use std::path::Path;

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('"', "&quot;")
}

fn html(node: &blitz_dom::Node, raw: bool, output: &mut String) {
    match &node.data {
        blitz_dom::NodeData::Text(text) => {
            output.push_str(&if raw {
                text.content.to_string()
            } else {
                escape(&text.content)
            });
        }
        blitz_dom::NodeData::Element(element) => {
            let tag = element.name.local.as_ref();
            output.push('<');
            output.push_str(tag);
            for attr in element.attrs().iter() {
                output.push_str(&format!(
                    " {}=\"{}\"",
                    attr.name.local,
                    escape(&attr.value)
                ));
            }
            output.push('>');
            for child in &node.children {
                html(
                    &node.tree()[*child],
                    matches!(tag, "style" | "script"),
                    output,
                );
            }
            if !matches!(
                tag,
                "area"
                    | "base"
                    | "br"
                    | "col"
                    | "embed"
                    | "hr"
                    | "img"
                    | "input"
                    | "link"
                    | "meta"
                    | "param"
                    | "source"
                    | "track"
                    | "wbr"
            ) {
                output.push_str(&format!("</{tag}>"));
            }
        }
        _ => {
            for child in &node.children {
                html(&node.tree()[*child], raw, output);
            }
        }
    }
}

pub fn snapshot(doc: &mut LeptosDocument, output: &Path, name: &str) {
    doc.render_png(output.join(format!("{name}.png")));
    let mut markup = String::from("<!doctype html>");
    html(doc.dom.root_node(), false, &mut markup);
    std::fs::write(output.join(format!("{name}.html")), markup).unwrap();
}
