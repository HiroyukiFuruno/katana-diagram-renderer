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
- [/] 1.10 Release and verify KRR `0.4.6` with structured CSS parsing and
  cascade precedence, typed flex/grid/table/box/overflow/typography layout,
  browser-style capture/target/bubble event dispatch, and scroll/content frame
  metrics. Local `release-check` passes with 100% line coverage and 0 uncovered;
  crates.io publication and public artifact verification remain.

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
- [ ] 3.6 Run KatanA compile/unit/release contract gates with registry-only KDV
  after KDV publication; no path/git dependency may remain in committed files.

### 4. Release Order And User Review

- [x] 4.1 Pass all KRR quality/release gates, publish and verify KRR `0.4.0`.
- [x] 4.2 Pass all KDV quality/release gates, publish and verify KDV `0.3.0`.
- [x] 4.3 Publish and verify KRR `0.4.3`, resolve it from crates.io in KatanA,
  and rerun the complete headless acceptance matrix for `v0.22.33` only.
- [ ] 4.4 Present final registry-only state-specific screenshots and automated
  evidence to the user. Local-candidate evidence has already been presented.
  Until explicit approval: no KatanA commit, push, PR, publish, or release.
- [x] 4.5 Publish and verify KRR `0.4.4`, then allow KDV `0.3.2` and KatanA
  `v0.22.34` to consume only its crates.io artifact. KatanA evidence must cover
  remote resources, embedded Mermaid SVG, worker error recovery, and the full
  interactive HTML acceptance matrix without Chromium or WebView.
- [/] 4.6 Publish and verify KRR `0.4.6`, then refresh KatanA `v0.22.36` to the
  registry artifact and rerun strict coverage, release contracts, platform
  checks, and the headless interactive HTML evidence before user review.

## Local Verification Evidence

- KRR: KRR `0.4.5` is published and verified. The local `0.4.6` patch keeps the
  in-process Rust/V8 architecture and adds structured CSS parsing/cascade,
  browser-style event propagation, typed layout coverage, and frame scroll
  metrics. `rtk just check`, strict `rtk just coverage` (12,542 / 12,542 lines,
  100%, 0 uncovered), `release-verify`, `release-openspec-archive`, and the full
  `release-check` pass. The packaged crate remains below the unchanged 10 MiB
  limit. Only public release and registry verification remain.
- KDV: KDV `0.3.3` is published and verified and accepts the KRR `^0.4` patch
  line without a local path or Git dependency.
- KatanA: local `v0.22.36` acceptance covers CSS, accordion, JavaScript event
  propagation, text input, HTTP main-document and subresource acquisition,
  fragment and link navigation, reload, resize, worker diagnostics, and image
  controls. Registry-only coverage and release gates remain pending until KRR
  `0.4.6` is visible on crates.io; KatanA publication remains user-gated.
