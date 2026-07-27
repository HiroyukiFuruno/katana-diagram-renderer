## Current Release Ledger

This ledger supersedes the rejected Chromium implementation record. Chromium,
Chrome for Testing, WebView, external helper processes, browser downloads, and
browser release assets are prohibited for this change. Checkboxes represent
current Rust/V8 work only; previous Chromium results are not completion evidence.

### 0. Scope And Architecture

- [x] 0.1 Keep the HTML execution boundary in KRR's in-process Rust/V8 runtime.
  KDV is a session adapter and KatanA is a main-document host/frame consumer.
- [x] 0.2 Remove Chromium/browser-helper code and add static checks rejecting
  external browser, WebView, and helper references from the interactive path.
  The release test recursively scans the interactive runtime's production Rust
  sources and its direct CSS/document/runtime support modules.
- [x] 0.3 Verify package/workflow/release checks contain no obsolete external
  browser asset, download, manifest, environment override, or helper gate.
- [x] 0.4 Preserve static HTML export separately; it is not a viewer fallback.

### 1. KRR In-Process Interactive Runtime

- [x] 1.1 Provide `HtmlRuntime` / persistent `HtmlBrowserSession` with raw HTML,
  full document origin, viewport, frame generation, input, resize, navigation,
  and close lifecycle.
- [x] 1.2 Own DOM, CSS layout/paint, JavaScript event dispatch, rasterization,
  hit-test, input state, and navigation intent in KRR without KDV/KatanA UI
  dependencies.
- [x] 1.3 Preserve raw HTML semantics: `head` metadata/script/style are not
  painted as body text and host code does not insert HTML text or scripts.
- [x] 1.4 Complete the contract matrix for styled normal flow, table/list/rule,
  details accordion, click handlers, text input, scroll, resize, focus/key
  paths, errors, and navigation. Each case must observe a frame or DOM result.
- [x] 1.5 Implement and test the permitted subresource policy for file and
  `http/https` document origins without giving KRR main-document acquisition.
- [x] 1.6 Make every production line reachable through behavioral tests and pass
  `rtk just coverage` with `100% / 0 uncovered` without exclusions or ignores.
- [x] 1.7 Release and verify KRR `0.4.1` with deterministic bundled-font SVG
  rasterization.
- [x] 1.8 Release and verify KRR `0.4.3` with browser-style system-font
  fallback, deterministic bundled Latin fallback, and committed IME text
  rendering. The crates.io 10 MiB package gate must remain unchanged.
- [x] 1.9 Release and verify KRR `0.4.4` with allowed cross-origin HTTP/HTTPS
  stylesheet/script/image loading, non-fatal blocked or failed subresources,
  and embedded SVG projection/layout. HTTPS mixed content, credential-bearing
  network URLs, local file escape, unsupported schemes, and iframe fetching
  remain rejected. Strict coverage stays at 100% lines and 0 uncovered.
- [x] 1.10 Release and verify KRR `0.4.6` with structured CSS parsing and
  cascade precedence, typed flex/grid/table/box/overflow/typography layout,
  browser-style capture/target/bubble event dispatch, and scroll/content frame
  metrics. Local `release-check` passes with 100% line coverage and 0 uncovered;
  crates.io publication and public artifact verification remain.
- [x] 1.11 Release and verify KRR `0.4.7` against a real self-contained HTML
  slide deck. Preserve explicit `<br>` line breaks, resolve viewport-relative
  lengths and `clamp()`, paint CSS linear gradients, implement selector-based
  ancestor lookup through `Element.closest()`, and repaint after slide class
  changes without introducing Chromium, WebView, or an external helper.

### 2. KDV Adapter

- [x] 2.1 Replace the rejected external-browser session worker with a KRR
  in-process session adapter that transports raw source, frame, input, resize,
  typed error, and navigation without parsing HTML.
- [x] 2.2 Add adapter contract tests for ordered input/frame/navigation lifecycle
  against KRR and pass KDV strict coverage and release checks.
- [x] 2.3 After KRR `0.4.0` publication, resolve only crates.io `^0.4.0`, release
  KDV `0.3.0`, and prove the published artifact before KatanA registry use.

### 3. KatanA Integration

- [x] 3.1 Replace the static HTML preview path with KDV frame presentation and
  raw native input forwarding; KatanA owns neither DOM nor CSS/layout/hit-test.
- [x] 3.2 Make navigation replace KatanA's active document path/source before a
  reload or resize, preventing stale initial-document state from reappearing.
- [x] 3.3 Exercise user-entered `http/https` main-document acquisition in an
  automated end-to-end scenario and retain the raw URL origin through reload.
- [x] 3.4 Capture headless-process evidence for CSS, accordion, button, input,
  local link navigation, reload, and resize using state-specific assertions.
- [x] 3.5 Reproduce Japanese committed text end-to-end, reject the tofu-glyph
  frame, and verify the local KRR `0.4.3` candidate renders `日本語 IME入力` in
  the input, JavaScript result, and status regions.
- [x] 3.6 Run KatanA compile/unit/release contract gates with registry-only KDV
  after KDV publication; no path/git dependency may remain in committed files.

### 4. Release Order And User Review

- [x] 4.1 Pass all KRR quality/release gates, publish and verify KRR `0.4.0`.
- [x] 4.2 Pass all KDV quality/release gates, publish and verify KDV `0.3.0`.
- [x] 4.3 Publish and verify KRR `0.4.3`, resolve it from crates.io in KatanA,
  and rerun the complete headless acceptance matrix for `v0.22.33` only.
- [x] 4.4 Present final registry-only state-specific screenshots and automated
  evidence to the user. Local-candidate evidence has already been presented.
  Until explicit approval: no KatanA commit, push, PR, publish, or release.
- [x] 4.5 Publish and verify KRR `0.4.4`, then allow KDV `0.3.2` and KatanA
  `v0.22.34` to consume only its crates.io artifact. KatanA evidence must cover
  remote resources, embedded Mermaid SVG, worker error recovery, and the full
  interactive HTML acceptance matrix without Chromium or WebView.
- [x] 4.6 Publish and verify KRR `0.4.6`, then refresh KatanA `v0.22.36` to the
  registry artifact and rerun strict coverage, release contracts, platform
  checks, and the headless interactive HTML evidence before user review.
- [x] 4.7 Pass KRR strict 100% line coverage and package/release gates for
  `0.4.7`, publish and verify the adjacent patch, then validate KatanA
  `v0.22.37` with registry-only dependencies. Before any KatanA commit, push,
  PR, or release, present headless initial/next-slide screenshots and machine
  assertions from the real `slides.html` document for fresh user approval.

### 5. v0.4.7 Slide Deck Compatibility Follow-up

- [x] 5.1 Preserve explicit line-break nodes during text extraction and wrapping,
  continue to collapse ordinary HTML whitespace, and apply Unicode line-break
  opportunities and display widths so Japanese text stays inside its CSS box.
- [x] 5.2 Resolve `vw`, `vh`, `vmin`, `vmax`, and nested `min()` / `max()` /
  `clamp()` length expressions against the active viewport in typography and
  box geometry.
- [x] 5.3 Parse and paint standards-style linear gradients with deterministic
  SVG definitions, angles/directions, color stops, and safe fallback for an
  unsupported background image.
- [x] 5.4 Implement `Element.matches()` and `Element.closest()` through KRR's
  existing selector engine and DOM ancestry, including detached/no-match paths.
- [x] 5.5 Add a reduced slide-deck contract covering gradient, viewport sizing,
  forced line breaks, class-driven visibility, pointer/keyboard navigation,
  page indicator mutation, and post-action repaint.
- [x] 5.6 Run the actual Google Drive `slides.html` through KatanA's headless
  native UI, compare the initial composition with a browser oracle, and prove a
  user action changes the active slide without a JavaScript exception.
- [x] 5.7 Pass focused tests, the full KRR suite, AST lint, strict coverage at
  100% with 0 uncovered lines, package verification, and publish dry-run without
  exclusions or threshold changes.
- [x] 5.8 Resolve definite viewport/container heights, `position`, edge insets,
  out-of-flow absolute/fixed boxes, and flex-column sizing so slide surfaces,
  fixed controls, and multi-column cards use their declared geometry.
- [x] 5.9 Implement browser-compatible inline formatting for nested phrasing
  content, per-edge border shorthands/longhands and cascade overrides,
  percentage border radii, `min-width`, and positioned `z-index` paint order.
  Prove these as generic CSS contracts and compare the real deck at the exact
  KRR frame dimensions so slide controls, progress, and card accents match the
  browser oracle without fixture-specific selectors or layout exceptions.

### 6. v0.4.8 Local Wrapper Compatibility Follow-up

- [x] 6.1 Resolve same-directory local iframe documents inside KRR without
  Chromium, WebView, a helper process, or host-side HTML parsing.
- [x] 6.2 Support wrapper contracts that use iframe `contentDocument`, `load`,
  synthetic `click()`, `URLSearchParams`, and the complete document `location`.
- [x] 6.3 Enforce file root, same-directory, source-size, depth, document-count,
  and cycle limits while keeping remote iframe fetching disabled.
- [x] 6.4 Render an iframe-local diagnostic and emit structured runtime context
  when a child document is missing or rejected instead of leaving a silent
  white surface or stopping the main document.
- [x] 6.5 Pass focused contracts, full workspace tests, AST lint, and strict
  coverage at 15,705 / 15,705 lines with 0 uncovered lines. The complete
  `release-check` passes, including 797 tests, package verification, crate-size
  enforcement, and `cargo publish --dry-run`. The real 3 MB local slideshow
  wrapper also renders its first slide at 1440 x 1000 through KRR `0.4.8`.
- [x] 6.6 Publish and verify KRR `0.4.8`, then update KatanA `v0.22.37` through
  the crates.io dependency chain and capture fresh local-file evidence.

### 7. v0.4.9 Chrome CSS Equivalence Follow-up

- [x] 7.1 Capture KRR and headless Chrome at the same CSS viewport and slide
  state, then compare representative element geometry, text metrics, colors,
  overflow, and the resulting raster without relying on visual judgment alone.
- [x] 7.2 Make layout measurement and SVG rasterization resolve the same
  concrete font face for every text run so fallback differences cannot change
  line wrapping, box geometry, or paint output.
- [x] 7.3 Add generic regression contracts for mixed Japanese/Latin font
  fallback, viewport-dependent typography, and layout/paint metric agreement.
- [x] 7.4 Re-run the real `slides.html` oracle comparison and regenerate KatanA
  local-file evidence from registry-only KRR `0.4.9`.
- [x] 7.5 Pass focused tests, the full workspace suite, AST lint, strict 100%
  line coverage with 0 uncovered lines, package verification, release target
  checks, and publish dry-run without exclusions or threshold changes.
- [x] 7.6 Publish and verify KRR `0.4.9` before refreshing the uncommitted
  KatanA `v0.22.37` candidate.

### 8. v0.4.10 Interactive Layout Performance

- [x] 8.1 Reproduce the real 3 MB `slides.html` first-frame and post-input
  latency directly through the in-process KRR session.
- [x] 8.2 Remove repeated recursive flex/grid measurement within a frame
  without caching layout state across JavaScript, input, hover, or resize
  updates.
- [x] 8.3 Add deterministic regression coverage for nested flow measurement
  reuse and retain strict 100% line coverage with 0 uncovered lines.
- [x] 8.4 Re-run the real slideshow through the registry-only KRR -> KDV ->
  KatanA chain and record first-frame and input-burst evidence.
- [x] 8.5 Pass the complete KRR release gate, publish and verify KRR `0.4.10`,
  then update KDV and KatanA through crates.io dependencies only.

### 9. v0.4.11 Same-origin Network Iframe Follow-up

- [x] 9.1 Reproduce the KatanA URL-viewer failure where a same-origin relative
  iframe is rejected after the host supplies the main HTTP document.
- [x] 9.2 Permit only same-origin `http`/`https` iframe documents while keeping
  cross-origin, `data:`, `file:`, filesystem escape, depth, count, cycle, and
  source-size restrictions enforced.
- [x] 9.3 Prove iframe CSS, JavaScript, `load`, and `contentDocument` behavior
  through an actual loopback HTTP transport integration test.
- [ ] 9.4 Pass strict 100% coverage and the complete release gate, publish and
  verify KRR `0.4.11`, then update KDV and KatanA through crates.io only.

## Local Verification Evidence

- KRR: KRR `0.4.8` is published and verified. The `0.4.9` candidate keeps the
  in-process Rust/V8 architecture and now proves Chrome CSS equivalence for the
  real 14-slide deck at the same 1230 x 867 CSS viewport. Structured contracts
  cover font-face resolution, mixed Japanese/Latin wrapping, inline
  fragmentation, dynamic hover selectors, nowrap, and gradient geometry. Its
  complete release gate passes 823 workspace tests, AST/Biome/TypeScript and
  asset checks, 16,796 / 16,796 covered lines, package size enforcement, and
  publish dry-run without exclusions or threshold changes.
- KDV: KDV `0.3.3` is published and verified and accepts the KRR `^0.4` patch
  line without a local path or Git dependency.
- KatanA: `v0.22.36` is published. The local `v0.22.37` candidate renders all
  14 slides from the actual Google Drive `slides.html`, passes 43 / 43 scripted
  actions, and changes slides by click and keyboard without a worker stop or
  JavaScript exception. Local `file://` and iframe-wrapper evidence remains
  pending until KRR `0.4.8` is visible on crates.io; KatanA publication remains
  user-gated.
