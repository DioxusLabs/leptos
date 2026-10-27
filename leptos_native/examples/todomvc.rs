use leptos::prelude::*;

fn main() {
    leptos_native::launch(|| {
        view! {
            <style>{include_str!("../../examples/todomvc/node_modules/todomvc-common/base.css")}</style>
            <style>{include_str!("../../examples/todomvc/node_modules/todomvc-app-css/index.css")}</style>
            <todomvc::TodoMVC />
        }
    });
}
