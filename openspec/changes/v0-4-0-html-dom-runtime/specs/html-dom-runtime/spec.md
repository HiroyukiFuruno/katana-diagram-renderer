## ADDED Requirements

### Requirement: KRR は browser-equivalent HTML page を所有しなければならない

システムは、KRR 内の in-process Rust/V8 persistent session で HTML parsing、CSS cascade/layout/paint、JavaScript event semantics、input、navigation を評価しなければならない（MUST）。KRR は KatanA から受け取った raw HTML を doctype、comment、head/body 構造を壊す host-side 文字列変換で書き換えてはならない（MUST NOT）。interactive runtime は既存の Rust/V8 DOM/CSS primitives を source of truth として強化し、static HTML export/render path を interactive viewer に転用してはならない（MUST NOT）。Chromium、WebView、外部 helper process、browser binary は runtime、test、release artifact に導入してはならない（MUST NOT）。session を開始できない場合、KDV/KatanA は typed error を表示し、static export image へ fallback してはならない（MUST NOT）。

#### Scenario: JavaScript が画面を動的に変更する

- **WHEN** local HTML の script または user action が DOM、style、form control、timer を変更する
- **THEN** KRR は browser engine が repaint した最新 frame を返す
- **THEN** KDV または KatanA は DOM/CSS/JavaScript を解釈せずに最新 frame を表示する

#### Scenario: 一つの script が実行時例外を送出する

- **WHEN** interactive document の一つの script が通常の JavaScript 実行時例外を送出する
- **THEN** KRR は document URL と例外位置を structured runtime log に記録する
- **THEN** KRR は後続 script、lifecycle event、layout、frame 出力を継続する
- **THEN** JavaScript 構文エラー、DOM bridge failure、execution timeout は session 起動失敗として返す

#### Scenario: head metadata と executable content を HTML semantics で評価する

- **WHEN** HTML が `title`、`style`、`script` と interactive body content を含む
- **THEN** browser engine は head metadata、stylesheet、script を HTML semantics に従って評価する
- **THEN** KatanA surface は `title`、stylesheet source、script source を本文 text として表示しない
- **THEN** stylesheet と script の効果は browser frame と action 後の frame update に反映される

#### Scenario: raw HTML の parser mode を保持する

- **WHEN** local file の raw HTML が doctype を持たない、または comment 内に `<head>` text を含む
- **THEN** KRR は完全な document URL origin と raw HTML を in-process runtime に渡す
- **THEN** host は doctype、`<base>`、navigation script を HTML text に挿入せず、KRR parser/runtime が document tree を決定する

### Requirement: KRR は browser frame を viewport と同じ surface として返さなければならない

システムは、KRR browser session の viewport と同じ寸法の pixel frame と generation を KDV に返さなければならない（MUST）。frame は一回限りの export image ではなく、repaint ごとに更新可能な interactive surface でなければならない（MUST）。DOM、layout、paint の一部だけが更新された場合も、KRR は変更領域と不変領域を含む完全な viewport frame を返さなければならない（MUST）。

#### Scenario: CSS layout が viewport 幅に追従する

- **WHEN** KDV が viewport resize を KRR browser session に渡す
- **THEN** KRR は browser engine の layout/paint を更新する
- **THEN** 後続 frame は新しい viewport width に対応する

### Requirement: HTML runtime は opt-in の段階別性能診断を保持しなければならない

システムは、将来の性能劣化を再現時に切り分けられるよう、`DEBUG=true` のときだけ interactive HTML session の起動および各 frame の段階別所要時間を structured log に出力しなければならない（MUST）。通常実行では時刻計測とログ文字列生成を行ってはならない（MUST NOT）。HTML source、document URL、入力値などの内容を診断ログへ含めてはならない（MUST NOT）。

#### Scenario: 開発者が HTML 性能診断を有効にする

- **WHEN** host process が `DEBUG=true` で interactive HTML session を起動する
- **THEN** KRR は session と frame の相関 ID、DOM parse、subresource load、V8 setup、script execution、DOM/CSS projection、layout/SVG、SVG rasterize、frame store、frame total の所要時間を出力する
- **THEN** KRR は node、hit target、SVG/RGBA byte など内容を含まない集計値だけを併記する

#### Scenario: 通常のリリース実行では性能診断を無効にする

- **WHEN** `DEBUG` が未指定または `true` 以外である
- **THEN** KRR は HTML 性能診断ログを出力しない
- **THEN** KRR は性能診断のための phase timestamp と log message を生成しない

#### Scenario: user action が viewport の一部だけを再描画する

- **WHEN** accordion、button、form input の action が DOM、layout、paint の一部だけを変更する
- **THEN** KRR は変更された領域だけの damage image ではなく、viewport 全体の pixel frame を返す
- **THEN** 後続 frame は action 前から不変の content と action 後の content を同時に含む

#### Scenario: HTML slide deck が viewport と script state に追従する

- **WHEN** HTML が viewport-relative length、`clamp()`、linear gradient、明示的な `<br>`、class による slide visibility を使用する
- **THEN** KRR は active viewport に対して CSS 値を解決し、gradient、寸法、強制改行、active slide だけを browser-equivalent frame に描画する
- **WHEN** pointer または keyboard handler が `Element.closest()` を使って対象を判定し、active class と page indicator を変更する
- **THEN** KRR は selector matching と ancestor traversal を評価し、JavaScript exception なしに次の slide を全面再描画する

### Requirement: KRR in-process runtime は外部 browser asset なしに検証可能でなければならない

システムは、KRR crate に含まれる Rust/V8 runtime だけで interactive session を起動できなければならない（MUST）。release workflow は dependency graph、package contents、runtime configuration を検証し、Chromium binary/archive/manifest、browser download、external helper、`KRR_CHROME_BIN` のような browser override が含まれないことを確認しなければならない（MUST）。

#### Scenario: KatanA release artifact が外部 browser runtime を要求しない

- **WHEN** KatanA release workflow が公開済み KRR version を解決する
- **THEN** workflow は crates.io package と application dependency だけを検証し、browser bundle を取得または executable 隣接へ配置しない
- **THEN** `HtmlRuntime::open` は開発機の Chrome、Keychain、Cargo build directory、external helper に依存しない

#### Scenario: forbidden external browser asset が混入している

- **WHEN** package、workflow、runtime configuration が Chromium/WebView/browser download/helper を参照する
- **THEN** release workflow は publish または KatanA packaging より前に失敗する
- **THEN** workflow は external browser asset を追加して問題を回避しない

### Requirement: KatanA は主文書を供給し、KRR は subresource と host capability を policy で制御しなければならない

システムは、KatanA が local file または user-entered URL の主文書を取得し、raw HTML と完全な document URL origin を KDV 経由で KRR に供給しなければならない（MUST）。KRR は document URL origin を基準に許可された stylesheet、script、image などの subresource だけを in-process runtime に解決しなければならない（MUST）。KDV はこの値を転送するだけとし、主文書を取得してはならない（MUST NOT）。KRR は network の許可外 origin、root 外 filesystem、subprocess、cross-origin iframe を resource policy で拒否しなければならない（MUST NOT）。

#### Scenario: local script と stylesheet を読み込む

- **WHEN** KatanA が workspace 内 HTML を raw HTML と完全な file document URL origin として供給し、relative local stylesheet と script を参照する
- **THEN** KRR runtime はそれらを評価して frame を更新する

#### Scenario: dynamic application が同一 origin の text resource を取得する

- **WHEN** interactive script が `XMLHttpRequest` の `GET` で document と同一 origin の relative text resource を取得する
- **THEN** KRR は既存の subresource policy と transport を通して resource を取得し、`load` または `error` event を dispatch する
- **THEN** controlled host I/O の待機時間を JavaScript execution timeout に算入しない
- **THEN** cross-origin、mixed-content、`POST` などの許可外 request は取得せず、当該 request の `error` event 後も document frame を継続する

#### Scenario: local wrapper が同一ディレクトリの iframe を読み込む

- **WHEN** file document が同一ディレクトリの relative HTML を iframe として参照する
- **THEN** KRR は子文書、relative stylesheet、script、image を同じ in-process Rust/V8 session で評価する
- **THEN** iframe の `contentDocument`、`load` event、synthetic `click()`、完全な document URL の `location` を評価し、wrapper script が要求した状態を frame に描画する
- **THEN** iframe の深さ、文書数、cycle、source size を上限で制御する

#### Scenario: local iframe を読み込めない

- **WHEN** iframe が欠落ファイル、root 外、別ディレクトリ、容量超過、無効 URL、cycle、または上限超過を参照する
- **THEN** KRR は主文書 session を停止せず、当該 iframe 内に document URL、resource、原因を含む診断を表示する

#### Scenario: network document が同一 origin の iframe を読み込む

- **WHEN** `http` または `https` document が同一 origin の HTML を iframe として参照する
- **THEN** KRR は iframe document、CSS、JavaScript、`load` event、`contentDocument` を同一 in-process runtime 内で評価する
- **THEN** cross-origin、`data:`、`file:` iframe は取得せず、当該 iframe 内に原因を表示する
- **THEN** KRR は同じ原因を structured runtime log に出力する

#### Scenario: workspace 外 resource を参照する

- **WHEN** HTML が symlink escape、absolute path、許可外 origin の network URL、remote iframe を参照する
- **THEN** KRR は request を拒否し、KDV/KatanA process に host capability を渡さない

#### Scenario: new tab が許可外 origin を参照する

- **WHEN** runtime の user action が許可外 origin を指す navigation/new-window request を生成する
- **THEN** KRR は request policy でそれを拒否し、navigation event を返さない
- **THEN** 許可外 origin の resource を取得しない
