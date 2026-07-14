## MODIFIED Requirements

### Requirement: KRR は browser surface を含む consumer 完結 API を提供しなければならない

システムは、KatanA 側でできていた Mermaid / Draw.io / PlantUML / score の能力を落とさない KRR 公開 API を提供しなければならない（MUST）。HTML viewer は KDV と KatanA に HTML/CSS/JavaScript interpreter を再実装させず、KatanA が取得した raw HTML と完全な document URL origin を受ける browser session、frame、input、navigation の contract を KRR で完結させなければならない（MUST）。

#### Scenario: KDV が KRR browser session を consumer として使う

- **WHEN** KDV が KRR の browser session API を呼ぶ
- **THEN** KDV は raw HTML と完全な document URL origin、browser frame、input/lifecycle、navigation event の転送だけを担う
- **THEN** KDV の既存 HTML→PDF/画像 export path は browser viewer の代替として変更しない

#### Scenario: KatanA が KDV browser surface を表示する

- **WHEN** KatanA が KDV の HTML browser surface を document tab に配置する
- **THEN** KatanA は KRR engine、DOM、CSS layout、JavaScript event semantics を実装しない
- **THEN** KRR の DTO または frame contract を縮小して既存 renderer capability を失わせない
