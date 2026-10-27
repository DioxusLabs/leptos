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

- **Checkbox circles/checkmarks absent.** Their labels use percent-encoded
  `data:image/svg+xml` CSS `background-image` URLs; the real input is hidden
  with `opacity: 0`. This is also a host integration gap: `LeptosDocument`
  supplies no resource provider, so Blitz defaults to `DummyNetProvider`,
  whose `fetch` does nothing. Do not report this as a proven SVG painter bug
  without retesting with a functioning resource provider.
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
