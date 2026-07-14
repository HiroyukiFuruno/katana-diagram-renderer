## Definition of Ready (DoR)

- [x] HTML/CSS の意味解釈は `KRR -> KDV -> KatanA` の順に責務を持つことを確認する。
- [x] static-only scope は initial investigation の記録であり、2026-07-11 の user requirement により release scope から除外する。
- [x] KRR `0.3.9`、KDV `0.2.8`、KatanA `v0.22.33` を公開 crate 経由で直列に更新する方針を確認する。
- [x] KRR の現在 worktree が未コミット差分を含むため、既存差分を破棄または reset しないことを確認する。

## Branch Rule

本タスクでは、以下のブランチ運用を適用します：

- **標準（Base）ブランチ**: `release/v0.3.9`
- **作業ブランチ**: `feature/v0.3.9-task-x`

KRR の公開配布を含むため、`master` へ直接 commit しない。KRR release PR は `release/v0.3.9` から `master` へ作成し、公開前に通常の品質 gate を通す。

## 1. KRR Static HTML Rendering Core

- [x] 1.1 `HtmlRenderer`、`HtmlRenderInput`、`HtmlRenderOutput` を KRR の public API として追加する。
- [x] 1.2 `html5ever` を使って HTML5 document を解析し、KDV/KatanA に HTML 構文回復を再実装させない。
- [x] 1.3 `<head>`、`<title>`、`<meta>`、`<link>`、`<style>`、`<script>` を visible output から除外する。
- [x] 1.4 `body`、tag、class、id selector と inline style を解決し、対応 property を中立 content の inline style として返す。
- [x] 1.5 table を KDV surface が消費できる table representation に正規化する。
- [x] 1.6 V8 疑似 DOM と script 実行案を採用せず、既存 V8 dependency を KRR runtime 用 `v8 = 150.0.0` へ復元する。
- [x] 1.7 KRR `0.3.9` へ version bump し、workspace internal dependency version を同期する。

### Definition of Done (DoD)

- [x] `HtmlRenderer` が UI 型を露出せず、静的 HTML/CSS の解釈を KRR 内で完結する。
- [x] KRR の HTML renderer に egui、KatanA UI、windowing dependency がない。

## 2. KRR Quality And Release Guards

### Definition of Ready (DoR)

- [x] 前タスクの core implementation と focused regression tests が完了している。
- [x] release target guard の基準となる KRR 最新公開 version を確認可能である。

- [x] 2.1 HTML5 構文回復、CSS cascade、inline override、metadata 非表示、script 非実行、table 正規化の regression test を追加する。
- [x] 2.2 `cargo test -p katana-render-runtime html -- --nocapture` を実行する。
- [x] 2.3 `cargo fmt --all -- --check` と `cargo clippy -p katana-render-runtime --lib --tests --all-features -- -D warnings` を実行する。
- [x] 2.4 dependency graph に `egui`、`eframe`、`winit`、`vello`、`katana-ui`、`katana-core` がないことを確認する。
- [x] 2.5 release target guard を patch/minor/major の一段更新だけ許可するよう更新する。
- [x] 2.6 `0.3.8 -> 0.3.9 / 0.4.0 / 1.0.0` の受理と `0.3.8 -> 0.29.0` の拒否を確認する。
- [ ] 2.7 `just check`、`just coverage`、`just VERSION=0.3.9 release-verify` を実行し、KRR package/publish dry-run を含む公開前検証を記録する。
  - Result: `just check` と `release-verify` は通過。最新の `just coverage` は total line coverage `98.75%`、uncovered lines `61` で失敗し、KRR `0.3.9` の公開は許可されない。
- [ ] 2.7a 既存 CLI、PlantUML、MathJax、runtime asset、KDR linter の uncovered lines を回帰 test または不要 code の削除で解消し、`just coverage` の `100% / 0 uncovered` gate を通過させる。
  - Progress: `249 -> 61` uncovered lines。HTML core は line coverage `100%` を維持。残件は PlantUML asset/JNI、KDR walker error、既存 renderer edge case に限られる。
- [x] 2.8 `release-check` が OpenSpec archive、`check`、`coverage`、`release-verify` を必須にすることを Justfile と regression test で固定する。
- [x] 2.9 KRR `0.3.9` が crates.io 未公開であることを確認し、package 内容に HTML parser source が含まれることを確認する。
  - Result: `assert-crates-not-published.sh 0.3.9` passed. `cargo package --list` includes all six `src/renderer/backends/html*.rs` modules.
- [x] 2.10 `./scripts/openspec validate v0-3-9-static-html-rendering-core --strict --no-interactive` を実行する。

### Definition of Done (DoD)

- [ ] KRR `0.3.9` の release check と package/publish dry-run が通過している。
- [ ] KRR の public artifact readiness と version guard の証跡が記録されている。

## 3. KRR Publication And KDV Handoff

### Definition of Ready (DoR)

- [ ] KRR の release check が完了し、`0.3.9` が未公開である。
- [ ] ユーザーが KRR の commit、push、release PR 作成を明示承認している。

- [ ] 3.1 `release/v0.3.9` を作成し、KRR change を commit、push、release PR 作成する。
- [ ] 3.2 KRR required CI と preflight が green であることを確認する。
- [ ] 3.3 ユーザー承認後に KRR release PR を merge し、GitHub Release/tag `v0.3.9` と crates.io `katana-render-runtime 0.3.9` を確認する。
- [ ] 3.4 KDV を crates.io 上の KRR `^0.3.9` へ更新し、KDV 内の CSS、visibility、table normalizer を KRR API 呼出へ置換する。
- [ ] 3.5 KDV が local path、未公開 workspace dependency、direct `v8`、egui、KatanA UI dependency を持たないことを検証する。
- [ ] 3.6 KDV `0.2.8` の release check、GitHub Release/tag、crates.io 公開を確認する。

### Definition of Done (DoD)

- [ ] KDV は公開済み KRR `0.3.9` の HTML renderer API のみを HTML/CSS interpretation に利用する。
- [ ] KDV `0.2.8` が公開済み crate として利用可能である。

## 4. User Review (Pre-Final Phase)

> ユーザーレビューで指摘された問題点。対応後に `[/]` でクローズする（通常のタスク `[x]` と区別するため）。

- [ ] 4.1 KatanA が公開済み KDV `0.2.8` を Cargo から解決した状態で HTML screenshot evidence を生成する。
- [ ] 4.2 screenshot が CSS 適用済みの visible HTML を示し、metadata/style/script text を本文に表示しないことを確認する。
- [ ] 4.3 ユーザーへ screenshot、CSS/JavaScript evaluation、navigation intent、KRR/KDV/KatanA の責務境界を提示する。
- [ ] 4.4 ユーザーから受けた feedback を本 `tasks.md` と KatanA recovery ledger に記録し、個別劣後の指定を除きすべて解決する。
- [ ] 4.5 2026-07-11 feedback: KatanA は KDV 経由で、CSS と JavaScript を評価した HTML を描画しなければならない。static-only `0.3.9` は release target にせず、`v0-4-0-html-dom-runtime` の KRR DOM runtime、KDV `0.3.0`、KatanA native-window evidence を完了する。

---

## 5. Final Verification And Release Work

- [ ] 5.1 KatanA が公開済み KDV/KRR crate を local path override なしで解決することを Cargo metadata と lockfile で確認する。
- [ ] 5.2 KatanA の HTML routing/tree/settings tests、OpenSpec strict validation、release readiness を再実行する。
- [ ] 5.3 KatanA `v0.29.0` の GitHub Release/tag が不在で、`v0.22.33` もまだ未公開であることを確認する。
- [ ] 5.4 KatanA release PR #320 の証跡を更新し、最終ユーザー OK を得るまで merge/release しない。
- [ ] 5.5 ユーザーが明示 OK した後にだけ KatanA `release/v0.22.33` PR を merge する。
- [ ] 5.6 GitHub Release/tag `v0.22.33`、公開 artifact、OpenSpec archive、branch/worktree hygiene を確認する。
