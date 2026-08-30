---
name: create-pull-request
description: katana-diagram-renderer の Pull Request を Draft で作成し、Codex review と指摘対応、品質ゲートを完了してから Ready 化する。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。PR は必ず Draft で作成し、review と指摘対応を完了してから Ready にします。
推測で `master` や `main` を選びません。

governance bootstrap はPR外の絶対path `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py` の `activate` / `finalize` / `verify` を使用する。`--expected-base --expected-head --expected-app-id --expected-diff-sha256` と完全な `--allowed-workflow` を固定し、前2者だけ `--apply`、verifyだけ `--smoke-pr` を指定する。activate/finalizeの前に別々の `KRR_GOVERNANCE_APP_JWT` と `KRR_GOVERNANCE_APP_TOKEN` を環境変数へ設定し、CLI引数・出力へ出さない。PR checkoutのコードをevidenceとして実行せず、token/private keyを引数へ渡さない。通常の `pr-ready-check` は緩和しない。

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
gh pr create --draft --base "<base-branch>" --head "<current-branch>" --title "<title>" --body-file "<body-file>"
```

`--base` は必須です。

作成直後に Draft 状態を機械確認します。`isDraft=true` でなければ、以降へ進みません。

```bash
gh pr view "<pr-number>" --json isDraft --jq '.isDraft'
```

## 5. Draft 上で初回 review と指摘対応

Draft のまま、次の順序を厳守します。

1. 依頼直前に GitHub API から current PR の `headRefOid` と `body` を再取得する。body は string 以外、NUL、lone surrogate、UTF-8 strict 不能を fail-closed で拒否し、空文字列への正規化をしない。受理したbody文字列だけを正規化せずUTF-8 bytesとしてSHA-256し、次の marker と `@codex review` を同じコメント本文にこの順で含めて初回 review を依頼する。markerのSHA値はlowercase hexでなければならない。

   ```bash
   pr_json="$(gh api "repos/<owner>/<repo>/pulls/<pr-number>")"
   head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
   body_sha256="$(python3 -c '
import hashlib, json, sys
value = json.load(sys.stdin)
body = value.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("current PR body is not valid text")
try:
    print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())
except UnicodeEncodeError:
    raise SystemExit("current PR body is not strict UTF-8")
' <<<"$pr_json")" || exit 1
   gh pr comment "<pr-number>" --body "<!-- krr-review phase=initial head=${head_sha} body-sha256=${body_sha256} -->
   @codex review"
   ```

2. review、review thread、PR コメントをすべて取得し、P0/P1/その他、対応要否、担当を分類する。
3. 修正が必要な指摘は、対象ファイルと DoD を明示して修正担当 subagent へ移譲する。同じファイル・責務を複数担当に重ねない。
4. P0/P1 に限らず対応対象と判断した通常指摘も、修正担当 subagent が **修正 → ローカル検証 → push → 該当 thread への reply → resolve** の順で完了させる。CI が green でも、未resolve の指摘があれば完了扱いにしない。
5. 修正を push したかどうかにかかわらず、最終 review は必須とする。依頼直前にGitHub APIからcurrent PRの`headRefOid`と`body`を再取得してbody digestを再計算し、次の marker と `@codex review` を同じコメント本文にこの順で含めて、最新 head に対する最終 review を依頼する。

   ```bash
   pr_json="$(gh api "repos/<owner>/<repo>/pulls/<pr-number>")"
   head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
   body_sha256="$(python3 -c '
import hashlib, json, sys
value = json.load(sys.stdin)
body = value.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("current PR body is not valid text")
try:
    print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())
except UnicodeEncodeError:
    raise SystemExit("current PR body is not strict UTF-8")
' <<<"$pr_json")" || exit 1
   gh pr comment "<pr-number>" --body "<!-- krr-review phase=final head=${head_sha} body-sha256=${body_sha256} -->
   @codex review"
   ```

6. 最終 review で新規指摘が出た場合は、指摘ごとに修正担当 subagent へ移譲し、修正・検証、push、該当 thread への reply、resolve を完了する。その後、依頼直前のcurrent PR head/bodyを再取得してbody digest付きfinal markerでreviewを再依頼し、未resolve 0 かつ新規指摘なしになるまでこの手順を反復する。PR bodyを編集した場合は同じHEADでも旧markerと旧reviewを無効とし、initial marker付きbot review → final marker付きbot reviewをやり直す。

初回・最終 review とも、コメント投稿だけでなく結果を取得して確認します。レビュー取得では review thread を省略せず、GraphQL の `pageInfo.hasNextPage` が `false` になるまで `pageInfo.endCursor` を次の `after` cursor に渡して全ページ取得します。必要に応じて次を使います。

```bash
gh pr view "<pr-number>" --json headRefOid,comments,isDraft,statusCheckRollup
gh api graphql \
  -f query='query($owner:String!, $repo:String!, $number:Int!, $reviewCursor:String, $threadCursor:String, $commentCursor:String) { repository(owner:$owner, name:$repo) { pullRequest(number:$number) { reviews(first:100, after:$reviewCursor) { nodes { id body state author { login } commit { oid } submittedAt } pageInfo { hasNextPage endCursor } } reviewThreads(first:100, after:$threadCursor) { nodes { id isResolved comments(first:50, after:$commentCursor) { nodes { id body author { login } } pageInfo { hasNextPage endCursor } } } pageInfo { hasNextPage endCursor } } } } }' \
  -f owner="<owner>" -f repo="<repo>" -F number="<pr-number>" -f reviewCursor="<review-cursor-or-null>" -f threadCursor="<thread-cursor-or-null>" -f commentCursor="<comment-cursor-or-null>"
```

レビューは `gh pr view` のboundedな一覧だけに依存せず、GraphQL `reviews` connection を `pageInfo.hasNextPage=false` になるまで取得します。各 `reviewThreads` connection も同様に全ページ取得し、各nodeのnested `comments` connectionについても `pageInfo.hasNextPage=true` なら `endCursor` を `commentCursor` に渡した追加取得を行います。nested comments の追加取得に失敗・省略した場合は fail-closed（Ready化不可）とします。各cursorの初回値は null とし、返された `endCursor` を同じconnectionの次のcursorへ設定します。canonical gateを使う場合も、全ページ取得を実施した結果で指摘・未resolve thread数を判定します。

最終 review 後、Ready 化前に次を機械確認します。

- 最終 review が最新 head (`headRefOid`) を対象に完了している。
- 未resolve thread が 0 件である。
- CI が green である。
- self-review、lint、テスト、coverage、OpenSpec/DoD がすべて PASS である。
- 各 Issue の `non-Draft target` は 256 件以下である（256 non-Draft target invariant）。超過した場合は bypass せず、影響する PR を Draft に戻すか closing reference を外してから merge 前に解消します。
- `pr-ready-check` は最初に、参照IssueがOPENであること、依存更新証跡が揃っていること、PR rangeのIssue契約が完全一致することを検査します。
- trusted Check Run evidence の `pr_body_sha256` は、current PR bodyを正規化せず strict UTF-8 SHA-256した値と一致する64桁小文字hexがちょうど1個だけ存在しなければなりません。missing、duplicate、stale、または異なるdigestはfail-closedで拒否します。

### Governance workflow の初回導入・改修

対象PRが governance workflow 自体の初回導入または改修である場合だけ、通常の `pr-ready-check` が workflow変更を拒否する境界を、次の一時 bootstrap 手順で解消します。これは通常PRへ適用してはならず、PR内の条件分岐・ブランチ名・Issue番号・PR author・workflow変更を根拠にした自己承認も禁止します。

1. PR外で運用する専用 GitHub App が、対象PRの固定 HEAD SHA、参照 Issue が OPEN であること、依存更新証跡、PR range の Issue 契約、最新 review・未resolve thread 0、既存 CI・DoD を独立に検証する。
2. App が対象の固定 SHA に一時 Check Run `KRR / PR governance bootstrap` を completed/successで作成または更新し、そのApp IDをrequired checkに固定する。Check RunとApp IDの対応をAPIで確認できない場合は進めない。
3. 通常の review/CI/DoD と branch protection を満たしたことを確認してから Ready 化・merge する。固定 SHA が変わったら bootstrap Check Run は無効として再検証する。
4. merge直後に一時Check Runとbootstrap用設定を除去し、専用Appの `KRR / PR governance (trusted check)` とGitHub Actionsの `KRR / PR governance review latch` をApp ID固定のrequired checkとして有効化し、実PRでsmoke検証する。

専用 App、固定 SHA、required check のいずれかを用意できない場合は merge せず、PR内の bypass で代替しません。

## 6. Ready 化と承認依頼

上記の review 完了・未resolve 0・CI/DoD PASS を確認した後だけ、Draft を Ready にします。

```bash
just pr-ready-check "<pr-number>" && \
  gh pr ready "<pr-number>"
gh pr view "<pr-number>" --json isDraft --jq '.isDraft'
```

確認結果が `false` になった後に限り、merge 承認を依頼します。承認後は `gh pr merge` の直前に同じ `just pr-ready-check "<pr-number>"` を再実行し、Ready PRの最新Issue/marker/thread/CI/base/headを再検証してから merge します。Ready 化前に承認を依頼したり、CI green だけを理由に review 完了と扱ったりしません。

## 7. Ready 後確認

```bash
gh pr view "<pr-number>" --web
gh pr checks "<pr-number>"
```

CI が失敗した場合は、`gh-fix-ci` 相当の調査に進みます。

## 報告

- PR URL
- base/head
- 初回・最終 review の完了結果
- 未resolve thread 数（0 件）
- self-review、lint、テスト、coverage、OpenSpec/DoD の検証結果
- CI 状態
