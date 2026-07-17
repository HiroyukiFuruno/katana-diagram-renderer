## ADDED Requirements

### Requirement: KRR は browser-equivalent HTML page を所有しなければならない

システムは、KRR 内で browser engine の persistent page を起動し、HTML5 parsing、CSS cascade/layout/paint、JavaScript/Web API/event loop を engine に評価させなければならない（MUST）。KRR は KatanA から受け取った raw HTML の doctype、comment、head/body 構造、parser mode を host-side の文字列処理または static parser で変更してはならない（MUST NOT）。KRR は custom DOM bridge、独自 CSS layout、静的 HTML serialization を KatanA interactive viewer の source of truth として使ってはならない（MUST NOT）。KRR browser session を開始できない場合、KDV/KatanA は typed error を表示し、旧 static parser、HTML text normalization、export image へ fallback してはならない（MUST NOT）。

#### Scenario: JavaScript が画面を動的に変更する

- **WHEN** local HTML の script または user action が DOM、style、form control、timer を変更する
- **THEN** KRR は browser engine が repaint した最新 frame を返す
- **THEN** KDV または KatanA は DOM/CSS/JavaScript を解釈せずに最新 frame を表示する

#### Scenario: head metadata と executable content を HTML semantics で評価する

- **WHEN** HTML が `title`、`style`、`script` と interactive body content を含む
- **THEN** browser engine は head metadata、stylesheet、script を HTML semantics に従って評価する
- **THEN** KatanA surface は `title`、stylesheet source、script source を本文 text として表示しない
- **THEN** stylesheet と script の効果は browser frame と action 後の frame update に反映される

#### Scenario: raw HTML の parser mode を保持する

- **WHEN** local file の raw HTML が doctype を持たない、または comment 内に `<head>` text を含む
- **THEN** KRR は完全な file document URL の main response body を raw HTML のまま Chromium に供給する
- **THEN** host は doctype、`<base>`、navigation script を HTML text に挿入せず、Chromium が quirks/standards mode と document tree を決定する

### Requirement: KRR は browser frame を viewport と同じ surface として返さなければならない

システムは、KRR browser session の viewport と同じ寸法の pixel frame と generation を KDV に返さなければならない（MUST）。frame は一回限りの export image ではなく、repaint ごとに更新可能な interactive surface でなければならない（MUST）。DOM、layout、paint の一部だけが更新された場合も、KRR は変更領域と不変領域を含む完全な viewport frame を返さなければならない（MUST）。

#### Scenario: CSS layout が viewport 幅に追従する

- **WHEN** KDV が viewport resize を KRR browser session に渡す
- **THEN** KRR は browser engine の layout/paint を更新する
- **THEN** 後続 frame は新しい viewport width に対応する

#### Scenario: user action が viewport の一部だけを再描画する

- **WHEN** accordion、button、form input の action が DOM、layout、paint の一部だけを変更する
- **THEN** KRR は変更された領域だけの damage image ではなく、viewport 全体の pixel frame を返す
- **THEN** 後続 frame は action 前から不変の content と action 後の content を同時に含む

### Requirement: KRR browser runtime は検証可能な配布成果物でなければならない

システムは、KRR release helper と manifest で固定した Chromium bundle を同一の platform runtime archive として配布しなければならない（MUST）。archive は helper executable、helper 隣接の `chromium/<platform>`、Chromium source manifest、KRR license、runtime manifest を含み、linux64、mac-arm64、mac-x64、win64 ごとの archive checksum を公開しなければならない（MUST）。release workflow は immutable commit を checkout し、Chromium download の SHA-256 を再検証して fresh directory へ展開し、archive 内容、実行権限、全 platform asset の存在を検証しなければならない（MUST）。全 asset は draft GitHub Release へ揃えてから公開し、同一 release tag の既存 asset と内容が異なる場合は上書きしてはならない（MUST NOT）。

#### Scenario: KatanA release artifact に browser runtime を組み込む

- **WHEN** KatanA release workflow が公開済み KRR version の platform runtime archive を取得する
- **THEN** workflow は公開 checksum を検証してから helper と Chromium bundle を KatanA executable 隣接へ配置する
- **THEN** `HtmlRuntime::open_packaged` は開発機の Chrome または Cargo build directory に依存せず packaged helper を起動する

#### Scenario: platform asset が欠損または改変されている

- **WHEN** 4 platform の archive/checksum のいずれかが欠損する、checksum が一致しない、または同一 tag の既存 asset と新規 asset が異なる
- **THEN** release workflow は crates.io publish または KatanA packaging より前に失敗する
- **THEN** workflow は既存 asset を強制上書きしない

### Requirement: KatanA は主文書を供給し、KRR は subresource と host capability を policy で制御しなければならない

システムは、KatanA が local file または user-entered URL の主文書を取得し、raw HTML と完全な document URL origin を KDV 経由で KRR に供給しなければならない（MUST）。KRR は document URL origin を基準に許可された stylesheet、script、image などの subresource だけを browser page に解決しなければならない（MUST）。KDV はこの値を転送するだけとし、主文書を取得してはならない（MUST NOT）。KRR は network の許可外 origin、root 外 filesystem、subprocess、remote iframe を request policy で拒否しなければならない（MUST NOT）。

#### Scenario: local script と stylesheet を読み込む

- **WHEN** KatanA が workspace 内 HTML を raw HTML と完全な file document URL origin として供給し、relative local stylesheet と script を参照する
- **THEN** KRR browser page はそれらを評価して frame を更新する

#### Scenario: workspace 外 resource を参照する

- **WHEN** HTML が symlink escape、absolute path、許可外 origin の network URL、remote iframe を参照する
- **THEN** KRR は request を拒否し、KDV/KatanA process に host capability を渡さない

#### Scenario: new tab が許可外 origin を参照する

- **WHEN** browser page の user action が許可外 origin を指す new tab/window を生成する
- **THEN** KRR は browser-level auto-attach で新規 page target を main request より前に停止し、その target を閉じる
- **THEN** 許可外 origin の server には main-document request が到達しない
