## ADDED Requirements

### Requirement: KRR は中立な HTML renderer API を公開しなければならない

システムは、diagram renderer API と同じ公開境界から `HtmlRenderer`、`HtmlRenderInput`、`HtmlRenderOutput` を利用可能にしなければならない（MUST）。HTML renderer の利用者に KRR 内部 parser 型または UI 型を露出してはならない（MUST NOT）。

#### Scenario: KDV が HTML renderer API を呼び出す

- **WHEN** KDV が HTML source を preview 用に準備する
- **THEN** KDV は `HtmlRenderer` と `HtmlRenderInput` を使い、`HtmlRenderOutput.content` を受け取る
- **THEN** KDV は CSS parsing、visibility filtering、table normalization を独自実装しない
- **THEN** KatanA は KDV preview surface の統合だけを担う

#### Scenario: KRR dependency boundary を検証する

- **WHEN** KRR の dependency graph と public API を検証する
- **THEN** `egui`、`eframe`、`winit`、`vello`、`katana-ui`、`katana-core` を含まない
- **THEN** V8 は KRR 内の runtime dependency としてのみ存在し、KDV または KatanA UI API へ露出しない
