#![cfg(feature = "auto-launch")]

use leptos::prelude::*;
use leptos_native as _;

#[test]
fn mount_to_body_child() {
    if std::env::var_os("LEPTOS_NATIVE_ENTRYPOINT_TEST").is_none() {
        return;
    }
    leptos::mount::mount_to_body(|| {
        view! { <counter::SimpleCounter initial_value=0 step=1 /> }
    });
}

#[test]
fn registered_host_renders_mount_to_body() {
    let output =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("output");
    std::fs::create_dir_all(&output).unwrap();
    let png = output.join("entrypoint-test.png");
    let result = std::process::Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "mount_to_body_child", "--nocapture"])
        .env("LEPTOS_NATIVE_ENTRYPOINT_TEST", "1")
        .env("LEPTOS_NATIVE_HEADLESS", &png)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let bytes = std::fs::read(&png).unwrap();
    assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
    println!("{}", String::from_utf8_lossy(&result.stdout));
}
