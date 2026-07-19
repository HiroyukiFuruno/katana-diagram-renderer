## Why

KatanA v0.22.33 は HTML を CSS 適用済みで表示する必要がある。しかし HTML/CSS の解析が KDV にあり、KRR の既存 renderer 境界を通らないため、描画責務と将来の JavaScript runtime の責務が分離されていない。

## What Changes

- KRR に HTML5 構文解析と静的 CSS 解決を行う公開 renderer API を追加する。
- KRR 出力は KDV が消費できる中立な HTML viewer input とし、KatanA UI 型・egui・windowing 依存を持たせない。
- `<head>`、`<style>`、`<script>` を本文から除外し、対応する静的 CSS を inline style として解決する。
- JavaScript 実行、form action、リンク遷移、ページ遷移はこの version の対象外とし、将来追加する場合も KRR の runtime 境界で扱う。
- KRR の release target check を major/minor/patch の一段更新だけ許可する規則に揃える。

## Capabilities

### New Capabilities

- `static-html-rendering-core`: HTML5 document を解析し、静的 CSS を解決した viewer 用中立出力を提供する。

### Modified Capabilities

- `renderer-runtime-interface`: KRR の中立 renderer API に HTML renderer contract を追加する。

## Impact

- `katana-render-runtime` の公開 API と package dependency に HTML5 parser を追加する。
- KDV は KRR `0.3.9` 公開後に HTML/CSS 正規化を KRR API 呼出へ置換する。
- KatanA は KDV/KRR を local path で参照せず、公開済み KDV/KRR crate の連鎖を Cargo から解決する。
