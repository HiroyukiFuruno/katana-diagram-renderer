## Context

KatanA v0.22.33 の HTML viewer は、local file と user-entered `http/https`
document を、CSS と JavaScript を含む持続的な interactive surface として表示する。
過去に検討された Chromium、Chrome for Testing、WebView、external helper process、
browser binary/archive は、この release line では明示的に不採用である。過去の試験や
asset は現行の実装・検証・release evidence に使用しない。

KRR が既存の Rust/V8 DOM/CSS/JavaScript primitives を in-process session として
保持し、KDV と KatanA は HTML semantics を持たない接続層に限定する。

## Goals

- KRR が raw HTML、完全な document URL origin、viewport を受け取り、DOM、CSS
  cascade/layout/paint、JavaScript event semantics、hit-test、input、navigation を
  一つの persistent session で所有する。
- KRR が viewport と同寸法の RGBA frame と単調増加 generation を返し、action 後も
  frame を更新できる。
- KDV は KRR session を worker 上で管理し、frame、input、resize、navigation、typed
  error を転送するだけにする。
- KatanA は主文書を取得して raw HTML と origin を KDV に渡し、native surface で frame
  と raw input を表示・転送する。navigation target の次の主文書取得も KatanA が行う。
- strict coverage を `100% / 0 uncovered` のまま満たし、HTML capability を action
  driven contract と native-window evidence の両方で確認する。

## Non-Goals

- Chromium、WebView、external helper process、browser download、browser runtime asset を
  導入しない。
- KDV/KatanA に HTML parser、CSS cascade/layout、JavaScript interpreter、browser
  hit-test、DOM navigation を追加しない。
- static HTML export/image conversion を interactive viewer の fallback として使わない。
- KRR が KatanA の主文書取得、workspace 操作、任意 host process を担わない。

## Responsibility Boundary

| Layer | Owns | Must not own |
| --- | --- | --- |
| KRR | in-process Rust/V8 page session, DOM/CSS/JS, layout, raster, hit-test, input dispatch, navigation intent, resource policy | external browser runtime, KatanA filesystem workflow, UI toolkit |
| KDV | session worker, ordered input/frame/navigation/error transfer | HTML/CSS/JS interpretation, document fetch, hit-test |
| KatanA | local/URL main-document fetch, tabs/history, native frame host, raw input forwarding | parser, cascade, layout, script execution, link resolution |

`HtmlRuntime::open` returns the persistent KRR session. A session starts with an
initial frame, accepts focus/pointer/key/text/scroll/resize input, and exposes
latest frame and normalized navigation events. Frame generation is monotonic;
KDV may coalesce obsolete frames but may not synthesize them.

## Document And Navigation Flow

1. KatanA reads a local file or fetches the entered `http/https` main document.
2. KatanA passes the unchanged raw HTML and the full document URL to KDV.
3. KDV transfers both values unchanged to KRR and starts/replaces the session.
4. KRR parses and evaluates the document in-process, returning full frames.
5. On a permitted link/navigation intent, KRR returns only the normalized URL.
6. KatanA fetches that next main document and repeats steps 2-4.

The host never injects a doctype, `<base>`, navigation script, or textual
rewrite. `head` metadata and executable content must be evaluated as HTML
semantics and must not appear as body text. KRR resolves allowed subresources
against the document origin. Unsupported schemes, workspace escapes,
disallowed origins, iframe/process capability, and unapproved external access
are rejected inside KRR policy.

## Interaction And Rendering Contract

KRR owns the target geometry used by click dispatch. KatanA/KDV forward only
coordinates and raw input. The acceptance matrix must include all of:

- CSS visual state and head metadata suppression;
- button `onclick` and `addEventListener` DOM/style mutation;
- `details`/`summary` accordion state;
- text input plus script-observable value mutation;
- relative local link navigation, host document replacement, reload, and resize;
- table, list, horizontal rule, wrapping, and ordinary styled content;
- valid and invalid focus, pointer, keyboard, scroll, resize, and session
  lifecycle paths.

Tests must verify semantics and frame properties, not merely that a method
returned successfully. The native KatanA scenario additionally captures the
before/after frames and asserts a distinct navigated document state, preventing
the initial document from being mistaken for a successful link action.

## Release And Quality Gates

- `just coverage` remains `100% / 0 uncovered`; exclusions, threshold changes,
  and ignored tests are prohibited.
- Static lint, formatter, AST rules, package contents, and release scripts must
  reject Chromium/WebView/external browser/helper references in this path.
- KRR `0.4.0` and KDV `0.3.0` are published and verified in sequence before
  KatanA resolves registry dependencies. Development may use an uncommitted
  worktree-local Cargo patch only.
- Any downstream defect is corrected by the adjacent KRR `0.4.x` or KDV
  `0.3.x` patch and the complete acceptance matrix is rerun.
- KatanA's only release target is `v0.22.33`; its SemVer guard rejects
  `v0.29.0` and non-adjacent versions. Commit, push, PR, publish, and release
  require explicit user approval after visual review.

## Risks And Controls

- [Semantic drift between layers] KRR-only execution plus source boundary tests
  prevent duplicate parsers or hit-tests.
- [False visual success] action screenshots use state-specific pixel markers
  and active-document assertions; link/reload/resize run on the navigated page.
- [Coverage gaming] the existing strict threshold is a release chokepoint and
  every unexecuted production path requires a behavioral contract test.
- [Remote capability escalation] KatanA fetches only the main document and KRR
  validates all subresource/navigation origins before use.
