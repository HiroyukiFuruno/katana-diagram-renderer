# runtime-asset-versioning Specification

## Purpose
TBD - created by archiving change v0-1-1-runtime-asset-version-pinning. Update Purpose after archive.
## Requirements
### Requirement: Mermaid.js / Draw.io.js / PlantUML JAR の取り込み version を固定しなければならない

システムは、Mermaid.js、Draw.io.js、PlantUML JAR の取り込み version を KRR repository 内で固定しなければならない（MUST）。固定 version は runtime metadata、checksum、reference snapshot の再現性に使われなければならない。TypeScript source から生成される KRR runtime bundle も checksum と生成手順を固定し、upstream vendor asset と混同せずに再現性を検証しなければならない（MUST）。

#### Scenario: Mermaid.js version を固定する

- **WHEN** KRR が Mermaid runtime を初期化する
- **THEN** 固定された Mermaid.js version の asset を読み込む
- **THEN** runtime metadata は Mermaid.js の version と checksum を返す
- **THEN** version が変わった場合は reference snapshot の更新を要求する
- **THEN** KRR 生成 `mermaid-runtime.min.js` の checksum が検証できる

#### Scenario: Draw.io.js version を固定する

- **WHEN** KRR が Draw.io runtime を初期化する
- **THEN** 固定された Draw.io.js の asset を読み込む
- **THEN** runtime metadata は Draw.io.js の version と checksum を返す
- **THEN** Draw.io.js version 更新に伴う reference snapshot が review 可能な差分として残る
- **THEN** KRR 生成 `drawio-runtime.min.js` の checksum が検証できる

#### Scenario: ZenUML runtime bundle を固定する

- **WHEN** KRR が ZenUML 対応 runtime を初期化する
- **THEN** 固定された mermaid-zenuml vendor asset を読み込める
- **THEN** KRR 生成 `zenuml-runtime.min.js` の checksum が検証できる
- **THEN** Mermaid.js / Draw.io.js の upstream version と KRR 生成 bundle の checksum を同じ metadata として扱わない

#### Scenario: PlantUML JAR version を固定する

- **WHEN** KRR が PlantUML runtime を初期化する
- **THEN** 固定 version の PlantUML JAR を OS 別の保存領域（cache）または明示 path から読み込む
- **THEN** runtime metadata は PlantUML の version と checksum を返す
- **THEN** checksum manifest は review 可能な artifact として管理される
- **THEN** crate package は PlantUML JAR 本体を含めず checksum manifest を含む
- **THEN** 保存領域（cache）に JAR が無い場合は固定 URL から download し、checksum 検証後に保存する
- **THEN** PlantUML JAR version 更新に伴う fixture / reference snapshot 差分が review 可能に残る

### Requirement: latest 確認と取り込み更新を just recipe で提供しなければならない

システムは、Mermaid.js / Draw.io.js / PlantUML JAR の latest 確認と指定 version 取り込み更新を just recipe として提供しなければならない（MUST）。

#### Scenario: latest version を確認する

- **WHEN** 開発者が latest check recipe を実行する
- **THEN** Mermaid.js、Draw.io.js、PlantUML JAR の現在固定 version と取得可能な latest version を表示する
- **THEN** repository 内の file を変更しない

#### Scenario: 指定 version を取り込む

- **WHEN** 開発者が update recipe に version を指定して実行する
- **THEN** Mermaid.js / Draw.io.js は対象 runtime asset を `vendor/<runtime>/<version>/` に取り込む
- **THEN** PlantUML は固定 JAR の download URL、checksum manifest、cache prefetch recipe を更新する
- **THEN** checksum と manifest を更新または検証する
- **THEN** full / representative の reference snapshot を再生成する
- **THEN** local full compare と CI representative compare を実行して score 低下を検知する
- **THEN** score が変わる場合は baseline 差分を review できる
- **THEN** CI の通常 compare 経路では reference snapshot を再生成しない

### Requirement: v0.1.0 transfer の挙動を壊してはならない

システムは、v0.1.1 の runtime asset version 固定によって v0.1.0 transfer の rendering / export / score 挙動を壊してはならない（MUST NOT）。

#### Scenario: v0.1.0 fixture を再検証する

- **WHEN** v0.1.1 の変更後に v0.1.0 の Mermaid / Draw.io fixtures を compare する
- **THEN** local full compare で既存 baseline と score policy を満たす
- **THEN** CI/CD representative compare で代表ケースの score policy を満たす
- **THEN** score 低下がある場合は version 更新差分として report に残す

### Requirement: Generated runtime bundle checksum は minify / mangle 後の最終 artifact を固定しなければならない

システムは、KDR 生成 runtime bundle の checksum を minify / mangle 後の最終 `*.min.js` artifact に対して固定しなければならない（MUST）。Checksum は source bundle、debug bundle、vendor asset checksum と混同してはならない（MUST NOT）。

#### Scenario: Runtime bundle checksum を検証する

- **WHEN** `just runtime-asset-check` または同等の checksum 検証を実行する
- **THEN** `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` の checksum は minify / mangle 後の成果物と一致する
- **THEN** `runtime-bundles.sha256` は最終 generated bundle の checksum を記録する
- **THEN** upstream vendor asset の checksum と KDR generated bundle の checksum は別々に report される

#### Scenario: Bundle を再生成する

- **WHEN** runtime bundle 生成 recipe を実行する
- **THEN** ESM source、`package.json` `imports`、Rollup config、Terser config から deterministic に最終 `*.min.js` が再生成される
- **THEN** Terser config は entry I/F の reserved name、comment 除去、source map 無効化、LF 改行を固定している
- **THEN** 再生成した checksum が repository 管理済み checksum と一致しない場合は検証が失敗する
