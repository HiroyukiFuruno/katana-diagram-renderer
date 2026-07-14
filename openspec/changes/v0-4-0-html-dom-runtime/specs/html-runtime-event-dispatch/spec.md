## ADDED Requirements

### Requirement: KRR は browser input と repaint event を公開しなければならない

システムは、KRR が生成する persistent browser session に pointer、keyboard、text input、focus、scroll、resize を渡す API と、browser repaint 後の frame update を取得する API を公開しなければならない（MUST）。KDV は event を座標と入力のまま中継し、HTML node、CSS hit-test、JavaScript listener を再実装してはならない（MUST NOT）。

#### Scenario: click handler が画面を変更する

- **WHEN** KDV が browser surface 上の pointer click を KRR session に渡す
- **THEN** KRR browser engine は通常の hit-test と JavaScript event dispatch を評価する
- **THEN** KRR は handler による repaint の frame update を返す

#### Scenario: form input が JavaScript action を起動する

- **WHEN** KDV が focus と text input event を KRR session に渡す
- **THEN** KRR browser engine は form value と input/change listener を更新する
- **THEN** KDV は更新された frame を表示する

### Requirement: KRR は navigation event を KDV/KatanA に委譲しなければならない

システムは、browser page の main-document navigation を normalized navigation event として KDV 経由で KatanA に返さなければならない（MUST）。KRR は KatanA workspace を操作してはならず、KDV は URI resolution、fetch、DOM navigation を実装してはならない（MUST NOT）。KatanA は navigation event の target から次の主文書を取得し、raw HTML と完全な document URL origin を KDV 経由で KRR に供給しなければならない（MUST）。

#### Scenario: local link が別の HTML document を指す

- **WHEN** browser page の local link が user action により navigation を要求する
- **THEN** KRR は normalized target を navigation event として返す
- **THEN** KatanA は target を取得し、raw HTML と完全な document URL origin を KDV 経由で KRR session に渡す
