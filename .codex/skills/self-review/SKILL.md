---
name: self-review
description: katana-diagram-renderer の差分をコミットや PR 前に自己レビューする。設計、テスト、品質ゲート、公開 API、描画ランタイムと CLI の境界を確認するときに使う。
---

# Self Review

現在の差分を対象に、コミットや PR に進める状態かを確認します。
既存の無関係な問題は巻き込まず、見つけた場合は OpenSpec や tasks.md に記録します。

## 1. 範囲確認

最初に確認します。

```bash
git status --short
git diff --stat
```

- 自分の変更と他者の変更を混ぜない。
- 未追跡ファイルを黙って含めない。
- 変更範囲が OpenSpec task と一致しているか確認する。

## 2. 設計確認

- library と CLI の責務が混ざっていない。
- 公開 API は最小で、内部実装を漏らしていない。
- 描画器（renderer）と CLI の境界が明確である。
- 外部コマンド（external command）、vendor bundle、チェックサム（checksum）、版固定（version pinning）の失敗が型で表現されている。
- 仕様化されていない fallback を追加していない。
- UI state、editor/preview、WebView、React の都合を入れていない。

## 3. Rust 品質確認

- 関数は 30 行前後に収まっている。
- ネストは深くしない。
- `unwrap`、`expect`、`panic!`、`todo!`、`unimplemented!`、`dbg!` を安易に追加していない。
- `println!` / `eprintln!` は CLI の出力責務として必要な場所にだけ置いている。
- コメントは WHY だけを日本語で残している。
- テスト都合で商用コードを曲げていない。

## 4. テスト確認

バグ修正では、修正前に失敗する再現テストがあることを確認します。

- library の unit test
- crate 境界をまたぐ integration test
- CLI の入力、終了コード、標準出力、標準エラー
- Mermaid/Draw.io/export の失敗経路
- checksum や version mismatch

固定待ちや sleep に頼ったテストを追加しません。

## 5. 品質ゲート

`/lint-and-ast-lint` を使い、必要な検査を通します。

標準の最小セット:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`just lint`、`just ast-lint`、`make lint` が追加されている場合は、そちらを優先します。

## 6. OpenSpec 確認

OpenSpec change 中なら確認します。

- 完了した task だけ `[x]` になっている。
- ユーザーフィードバックは `[/]` として追跡されている。
- 仕様変更が出た場合、artifact が更新されている。

## 7. PR レビュー接続条件

Self-review の PASS は、外部 cloud review の完了、Ready 化、または merge 準備完了の代替ではありません。Self-review は Draft PR 作成へ進む前提条件としてのみ扱います。

Draft PR 作成後は、必ず次の順序で後続工程へ接続します。cloud review の依頼コメントには、対象 HEAD を追跡できるよう `krr-review phase=initial head=<HEAD_SHA>`（初回）または `krr-review phase=final head=<HEAD_SHA>`（最終）の marker を記録します。

1. Draft PR の対象 HEAD SHA を固定し、`krr-review phase=initial head=<HEAD_SHA>` marker とともに初回 cloud review を依頼する。
2. 指摘を取得・分類し、分離可能な指摘の修正を subagent へ委譲する。
3. 各指摘の修正を確認し、review thread へ reply して resolve する。
4. 指摘がなかった場合や修正による push がなかった場合も省略せず、最新 HEAD SHA に対して `krr-review phase=final head=<HEAD_SHA>` marker 付きで最終 cloud review を依頼する。最終 review は必ず依頼時点の最新 HEAD を対象にする。
5. `pr-ready-check` で review 完了、未 resolve thread 0、CI / DoD PASS を機械確認する。

最終 cloud review で新規指摘が出た場合は、指摘を subagent へ修正委譲し、修正確認、push、review thread への reply、resolve を行ったうえで、更新後の最新 HEAD SHA に `krr-review phase=final head=<HEAD_SHA>` marker を付けて最終 review を再依頼します。新規指摘がなくなるまでこの手順を反復します。

CI green だけでレビュー完了、Ready 化、または merge 準備完了とは扱いません。

## 報告形式

```markdown
# Self Review: <対象>

## 結論
PASS / FAIL

## 確認した差分
- <ファイル>

## 検証結果
- <コマンド>: PASS / FAIL

## 指摘
- なし / 修正が必要な内容
```

FAIL のままコミットや PR に進みません。
