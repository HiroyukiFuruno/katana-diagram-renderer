## Verbatim Requirements

- [ ] 0.1 KatanA は KDV を経由し、KRR がブラウザエンジンで評価・描画した HTML を表示する。KatanA 自身は HTML/CSS/JavaScript interpreter または browser surface を持たない。
- [x] 0.2 KRR は持続 browser page を source of truth とし、CSS layout、Web API、JavaScript event loop を browser engine に委譲する。V8 疑似 DOM、独自 CSS layout、正規表現 HTML parser、egui、KDV/KatanA WebView を採用しない。
  - Current verification: public `HtmlRuntime::open` returns the KRR-owned persistent browser session; Chromium child integration covers HTML/CSS/JavaScript, action-driven repaint, form input, timers/microtasks, CSS animation, navigation events, resource policy, and packaged Chromium resolution without `KRR_CHROME_BIN`. The local `html_browser_preview` example produces browser-rendered captures for initial render, accordion click, button DOM mutation, text input, and a link-navigation probe. `renderer_transfer` and `release_check` keep the static DOM/V8 bridge out of the interactive public API, and forbidden dependency checks keep egui, KDV, KatanA, and WebView out of KRR.
- [ ] 0.3 KRR `0.4.0`、KDV `0.3.0`、KatanA `v0.22.33` は公開済み crate だけで直列に接続し、最終 release は user review の明示 OK 後にだけ行う。
- [ ] 0.4 KatanA は local `.html` / `.htm` file と、ユーザーが入力した `http/https` URL の両方を HTML browser tab として開ける。KatanA は主文書を取得し、raw HTML と完全な document URL origin を KDV 経由で KRR browser session に渡す。KDV は転送のみとし、KDV/KatanA は HTML renderer を持たない。

## 1. Runtime Boundary And Version Target

- [x] 1.1 crates.io / GitHub Release の KRR `0.3.8` と KDV `0.2.7` を再確認し、release guard が KRR `0.4.0` と KDV `0.3.0` だけを受理することをテストする。
- [x] 1.2 未公開 KRR `0.3.9` static renderer は `0.4.0` DOM runtime の foundation として扱い、static-only version を publish target にしないことを tasks と release evidence に記録する。
- [x] 1.3 KRR が所有・配布する browser engine adapter を選定し、KRR dependency graph に egui、KDV、KatanA、KDV/KatanA WebView がないこと、browser binary/version/license を release artifact として検証する。
  - Current verification: KRR は `headless_chrome` adapter と helper child process で Chrome for Testing `150.0.7871.115` を所有する。`vendor/chromium/150.0.7871.115/manifest.json` に `BSD-3-Clause` license、mac-arm64/mac-x64/linux64/win64 の URL、SHA-256、実行パスを固定済み。`rtk cargo tree -p katana-render-runtime --edges normal --prefix none` と forbidden dependency grep で egui、KDV、KatanA、WebView/wry/tao/webkit が入っていないことを確認済み。
- [x] 1.4 KRR workspace version と internal dependency を `0.4.0` に更新し、version guard regression test を追加する。

## 2. KRR Browser Runtime

- [x] 2.1 現在の custom DOM/V8 bridge/CSS normalizer を browser-equivalent renderer の release candidate から除外し、KRR `HtmlRuntime` / `HtmlRuntimeSession` を browser page session に再編する。
  - Current verification: public `HtmlRuntime::open` now starts the KRR-owned browser `HtmlBrowserSession`, and public `HtmlRuntimeSession` is an alias of that browser session. The former custom DOM/V8 session is renamed `StaticHtmlRuntime`, kept crate-private for static HTML export only, and its click-dispatch bridge is test-only. `renderer_transfer` fixes the public browser-session contract, and `release_check::html_interactive_runtime_excludes_static_dom_bridge_api` prevents `HtmlRuntimeEvent` / `HtmlNodeId` / `HtmlRuntimeDispatch` / `HtmlNavigationIntent` from re-entering the public interactive API.
- [x] 2.2 KRR 内で browser page を起動し、HTML5 parser、CSS cascade/layout/paint、JavaScript/Web API/event loop を engine に評価させる。
  - Current verification: `krr-html-chromium-engine` launches Chrome for Testing through `headless_chrome`, loads raw HTML into a persistent page, and returns Chromium screenshots. `rtk cargo test -p katana-render-runtime --test html_browser_engine` covers inline CSS/JavaScript pixels, local stylesheet/script/image resources, timer/microtask JavaScript, CSS animation refresh, form input, and navigation events.
- [x] 2.3 KRR public API は viewport 指定、初期 frame、frame 更新通知、入力座標・keyboard・focus・resize・scroll を受ける persistent session を提供する。
  - Current verification: `HtmlBrowserSession` exposes viewport, initial/latest frame, one-shot `take_frame_update()`, explicit `refresh_frame()`, navigation events, pointer/key/text/focus/scroll/resize, and persistent process lifecycle. `HtmlBrowserInput::Focus` and the frame-update contract are covered by unit tests and `html_browser_engine` integration tests.
- [x] 2.4 KatanA から受け取る完全な document URL origin を基準に、許可された local CSS、image、script または同一 origin の `http/https` subresource だけを解決し、変更時は session を安全に reload する。主文書の file/URL 取得は KRR が行わない。host filesystem escape、subprocess、unsupported scheme は KRR の request policy で拒否する。
  - Current verification: KRR accepts raw HTML plus full document origin. For `http/https`, KRR navigates to the document URL and fulfills only the main-document request with injected raw HTML, so same-origin subresources use the real document origin. Integration tests cover same-origin HTTP script/CSS, blocked cross-origin redirect script, blocked cross-origin iframe, local resource loading, local filesystem escape rejection, and safe session reload through `navigate`.
- [x] 2.5 browser engine が生成した viewport と同一寸法の pixel surface を KDV に返す。KDV/KatanA は DOM、CSS、layout、hit-test を解釈しない。
  - Current verification: `HtmlBrowserFrame` validates RGBA buffer size against the requested viewport, Chromium screenshot dimensions are checked against viewport, resize integration updates the frame to the new viewport, and KDV/KatanA-side DOM/CSS/layout/hit-test implementation is still prohibited.
- [x] 2.6 JavaScript exception、navigation failure、runtime crash、execution timeout を typed error に変換し、古い frame を成功扱いにしない。
  - Current verification: child wire error codes are mapped to public `HtmlBrowserEngineErrorCode` values (`invalid_message`, `protocol_version`, `invalid_request`, `chromium`, `not_loaded`, `stdin_read`, and unknown fallback) through `HtmlBrowserError::EngineRejected`; process timeout/crash paths remain typed as `ProcessTimeout` / `ProcessCrashed`; `accept_response_errors_do_not_republish_stale_frame_updates` fixes stale-frame behavior after error responses. Integration coverage fixes protocol errors, invalid UTF-8 stdin read errors, Chromium launch failures, missing packaged/override browser paths, and local document write failures.
- [x] 2.7 browser engine process の lifecycle、resource limit、crash recovery を KRR が管理し、KDV/KatanA process に browser capability を漏らさない。
  - Current verification: KRR owns the browser child through `HtmlBrowserProcess` / `HtmlBrowserSession`; child processes are terminated on timeout, explicit close, and Drop. Process-level failures detach the child handle, `recover_process()` restarts the KRR-owned helper with the current raw HTML and viewport, and tests fix crash detach/recovery plus process-failure classification. Resource limits remain KRR-side through source size validation, viewport validation, request timeout, latest-frame coalescing, and request policy; KDV/KatanA only see typed errors, frames, input, and navigation events.

## 3. Interaction And Navigation Contract

- [x] 3.1 pointer、keyboard、text input、focus、scroll、resize を KDV から KRR browser session へ渡し、JavaScript action 後の frame を KDV へ返す。
  - Current verification: `html_browser_engine` forwards pointer click, keyboard, text input, surface focus, scroll, resize, and explicit frame refresh through `HtmlBrowserSession`; JavaScript action frames are returned as browser pixel frames without KDV-side DOM/hit-test semantics.
- [x] 3.2 `onclick`、`addEventListener`、form control、timer、microtask、CSS animation を browser engine の通常動作として評価し、KDV が action semantics を再実装しない。
  - Current verification: Chromium integration tests cover link/button click handling, `addEventListener`, form input listeners, timer-driven DOM updates, Promise microtasks, CSS animation frame refresh, and `preventDefault` respecting browser event semantics.
- [ ] 3.3 KatanA は URL input action から `http/https` の主文書を取得して browser tab を開く。local link と URL navigation は KRR の navigation event として KDV 経由で返す。KatanA は navigation target の次の主文書を取得して raw HTML と完全な document URL origin を再投入し、KRR は許可された subresource の URI 解決だけを担う。KDV は転送のみとし、KDV/KatanA は DOM navigation を実装しない。
- [x] 3.4 KRR request policy が host filesystem escape、subprocess、unsupported scheme を拒否し、`file` と `http/https` の allowed URL、redirect、iframe policy を integration test で固定する。
  - Current verification: `html_browser_engine` fixes local allowed resources and local outside-directory blocking; `html_browser_resource_policy` fixes same-origin HTTP resources, cross-origin redirect blocking, and cross-origin iframe blocking; policy unit tests fix same-origin HTTP, local canonical children, malformed URL, unsupported scheme, and outside-file rejection.

## 4. Regression And Release Gates

- [x] 4.1 browser engine 実測で HTML/CSS layout、external local stylesheet/script/image、DOM mutation、form input、timer/microtask、scroll、resize を KRR integration test に追加する。
  - Current verification: `rtk cargo test -p katana-render-runtime --test html_browser_engine` covers HTML/CSS layout pixels, local stylesheet/script/image resources, DOM mutation through JavaScript, form input, timer/microtask updates, scroll, resize, and browser frame refresh.
- [x] 4.2 browser engine 実測で click handler、`addEventListener`、internal navigation、`preventDefault`、CSS animation frame 更新を KRR integration test に追加する。
  - Current verification: `html_browser_engine` covers click handler and `addEventListener` dispatch, local link navigation events, page-level `preventDefault` without KDV navigation semantics, and CSS animation updates via `HtmlBrowserSession::refresh_frame()`.
- [x] 4.3 KRR public API / dependency graph / rustfmt / clippy / workspace tests / `just coverage` を実行し、line coverage `100% / 0 uncovered` を通す。
  - Current verification: `rtk just check` passed; workspace tests 589 passed, AST lint passed, runtime asset checks passed, Chromium manifest/install helper checks passed. `rtk just coverage` passed strict `--fail-under-lines 100 --fail-uncovered-lines 0` with `TOTAL ... Lines 8018, Missed Lines 0, Cover 100.00%`. `rtk cargo test -p katana-render-runtime --test html_browser_engine` passed with 21 tests and no `KRR_CHROME_BIN`.
- [x] 4.4 `just VERSION=0.4.0 release-verify`、package 内容、publish dry-run、OpenSpec strict validation を通す。user approval 前に commit、push、PR、release を行わない。
  - Current verification: `rtk just VERSION=0.4.0 release-verify` passed; Chromium archive install/check gate passed, tag safety and unpublished crates.io target checks passed, package checks passed, and `cargo publish --dry-run` only was executed. `rtk ./scripts/openspec validate v0-4-0-html-dom-runtime --strict` passed. No commit, push, publish, or release was performed.

## 5. KDV And KatanA Handoff

- [ ] 5.1 KRR `0.4.0` が crates.io 公開済みであることを確認した後にだけ、KDV を crates.io `^0.4.0` に更新する。
  - Current verification: read-only public checks on 2026-07-13 show crates.io latest `katana-render-runtime` / `katana-render-runtime-cli` is still `0.3.8`, latest `katana-document-viewer` is still `0.2.7`, GitHub release `v0.4.0` is not found, and remote tag `refs/tags/v0.4.0` is absent. KDV work remains blocked until KRR `0.4.0` is actually published; no publish/release/push/commit was performed.
- [ ] 5.2 KDV の HTML→PDF/画像 export 実装は維持する。KatanA interactive viewer 用の direct HTML parser / CSS cascade / visibility / table normalizer と image control だけを、KRR browser frame surface、入力中継、frame 更新、navigation event の consumer に置換して KDV `0.3.0` の release gate を通す。
- [ ] 5.3 KDV `0.3.0` が crates.io 公開済みであることを確認した後にだけ、KatanA が Cargo で KDV を解決する。
- [ ] 5.4 KatanA native window で browser-equivalent CSS layout、JavaScript action、input、internal navigation、reload を確認する。証跡取得は browser runtime が動作し、ユーザーが再開を指示した後にだけ行う。
- [ ] 5.5 user review に実機操作可能な確認材料、browser capability policy、KRR → KDV → KatanA responsibility boundary を提示し、明示 OK まで KatanA `v0.22.33` を release しない。
