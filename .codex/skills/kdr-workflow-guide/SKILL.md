---
name: kdr-workflow-guide
description: katana-diagram-renderer の開発で、OpenSpec、品質ゲート、自己レビュー、コミット、PR 作成を迷わずつなぐための案内スキル。大きい変更、バグ修正、品質確認、次に使うスキルの判断で使う。
---

# KDR Workflow Guide

このスキルは、katana-diagram-renderer（KDR）の repo-local skill を組み合わせる入口です。
KDR は Mermaid、Draw.io、ZenUML などの図表描画ランタイムと `kdr` CLI を扱うため、runtime asset、checksum、crate 境界、CLI 公開面を弱めないことを重視します。

## 1. 仕様から始める変更

変更が大きい、責務境界が曖昧、または利用者向けの公開 API が変わる場合は、先に OpenSpec で固定します。

1. `/openspec-propose`
   - `proposal.md`、`design.md`、仕様差分（specs）、`tasks.md` を作る。
2. `/openspec-apply-change`
   - `tasks.md` の単位で実装し、完了した項目だけ `[x]` にする。
3. `/openspec-verify-change`
   - 実装が仕様、設計、タスクと一致しているか確認する。
4. `/openspec-archive-change`
   - 実装、検証、PR 統合が終わった変更だけ archive へ移す。

OpenSpec の archive は PR が実際に merge された後だけ行います。PR 作成前や
Draft / Ready の段階では archive へ移しません。リリース作業では
`/impl-release` の Draft PR、cloud review、`pr-ready-check`、merge 後 archive
という順序と整合させます。

## 2. 日常的な実装変更

小さい修正でも、検証なしに進めません。

1. 変更前に `git status --short` で既存差分を見る。
2. バグ修正なら先に再現テストを追加する。
3. 変更後に `/lint-and-ast-lint` で必要な品質ゲートを通す。
4. `/self-review` で差分を見直す。
5. ユーザーが明示した場合だけ `/commit_and_push` を使う。

### Branch Policy

- 公開配布（crates.io）、release tag、公開 CLI、公開 API、package metadata に影響しない変更は `master` 直接作業でよい。
- 公開配布や release に影響する変更は、作業前に branch 方針を確認する。
- ユーザーが push を明示した場合は、ローカル commit で止めず、通常の `git push` まで実行する。
- pre-push が失敗した場合は回避せず、失敗した検査を修正してから再度 push する。

## 3. 一括変更

複数ファイルの置換、削除、移動、生成をまとめて行う場合は、先に `/bulk-modification-protocol` を使います。

- 事前に安全な差分か確認する。
- 大きな置換は責務ごとの小さい単位に分ける。
- 変更後は `git diff` を読み、消してはいけない理由や制約を巻き込んでいないか確認する。
- ファイル編集とコミットは同じ流れで続けない。検証結果をユーザーに報告してから承認を待つ。

## 4. 品質ゲート

KDR の品質ゲートは、描画ランタイム、runtime asset、CLI、crate 公開面の安定性を守るために使います。

- `just fmt-check`
- `just lint`
- `just ast-lint`
- `just unit-test`
- `just runtime-bundle-check`
- `just biome`
- `just typecheck`
- `just runtime-asset-check`

`Justfile` に入口がある場合は、自己流コマンドではなく `just` の入口を優先します。

## 5. PR 作成

PR を作る前に `/self-review` と必要な品質ゲートを終えます。
PR 作成は `/create_pull_request` に委譲し、Ready PR を直接作成しません。次の状態遷移を厳守します。

1. Draft PR を作成し、`isDraft=true` を機械確認する。
2. Draft のまま初回 `@codex review`（cloud review）を、コメント本文に `krr-review phase=initial head=<HEAD_SHA>` marker を付けて依頼し、review / thread / コメントを取得・分類する。
3. 修正が必要な指摘は、対象ファイルまたは責務ごとに重複なく subagent へ並列委譲する。
4. 各指摘を修正・検証し、該当 thread へ対応内容を reply してから resolve する。
5. 指摘の有無や修正 push の有無にかかわらず、最新 HEAD に対する最終 cloud review を、コメント本文に `krr-review phase=final head=<HEAD_SHA>` marker を付けて必ず依頼し、結果を取得する。最終 review で新規指摘が出た場合は、対象ごとに subagent で修正・検証し、push、該当 thread への reply / resolve を行った後、更新後の最新 HEAD に `krr-review phase=final head=<HEAD_SHA>` marker を付けて再度 review する。このサイクルを未 resolve thread 0 かつ新規指摘なしになるまで反復する。
6. 最新 HEAD の review 完了、未 resolve thread 0、CI / DoD PASS を確認し、`just pr-ready-check "<pr>"` を実行する。local gate は参照Issueの OPEN、依存更新証跡、PR range の Issue contract 完全一致（不足・余分なし）を先に検証する。
7. `pr-ready-check` 成功後だけ `gh pr ready` で Ready 化し、ユーザーへ merge 承認を求める。承認後、`gh pr merge` の直前に同じ `just pr-ready-check "<pr>"` を再実行し、Ready PRの最新Issue/marker/thread/CI/base/headを再検証する。承認前に merge しない。

CI green だけでは review 完了、Ready 化、または merge の条件を満たしません。self-review、lint、テスト、coverage、OpenSpec / DoD、最新 HEAD の cloud review、未 resolve thread 0 を個別に確認します。

## 6. 持ち込まないもの

KDR には次の katana 固有スキルを持ち込みません。

- 画面 UI の手順
- 多言語翻訳
- アイコン管理
- changelog 作成
- アプリ固有のスクリーンショット運用
