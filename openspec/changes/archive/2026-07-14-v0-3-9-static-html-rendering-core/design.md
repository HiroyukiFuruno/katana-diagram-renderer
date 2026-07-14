## Context

KDV の direct HTML preview は、HTML の可視要素抽出、CSS の解析、inline style 化、table 正規化を KDV 内で実施している。KatanA は KDV preview surface を受け取るだけだが、HTML/CSS の意味解釈が KRR を経由しないため、`KRR -> KDV -> KatanA` の renderer 境界になっていない。

KRR には V8 runtime が既にある。ただし V8 単体は DOM/CSS layout engine ではない。正規表現で DOM を模倣して JavaScript を動かす案は HTML/CSS renderer として不正確であり、採用しない。

## Goals / Non-Goals

**Goals:**

- KRR が HTML5 document を解析し、KDV が消費できる可視 HTML content を返す。
- KRR が `<style>` の静的 CSS を解決し、`body`、tag、class、id の単純 selector と inline style の優先順位を決定する。
- `color`、`font-weight`、`font-style`、`font-family`、`text-align`、`text-decoration`、`background`、`background-color` を KDV viewer surface 用の inline style として出力する。
- `<head>`、`<title>`、`<style>`、`<script>`、`<meta>`、`<link>` を本文出力から除外する。
- KRR/KDV/KatanA が公開済み crate のみを通じて依存する release 順序を維持する。

**Non-Goals:**

- browser と同等の CSS layout、iframe、network resource、form submission、page navigation を実装しない。
- JavaScript を実行しない。将来対応する場合は V8 に加えて標準 DOM/CSS runtime を KRR の責務として追加する。
- KRR に egui、KatanA UI state、windowing、KDV の export/viewer state を導入しない。

## Decisions

### HTML5 parser を KRR に置く

`html5ever` と `markup5ever_rcdom` を KRR の package dependency として利用する。タグの大文字小文字、暗黙要素、欠落閉じタグを HTML5 規則で回復し、KDV に HTML 文字列走査を持たせない。

`regex` または V8 上の疑似 DOM は採用しない。前者は HTML 構文を正しく扱えず、後者は JavaScript engine だけで DOM/CSS renderer を代替できないためである。

### CSS の解決結果を中立 HTML として返す

KRR の `HtmlRenderer` は source を受け、KDV が既存の neutral document surface へ変換可能な content を返す。CSS rules は source order と selector specificity で選び、inline `style` を最優先にする。KDV はその output を viewer input へ渡し、CSS parsing/cascade を再実装しない。

対応 property と selector は KDV surface が現在描画できる範囲に限定する。未対応 selector/property を対応済みと表明せず、final screenshot と surface regression test で利用者が確認できる状態にする。

### script は可視本文から除外し、実行しない

`script` は本文表示も実行も行わない。V8 を JavaScript 実行に使う将来の選択肢は KRR 内に限定するが、DOM mutation、event、link navigation を実現する標準 runtime が無い限り、対応済みとして公開しない。

### 公開 crate を直列に更新する

KRR `0.3.9` を先に release/publication し、KDV `0.2.8` が crates.io 上の KRR `^0.3.9` を依存する。KDV の publication 後に KatanA `v0.22.33` が KDV `0.2.8` を Cargo から解決する。いずれの段階でも local path、`[patch.crates-io]`、未公開 workspace dependency は使わない。

### release target は直前版から一段だけ進める

release guard は `patch +1`、`minor +1.0`、`major +1.0.0` の三候補だけを受理する。published release/tag の直前 version を基準にするため、`v0.29.0` のような飛び番を release readiness で拒否できる。

## Risks / Trade-offs

- [KDV surface が未対応の CSS を完全には表示できない] → 対応 property/selector を OpenSpec と regression test に固定し、利用者確認用の KatanA screenshot を release 前の必須証跡にする。
- [KRR と KDV に HTML 解釈が二重に残る] → KDV の CSS、visibility、table normalizer を KRR 呼出へ置換し、KDV 側に parsing/cascade module を残さない。
- [未公開 KRR を KDV が参照してしまう] → KRR publication の public verification が完了するまで KDV dependency version を変更せず、local path を使わない。
- [V8 を使った疑似 JavaScript 対応を誤って公開する] → script 非実行を KRR unit test と KDV/KatanA user-intent verification の対象にする。

## Migration Plan

1. KRR `0.3.9` に HTML renderer API、HTML5 parser、CSS regression tests、version guard を実装する。
2. KRR の release check、package、publish dry-run、GitHub Release/tag、crates.io 公開を確認する。
3. KDV `0.2.8` を公開済み KRR `^0.3.9` へ更新し、HTML parsing/cascade を KRR API 呼出へ置換する。
4. KDV を release/publication し、KatanA の Cargo.lock を公開済み KDV `0.2.8` に更新する。
5. KatanA で HTML screenshot、OpenSpec validation、release readiness を再実行し、利用者の明示 OK 後にだけ `v0.22.33` を release する。

## Open Questions

なし。JavaScript、navigation、form は本 version の非対象として明示し、将来 change で要件化する。
