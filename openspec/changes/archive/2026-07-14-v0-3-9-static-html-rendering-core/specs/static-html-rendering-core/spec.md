## ADDED Requirements

### Requirement: KRR は静的 HTML document を viewer 用 content に変換しなければならない

システムは、`HtmlRenderer` を公開し、HTML source を HTML5 規則で解析して KDV が消費できる中立 content に変換しなければならない（MUST）。HTML の構文回復を KDV または KatanA に再実装させてはならない（MUST NOT）。

#### Scenario: HTML5 document を変換する

- **WHEN** consumer が `HtmlRenderer` に HTML source を渡す
- **THEN** KRR は visible document content を含む `HtmlRenderOutput` を返す
- **THEN** 大文字小文字や欠落閉じタグを HTML5 parser が解釈した結果を返す
- **THEN** KRR の public API は egui、KatanA UI state、windowing 型を公開しない

#### Scenario: metadata を本文から除外する

- **WHEN** HTML source が `<head>`、`<title>`、`<meta>`、`<link>`、`<style>`、`<script>` を含む
- **THEN** `HtmlRenderOutput.content` はそれらの要素と text を本文に含めない
- **THEN** `<body>` 内の可視 content は保持する

### Requirement: KRR は対応する静的 CSS を解決しなければならない

システムは、静的 CSS の `body`、tag、class、id selector と inline style を解決しなければならない（MUST）。対応 property は `color`、`font-weight`、`font-style`、`font-family`、`text-align`、`text-decoration`、`background`、`background-color` に限定し、結果を viewer 用 inline style として出力しなければならない（MUST）。

#### Scenario: stylesheet の style を可視要素へ適用する

- **WHEN** `<style>` が body/tag/class/id selector の対応 property を定義する
- **THEN** KRR はその style を対象要素の inline style として `HtmlRenderOutput.content` に反映する
- **THEN** `<style>` の CSS text 自体は visible content に含めない

#### Scenario: inline style が stylesheet を上書きする

- **WHEN** stylesheet と同じ property を inline `style` が指定する
- **THEN** KRR は inline `style` の値を最終値として出力する
- **THEN** id selector は class/tag selector より高い優先順位で解決される

### Requirement: KRR は script を実行してはならない

システムは、この version の HTML renderer で JavaScript、form action、link navigation、page navigation を実行してはならない（MUST NOT）。

#### Scenario: source が script を含む

- **WHEN** script が DOM mutation または event handler を定義する HTML source を KRR へ渡す
- **THEN** `HtmlRenderOutput.content` は script source を含めない
- **THEN** script による document mutation を出力へ反映しない

### Requirement: KRR の HTML renderer は公開 crate として提供されなければならない

システムは、HTML renderer を `katana-render-runtime` の公開 crate API として提供しなければならない（MUST）。KDV または KatanA が local path、未公開 workspace dependency、`[patch.crates-io]` でこれを取り込んではならない（MUST NOT）。

#### Scenario: KDV が KRR HTML renderer を採用する

- **GIVEN** `katana-render-runtime v0.3.9` が crates.io に公開済みである
- **WHEN** KDV が HTML preview dependency を更新する
- **THEN** KDV は crates.io から解決された KRR `^0.3.9` を利用する
- **THEN** KDV は HTML/CSS parsing と cascade を KRR API 呼出で代替する
