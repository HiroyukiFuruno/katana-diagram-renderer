---
name: create_pull_request
description: katana-diagram-renderer の Pull Request を、自己レビューと品質ゲート後に GitHub CLI で作る。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。PR は必ず Draft で作成し、cloud review と指摘対応を完了してから Ready にします。
推測で `master` や `main` を選びません。

## 1. 前提確認

```bash
git status --short
git branch --show-current
git branch -a
```

- commit 済みである。
- `/self-review` が完了している。
- `/lint-and-ast-lint` で必要な検証が通っている。
- 未追跡や他者差分を混ぜていない。

## 2. base branch を決める

1. ユーザーが明示した base があればそれを使う。
2. OpenSpec の task branch なら、対応する integration branch を base にする。
3. integration branch 自体なら、通常は repository default branch を base にする。
4. 判断できない場合は、候補と理由を示してユーザーに確認する。

base branch の存在を確認します。

```bash
git branch -a | rg "<base-branch>"
```

## 3. PR template を確認する

```bash
test -f .github/PULL_REQUEST_TEMPLATE.md
```

template があれば優先します。
なければ次の形で本文を作ります。

```markdown
<!-- 日本語でレビューしてください。 -->

## 概要

## 対応内容

## 影響範囲

## 動作確認
```

## 4. Draft PR を作る

Draft PR を作る前に、branch の全 commit が参照する同一 repository の Issue 集合を収集します。PR 本文の closing Issue 集合は commit 参照 Issue 集合と完全一致させ、不足も余分も許可しません。各 Issue には GitHub closing keyword（`Closes #N`、`Fixes #N`、`Resolves #N`、または同一 repository の完全な Issue URL）による closing reference を含めます。`Refs #N` だけでは不十分です。

```bash
pr_url="$(gh pr create --draft --base "<base-branch>" --head "<current-branch>" --title "<title>" --body-file "<body-file>")"
gh pr view "${pr_url}" --json isDraft --jq '.isDraft'
head_sha="$(git rev-parse HEAD)"
gh pr comment "${pr_url}" --body "<!-- krr-review phase=initial head=${head_sha} -->"$'\n@codex review'
```

`--base` は必須です。`isDraft=true` を確認してから初回 review を依頼します。

## 5. Review と指摘対応

```bash
head_sha="$(git rev-parse HEAD)"
gh pr comment "${pr_url}" --body "<!-- krr-review phase=final head=${head_sha} -->"$'\n@codex review'
```

初回・最終 review とも結果を取得し、review thread は全ページ確認します。指摘は責務単位で subagent に委譲し、修正→検証→通常の commit/push→該当 thread への reply→resolve の順で処理します。push 後は最新 HEAD の final marker review を再依頼し、未対応指摘と未resolve thread が 0 件になるまで繰り返します。

## 6. Ready 化と承認後 merge

CI、self-review、lint、test、coverage、OpenSpec/DoD、最新 HEAD review、未resolve 0 を確認した後だけ、次を実行します。各 Issue の `non-Draft target` は 256 件以下である（256 non-Draft target invariant）ことも確認します。超過した場合は bypass せず、影響する PR を Draft に戻すか closing reference を外してから merge 前に解消します。

```bash
just PR="<pr-number>" pr-ready-check && \
  gh pr ready "<pr-number>"
```

Ready 化後に merge 承認を依頼し、承認後だけ通常の merge を実行します。承認前の merge、`--admin`、`--no-verify`、Draft なし PR 作成は禁止です。

```bash
gh pr merge --merge --delete-branch "<pr-number>"
```

CI が失敗した場合は修正して同じ gate に戻ります。

## 報告

- PR URL
- base/head
- 検証結果
- CI 状態
