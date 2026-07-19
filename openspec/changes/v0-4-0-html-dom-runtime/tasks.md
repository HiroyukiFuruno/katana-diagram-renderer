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

### 2. KDV Adapter

- [x] 2.1 Replace the rejected external-browser session worker with a KRR
  in-process session adapter that transports raw source, frame, input, resize,
  typed error, and navigation without parsing HTML.
- [/] 2.2 Add adapter contract tests for ordered input/frame/navigation lifecycle
  against KRR and pass KDV strict coverage and release checks.
- [ ] 2.3 After KRR `0.4.0` publication, resolve only crates.io `^0.4.0`, release
  KDV `0.3.0`, and prove the published artifact before KatanA registry use.

### 3. KatanA Integration

- [x] 3.1 Replace the static HTML preview path with KDV frame presentation and
  raw native input forwarding; KatanA owns neither DOM nor CSS/layout/hit-test.
- [x] 3.2 Make navigation replace KatanA's active document path/source before a
  reload or resize, preventing stale initial-document state from reappearing.
- [x] 3.3 Exercise user-entered `http/https` main-document acquisition in an
  automated end-to-end scenario and retain the raw URL origin through reload.
- [x] 3.4 Capture native-window evidence for CSS, accordion, button, input,
  local link navigation, reload, and resize using state-specific assertions.
- [ ] 3.5 Run KatanA compile/unit/release contract gates with registry-only KDV
  after KDV publication; no path/git dependency may remain in committed files.

### 4. Release Order And User Review

- [ ] 4.1 Pass all KRR quality/release gates, publish and verify KRR `0.4.0`.
- [ ] 4.2 Pass all KDV quality/release gates, publish and verify KDV `0.3.0`.
- [ ] 4.3 Resolve KDV `0.3.0` from crates.io in KatanA and rerun the complete
  native acceptance matrix for release target `v0.22.33` only.
- [ ] 4.4 Present state-specific screenshots and automated evidence to the user.
  Until explicit approval: no KatanA commit, push, PR, publish, or release.

## Local Verification Evidence

- KRR: `rtk just check`, strict `rtk just coverage`, the interactive preview
  example, and `release-target-check` / `release-verify` for `0.4.0` passed.
- KDV: adapter contract integration, adapter 100% coverage, `rtk just check`,
  `rtk just coverage`, and regenerated native Storybook acceptance artifacts
  passed with the
  temporary local patch used only for pre-publication integration. Its release
  DoD correctly remains blocked on tracked source and refreshed human review.
- KatanA: native-window acceptance captured CSS, accordion, JavaScript action,
  text input, local navigation, reload, and resize. Registry-only preflight
  correctly remains blocked until KRR and KDV are published. The optimized
  KatanA binary reproduced the same initial, button-action, and navigation
  screenshots byte-for-byte as the debug binary. Its user-entered HTTP main
  document test also kept the raw origin through refresh and the KRR browser
  session: `rtk cargo test -p katana-ui --locked
  user_entered_http_document_keeps_origin_through_refresh_and_browser_session
  -- --test-threads=1`.
