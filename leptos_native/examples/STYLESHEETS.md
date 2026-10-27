# Web stylesheet parity

The native benchmark uses the unmodified Bootstrap **5.2.3** stylesheet from
<https://cdn.jsdelivr.net/npm/bootstrap@5.2.3/dist/css/bootstrap.min.css>, plus
`.danger { background: gold }`, exactly as its web `index.html` does.
`bootstrap.min.css` is vendored, with its MIT license in `bootstrap.LICENSE`.
Its SHA-384 is
`adbb00d9504a421820c33c47ee93c2680a8ee3a3209ce33cd335b5456b87eb50c62f064911d2b629a76ad85f42506eb9`.

TodoMVC includes the existing **tracked** files directly, in web order:

- `examples/todomvc/node_modules/todomvc-common/base.css`
- `examples/todomvc/node_modules/todomvc-app-css/index.css`

Neither app has native-specific CSS overrides. Tests require no network.

## Reproduce the comparison

From the repository root:

```sh
cargo test --release --manifest-path leptos_native/Cargo.toml --features native \
  --test todomvc --test js_framework_benchmark -- --nocapture
python leptos_native/examples/render_web_references.py
```

The reference script requires Chrome/Chromium and Python's `websockets` package.
On a headless Linux host without a session bus, it can be run under
`dbus-run-session -- python ...`.

Tests save native PNGs and static HTML snapshots of the mounted DOM, including
the inline stylesheets and generated row labels. The script renders those HTML
snapshots in headless Chrome, with external DNS disabled. These are **static web
references, not a live wasm run**. CDP sets the exact native viewport dimensions
(benchmark 1000x700, TodoMVC 720x620); relying only on Chrome's `--window-size`
gave a shorter content viewport. Native snapshots serialize ordinary HTML,
not Blitz's XML-like self-closing serialization of empty non-void elements.
Focus is retained for the initial autofocus screenshot and cleared for later
references to avoid comparing a newly-autofocused page with the native app's
post-click state. Hover states and browser history are not compared.

Artifacts live in `leptos_native/output/`:

| App/state | Native | Chrome |
| --- | --- | --- |
| Benchmark empty | `benchmark-before.png` | `benchmark-before-web.png` |
| Benchmark selected | `benchmark-selected.png` | `benchmark-selected-web.png` |
| Benchmark 11,000 rows | `benchmark-after.png` | `benchmark-after-web.png` |
| TodoMVC three todos | `todomvc-before.png` | `todomvc-before-web.png` |
| TodoMVC Completed filter | `todomvc-completed.png` | `todomvc-completed-web.png` |
| TodoMVC after clear | `todomvc-after.png` | `todomvc-after-web.png` |

## Native event/redraw behavior

The host drains Leptos tasks after the complete Blitz UI event/default-action
queue, before returning to the window's input handler. It retains event dirtiness
even when callbacks mutate the DOM synchronously without waking a future.
Polling consumed work explicitly requests redraw through the document's shell
provider. A pending future still wakes Blitz through the stored task-context
waker, and its next poll also requests redraw.

`tests/runtime.rs` checks pointer down/up → counter update → redraw request,
without an extra input or caller poll, plus synchronous attribute changes.
A separate test releases a pending future without input, asserts the registered
shell waker fires, and checks that polling updates the DOM and requests redraw.
These use a recording shell provider, not a real window. The original window
timing is not reproduced headlessly; this verifies the host-side ordering and
redraw contract, not the OS presentation of the frame.

### Upstream Blitz input gaps (pinned fe33b77)

- **Focused buttons have no Enter/Space activation.**
  `focused_clear_completed_has_no_blitz_keyboard_activation` Tab-focuses the
  actual TodoMVC button, sends both keys through `Document::handle_ui_event`, and
  verifies the completed todo remains. A direct click clears it. Blitz's
  `events/driver.rs` forwards focused key events to default handling;
  `events/keyboard.rs` implements Tab and text editing but no button activation,
  and the KeyUp default action in `events/mod.rs` is empty.
- **Double-click delivery works for two clicks, but repeated pairs can fail.**
  `pointer_double_clicks_edit_each_todo_without_another_input` sends two pointer
  down/up sequences within 500ms at each of three labels; all enter editing and
  focus the input without another event. The rapid-repeat test then demonstrates
  that two more clicks at the same label within 500ms do not reopen editing.
  Blitz's `events/pointer.rs::handle_pointerdown` increments the count without
  resetting after a pair when each down is within 500ms and 2px of the previous
  one. Its double-click dispatch checks exactly `click_count == 2`, so counts
  3 and 4 emit no double-click. This reproduces one failure mode, but the user's
  exact click timing/movement was not recorded, so it does not establish the
  cause of every reported missed edit.

## Visible differences (Chrome 137, Blitz CPU renderer)

These identify the CSS features exercised by the differences, not isolated
minimal reproductions or proven internal Blitz root causes. No app CSS was
added to conceal them.

### Benchmark

- **Missing alternating gray rows.** Bootstrap implements striping through
  `--bs-table-accent-bg` and cell `box-shadow: inset 0 0 0 9999px ...`, rather
  than a row background. Native rows remain white. The gold `.danger` row
  background does render. Investigate table-cell inset-shadow painting.
- **Different column widths.** Chrome's label starts near x=118, native near
  x=246 in the selected snapshot and x=393 in the 11,000-row snapshot.
  Investigate automatic table layout with percentage widths from `.col-md-*`
  applied to `td`, including empty trailing cells.
- **Different row pitch/borders.** Native rows are about 42px tall versus
  Chrome's 41px, accumulating vertically. Investigate collapsed table borders
  and line-height/font metric rounding, not Bootstrap grid spacing.

The empty-state header/grid/buttons closely match. Bootstrap 5 has **no**
`.jumbotron`, `.btn-block`, or Glyphicons styling: absent gray hero panels and
remove icons in **both** renderers are upstream version/markup mismatches,
not Blitz bugs. The previous native stylesheet invented those appearances.

### TodoMVC

SVG data-URI backgrounds now load in both windowed and headless rendering.
The host enables Blitz's SVG decoder/painter and synchronous data-URI provider;
headless painting resolves styles, then ingests the resulting image responses
before painting. The tests assert decoded SVG backgrounds in both the initial
and Completed-filter snapshots. External HTTP/file resources remain unsupported.

- **Completed checkbox keeps the unchecked circle.** The circle now renders,
  but the Completed-filter snapshot still lacks Chrome's green checkmark.
  Investigate `.toggle:checked + label` state/style invalidation; this is no
  longer an absent resource-provider or disabled-SVG issue.
- **Input placeholder absent.** Chrome displays the italic "What needs to be
  done?" text. Investigate placeholder text painting and the stylesheet's
  `::-webkit-input-placeholder` styling.
- **Filters run together below the item count; selected outline absent.**
  Chrome lays out spaced All/Active/Completed links on the footer line with a
  red selected border. Native loses inline-link margin/padding/border and
  places the absolutely-positioned `.filters` on a lower static-position line.
  Investigate inline box decoration and absolute positioning beside floats.
- **Clear completed overlaps the filters.** In the Completed-filter native
  snapshot it is painted over the centered links, instead of at the right.
  Its relevant declarations are `float: right; position: relative`, while
  `.todo-count` floats left and `.filters` is absolutely positioned.
- **Toggle-all chevron stays light gray.** Both render the generated rotated
  `label:before` glyph, but native does not show the darker color from
  `.toggle-all:checked + label:before`. Investigate checked-state style
  invalidation/pseudo-element selector state.
- **Different typography and vertical metrics.** Native's heading is wider
  and higher; row/footer text and row boundaries sit several pixels lower.
  Relevant features are the Helvetica/Arial fallback chain, font weights
  200/300/400, explicit line heights, and the absolutely-positioned heading.
  Font resolution versus line-box layout needs isolation before assigning
  an internal cause.

Generated content is **not globally missing**: the toggle-all `:before` glyph
and layered-paper `.footer:before` shadows render. Destroy buttons require
`:hover` to display their `:after` multiplication sign; neither unhovered
reference shows them, so this comparison does not establish a destroy-glyph
bug. Completed text strike-through and the principal panel shadows render.
