use leptos::prelude::*;

fn main() {
    leptos_native::launch(
        || view! { <counter::SimpleCounter initial_value=0 step=1 /> },
    );
}
