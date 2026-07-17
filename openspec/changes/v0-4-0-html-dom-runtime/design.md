## Context

v0.22.33 の HTML viewer は、ブラウザと同じ HTML/CSS/JavaScript の表示と動作を KatanA の document surface 内で提供する。現在の custom HTML parser、CSS normalizer、V8 DOM bridge は限定 API の静的 content を返すだけで、一般的な Web API、form input、timer、layout/paint、page lifecycle を表現できない。画像 snapshot は action 後に更新しても、browser session そのものにはならない。

この不足は KDV や KatanA に補完実装を足して解決してはならない。HTML semantics を複数層に分散させると、互換性、性能、security policy、更新検知が一致しなくなるためである。

## Goals / Non-Goals

**Goals:**

- KRR が Rust 製 browser engine を保持し、HTML5 parsing、CSS layout/paint、JavaScript/Web API/event loop を一つの persistent page session として所有する。
- KDV が KRR page の pixel surface を表示し、pointer、keyboard、IME/text input、focus、scroll、resize を中継する。
- KatanA は KDV browser surface を document tab に配置し、既存の focus と document navigation 導線を接続するだけにする。
- local HTML と workspace 内の local stylesheet、script、image、および `http/https` URL を読み込み、編集後は coalesced reload で browser session を更新する。
- workspace 外 filesystem、subprocess、unsupported scheme を KRR request policy で拒否する。network、redirect、iframe は URL policy で明示制御する。

**Non-Goals:**

- KDV または KatanA に DOM/CSS/JavaScript/layout/hit-test を再実装しない。
- `egui` を KRR の dependency にしない。
- arbitrary remote content、browser extension、host process access を許可しない。

## Decisions

### KRR が embedded browser engine を所有する

KRR は Chromium browser process と、その engine binary/version/license manifest を公開 artifact として所有する。CDP Rust adapter は KRR の process から Chromium を管理するためだけに使い、KRR 自身は DOM/CSS/JavaScript を実装しない。Chromium binary は release artifact として KRR が version-lock して配布する。

KRR は既存 diagram runtime の `rusty_v8` を保持するため、browser engine は KRR が起動・監督する専用 Rust process とする。KRR の public session は IPC を隠蔽し、KDV/KatanA はこの process を知識として持たない。

Servo は form value を更新できず browser-equivalent fixture に不合格だったため採用しない。KDV/KatanA に platform WebView を直接持たせる案は責務境界に反するため採用しない。

### persistent page と frame stream を public contract にする

`HtmlRuntime::open` は KatanA source host が取得した raw HTML、完全な document URL origin、viewport、request policy を受け、browser page を保持する `HtmlBrowserSession` を返す。session は initial frame と、page が repaint を要求した時の frame update を返す。frame は viewport と同一の pixel buffer と generation を持つ。

KDV は reusable browser-session adapter として、KatanA から raw HTML と完全な document URL origin を受け、そのまま KRR session の lifecycle、frame surface、input forwarding、navigation event を接続する。同期的な KRR session は KDV の専用 worker thread が所有し、typed channel で request と latest frame/navigation/error を中継して consumer の UI thread を block しない。pointer move と repaint request は coalesce できるが、key、text、focus、resize、navigation は順序または event を失わない。KDV は DOM node、CSS property、クリック領域、主文書取得を知識として持たない。pointer/keyboard/text/focus/scroll/resize は座標・入力 event のまま KRR へ渡す。KRR は browser engine の hit-test と event dispatch を使うため、JavaScript action、form control、timer、animation、layout 更新は engine が一貫して処理する。browser process からの frame IPC は generation と latest-frame coalescing を持ち、未描画 frame を蓄積しない。

KatanA は source host として local file または user-entered URL から raw HTML と origin を取得し、URL input、tab、history を管理する。この境界により、将来 KatanA UI を KUC に置き換えても KDV/KRR browser adapter は再利用できる。

### 主文書取得を KatanA に、subresource policy を KRR に閉じる

KatanA は local file または user-entered URL から主文書を取得し、raw HTML と完全な document URL origin を KDV 経由で KRR に渡す。KRR は完全な `file/http/https` document URL へ browser navigation を開始し、その最初の top-level main-document request だけを KatanA 由来の raw HTML body で fulfill する。raw HTML に doctype、`<base>`、navigation script を挿入せず、comment/head/body/quirks semantics は Chromium parser だけが決定する。KRR は click target または `href` を host script で解釈せず、Chromium が event listener、`preventDefault()`、`javascript:` URL、same-document hash、default action を評価した後に発生した top-level `Document` request を CDP Fetch で確定して navigation event にする。new tab/window は browser-level `Target.setAutoAttach` の `waitForDebuggerOnStart` で page target を main request より前に停止し、Chromium が event dispatch 後に発行した `Page.windowOpen` URL と停止 target を対応付けてから target を閉じる。`preventDefault()` で `Page.windowOpen` が発生しない action、または停止 target を伴わない host-side URL は navigation event にしない。KRR は document origin を基準に許可された CSS/script/image などの subresource だけを解決する。canonical path が root 外へ出る dependency、許可外 origin、remote iframe、subprocess request は engine request interceptor で拒否する。browser page が main-document navigation を要求した時は KRR が normalized navigation event を KDV 経由で返し、KatanA が次の主文書を取得して session に再投入する。KDV はこの経路を転送するだけである。

source save または許可済み local dependency の変更は、KatanA の save action と watcher event を coalesce して KRR session reload を要求する。auto-save 0 は有効値のまま保ち、input の有無と before/after revision を trigger にし、継続的なファイル全読込 polling はしない。

### browser binary は crate version と一体で管理する

KRR の Rust API だけでは engine 実行に必要な asset を表せない。KRR release artifact は engine binary、version、license、SHA-256、platform matrix を manifest に固定し、package/release gate で検証する。公開前の開発・end-to-end 検証では isolated worktree と一時的な Cargo patch で KRR、KDV、KatanA を接続できる。ただし path/git override は最終 commit、lockfile、package、release artifact に残さず、release 判定は公開済み KRR/KDV crate と checksum 検証済み runtime asset だけで再実行する。

## Risks / Trade-offs

- [engine の Web 標準互換が要件に届かない] → browser-equivalent fixture と user workflow を POC acceptance gate にし、engine 名では合格にしない。
- [bundle size / startup time が軽量性を損なう] → lazy page startup、shared engine process、viewport frame only、idle session disposal を計測して release gate に含める。
- [continuous animation が無制限に frame を流す] → KRR が repaint を coalesce し、latest-frame only の IPC、visible surface の frame cadence、memory 上限を管理する。KatanA/KDV は source document を保持しない。
- [部分 repaint を完全 frame と誤認する] → browser action の連続操作ごとに、変更領域と不変領域が同一 viewport frame に残ることを KRR integration test と packaged-runtime preview で固定する。KDV/KatanA は damage image の合成や HTML semantics の補完を行わない。
- [local resource が workspace 外を読む] → request interceptor と canonical path validation を integration test で固定する。
- [new tab/window が policy 判定より先に外部通信する] → browser-level auto-attach で新規 page target を debugger pause し、main request が外部 server に到達していないことを integration test で固定する。
- [KDV/KatanA が viewer に暫定 parser を残す] → KatanA interactive viewer path から direct HTML parser/CSS/visibility/table normalizer を外し、source boundary test で禁止する。KDV の HTML→PDF/画像 export path は対象外として維持する。
- [KRR/KDV の version が飛ぶ] → 最初の release は KRR `0.4.0`、KDV `0.3.0` を guard で検証する。統合検証で不備を検出した場合は KRR `0.4.x` または KDV `0.3.x` の隣接 patch を公開し、KatanA `v0.22.33` の dependency、runtime asset、全 acceptance を更新して再実行する。
- [検証を通すために品質基準が緩和される] → coverage `100% / 0 uncovered`、lint、AST、integration、release asset gate の threshold、対象、失敗条件を緩和・除外・ignore しない。不合格は実装またはテスト不足として修正する。

## Migration Plan

1. KRR で browser engine POC を作り、browser-equivalent fixture、input、frame update、resource policy、platform packaging を実測する。
2. POC acceptance を通した engine を KRR `HtmlBrowserSession` と frame/input/navigation contract に固定し、custom DOM/V8/CSS renderer を release candidate から除外する。
3. KRR の strict coverage、package、engine artifact manifest、publish dry-run を完了し、同時に isolated worktree の一時的な local patch で KDV adapter と KatanA end-to-end integration を開始する。
4. KRR `0.4.0` を公開し、KDV の一時的な patch を crates.io KRR `^0.4.0` へ置換して同じ gate を再実行する。KatanA interactive viewer の direct HTML parser/CSS modules と image control を browser surface adapter に置換し、既存の HTML→PDF/画像 export path は維持して KDV `0.3.0` を公開する。
5. KatanA の一時的な patch を crates.io KDV `0.3.0` へ置換し、checksum 検証済み KRR runtime asset を含む native document surface で browser workflow を再確認する。不備があれば KRR/KDV の隣接 patch を公開して再検証し、KatanA `v0.22.33` は user review の明示 OK 後にだけ release する。

## Open Questions

- Chrome `150.0.7871.115` は KRR child process の CDP POC で raw HTML の form input と JavaScript handler を実行できた。latest-frame IPC、local resource policy、input を browser process で実装する。
- engine artifact を KRR crate package に含めるか、KRR release asset として version-lock して配布するか。
- CSS animation の visible frame cadence と background tab の suspension policy。
