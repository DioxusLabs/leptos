use leptos::prelude::*;

fn main() {
    leptos_native::launch(|| {
        view! {
            <style>{include_str!("todomvc.css")}</style>
            <todomvc::TodoMVC />
        }
    });
}
