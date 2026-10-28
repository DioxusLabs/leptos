use leptos::prelude::*;

fn main() {
    leptos_native::launch(|| {
        view! {
            <style>{include_str!("bootstrap.min.css")}</style>
            <style>".danger { background: gold }"</style>
            <js_framework_benchmark_leptos::App />
        }
    });
}
