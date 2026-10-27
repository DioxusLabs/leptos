use leptos::prelude::*;

fn main() {
    leptos_native::launch(|| {
        view! {
            <style>{include_str!("js_framework_benchmark.css")}</style>
            <js_framework_benchmark_leptos::App />
        }
    });
}
