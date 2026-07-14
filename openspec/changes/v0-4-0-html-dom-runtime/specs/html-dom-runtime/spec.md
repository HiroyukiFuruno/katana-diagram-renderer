## ADDED Requirements

### Requirement: KRR は browser-equivalent HTML page を所有しなければならない

システムは、KRR 内で Rust 製 browser engine の persistent page を起動し、HTML5 parsing、CSS cascade/layout/paint、JavaScript/Web API/event loop を engine に評価させなければならない（MUST）。KRR は custom DOM bridge、独自 CSS layout、静的 HTML serialization を KatanA interactive viewer の source of truth として使ってはならない（MUST NOT）。

#### Scenario: JavaScript が画面を動的に変更する

- **WHEN** local HTML の script または user action が DOM、style、form control、timer を変更する
- **THEN** KRR は browser engine が repaint した最新 frame を返す
- **THEN** KDV または KatanA は DOM/CSS/JavaScript を解釈せずに最新 frame を表示する

### Requirement: KRR は browser frame を viewport と同じ surface として返さなければならない

システムは、KRR browser session の viewport と同じ寸法の pixel frame と generation を KDV に返さなければならない（MUST）。frame は一回限りの export image ではなく、repaint ごとに更新可能な interactive surface でなければならない（MUST）。

#### Scenario: CSS layout が viewport 幅に追従する

- **WHEN** KDV が viewport resize を KRR browser session に渡す
- **THEN** KRR は browser engine の layout/paint を更新する
- **THEN** 後続 frame は新しい viewport width に対応する

### Requirement: KatanA は主文書を供給し、KRR は subresource と host capability を policy で制御しなければならない

システムは、KatanA が local file または user-entered URL の主文書を取得し、raw HTML と完全な document URL origin を KDV 経由で KRR に供給しなければならない（MUST）。KRR は document URL origin を基準に許可された stylesheet、script、image などの subresource だけを browser page に解決しなければならない（MUST）。KDV はこの値を転送するだけとし、主文書を取得してはならない（MUST NOT）。KRR は network の許可外 origin、root 外 filesystem、subprocess、remote iframe を request policy で拒否しなければならない（MUST NOT）。

#### Scenario: local script と stylesheet を読み込む

- **WHEN** KatanA が workspace 内 HTML を raw HTML と完全な file document URL origin として供給し、relative local stylesheet と script を参照する
- **THEN** KRR browser page はそれらを評価して frame を更新する

#### Scenario: workspace 外 resource を参照する

- **WHEN** HTML が symlink escape、absolute path、許可外 origin の network URL、remote iframe を参照する
- **THEN** KRR は request を拒否し、KDV/KatanA process に host capability を渡さない
