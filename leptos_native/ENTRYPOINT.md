# One application entrypoint, two targets

The counter's application factory and `mount_to_body` call are shared. From
the repository root:

```sh
# Browser (the existing Trunk entrypoint)
cargo check --manifest-path examples/counter/Cargo.toml --target wasm32-unknown-unknown
cd examples/counter
trunk serve
```

```sh
# Native CPU-rendered Blitz window
cargo run --manifest-path examples/counter/Cargo.toml --features native

# Same binary/entrypoint, one headless PNG frame instead of a window
LEPTOS_NATIVE_HEADLESS=counter-native.png \
  cargo run --manifest-path examples/counter/Cargo.toml --features native
```

`counter/native` enables `leptos/native` and an optional `leptos_native`
dependency with `auto-launch`. The counter library links that dependency using
`use leptos_native as _`; application component code is unchanged.

Under `native`, `leptos::mount::mount_to_body` discovers exactly one
`NativeLauncher` through `inventory`. The host submits its launcher when
`auto-launch` is enabled. The application factory is type-erased to a boxed
closure producing `AnyView`, then evaluated **inside** the host's normal
document/owner/executor scope, not during registration. No event loop is
created before `main`. Missing and duplicate hosts produce explicit errors.

This keeps dependency direction `app -> leptos_native -> leptos`:
Leptos has no dependency on its host, Blitz shell, or winit. The host is a normal
workspace member, so the root lockfile includes its windowing dependencies;
browser-only builds do not compile them. The native host is
opt-in and does not register itself when used only as a document/test helper.

## Browser-only initialization exception

The host API and its dependencies require `leptos_native/native` (disabled by
default so workspace-wide browser checks do not unify `leptos/native`).
`auto-launch` includes `native` and registers the shared entrypoint launcher.
Run all native tests with:

```sh
cargo test -p leptos_native --release --features auto-launch -- --nocapture
```

The counter's `main.rs` is not byte-for-byte unchanged: its two browser logging
initializers are wrapped in `#[cfg(target_arch = "wasm32")]`. The application
factory and mount call remain unchanged; there is no alternate native main.

`console_log::init_with_level` can install its logger on native, but later
logging calls use browser console bindings. Likewise,
`console_error_panic_hook::set_once` installs a hook whose panic handling calls
JavaScript. Both are unsafe choices for a native process even though their
initializers compile. Native retains Rust's default panic handler and does
not install the browser logger. Web behavior is unchanged.

Other apps adopting this setup must likewise gate genuinely browser-only
startup code; `leptos/native` alone cannot automatically link an optional host
or translate arbitrary direct web-sys calls.
