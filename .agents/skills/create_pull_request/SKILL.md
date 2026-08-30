---
name: create_pull_request
description: katana-diagram-renderer の Pull Request を、自己レビューと品質ゲート後に GitHub CLI で作る。base branch を文脈から確認し、PR 本文に検証結果を含める。
---

# Create Pull Request

PR 作成前に、差分、検証、base branch を確認します。PR は必ず Draft で作成し、cloud review と指摘対応を完了してから Ready にします。
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
pr_url="$(gh pr create --draft --base "<base-branch>" --head "<current-branch>" --title "<title>" --body-file "<body-file>")"
gh pr view "$pr_url" --json isDraft --jq '.isDraft'
pr_number="$(gh pr view "$pr_url" --json number --jq '.number')"
pr_json="$(gh api "repos/<owner>/<repo>/pulls/$pr_number")"
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
gh pr comment "$pr_url" --body "<!-- krr-review phase=initial head=$head_sha body-sha256=$body_sha256 -->"$'\n@codex review'
```

`--base` は必須です。`isDraft=true` を確認してから初回 review を依頼します。

## 5. Review と指摘対応

```bash
pr_json="$(gh api "repos/<owner>/<repo>/pulls/$pr_number")"
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
gh pr comment "$pr_url" --body "<!-- krr-review phase=final head=$head_sha body-sha256=$body_sha256 -->"$'\n@codex review'
```

初回・最終 review とも結果を取得し、review thread は全ページ確認します。指摘は責務単位で subagent に委譲し、修正→検証→通常の commit/push→該当 thread への reply→resolve の順で処理します。markerはcurrent PRのhead/bodyを依頼直前に再取得して生成し、bodyがstringでない、NULまたはsurrogateを含む、UTF-8 strictで符号化できない場合はfail-closedで停止します。bot reviewが最新HEADに提出された場合だけ完了とします。push 後は最新HEADのbody digest付きfinal marker reviewを再依頼し、未対応指摘と未resolve threadが0件になるまで繰り返します。PR bodyを編集した場合は同じHEADでも旧markerと旧reviewを無効とし、initial marker付きbot review→final marker付きbot reviewをやり直します。

## 6. Ready 化と承認後 merge

CI、self-review、lint、test、coverage、OpenSpec/DoD、最新 HEAD review、未resolve 0 を確認した後だけ、次を実行します。各 Issue の `non-Draft target` は 256 件以下である（256 non-Draft target invariant）ことも確認します。超過した場合は bypass せず、影響する PR を Draft に戻すか closing reference を外してから merge 前に解消します。
- `pr-ready-check` は最初に、参照IssueがOPENであること、依存更新証跡が揃っていること、PR rangeのIssue契約が完全一致することを検査します。trusted Check Run evidenceの`pr_body_sha256`がcurrent PR body digestとexactly one一致し、missing、duplicate、stale digestをfail-closedで拒否することも確認します。

### Governance workflow の初回導入・改修

対象PRが governance workflow 自体の初回導入または改修である場合だけ、通常の `pr-ready-check` が workflow変更を拒否する境界を、次の一時 bootstrap 手順で解消します。これは通常PRへ適用してはならず、PR内の条件分岐・ブランチ名・Issue番号・PR author・workflow変更を根拠にした自己承認も禁止します。

1. PR外で運用する専用 GitHub App が、対象PRの固定 HEAD SHA、参照 Issue が OPEN であること、依存更新証跡、PR range の Issue 契約、最新 review・未resolve thread 0、既存 CI・DoD を独立に検証する。
2. App が対象の固定 SHA に一時 Check Run `KRR / PR governance bootstrap` を completed/successで作成または更新し、そのApp IDをrequired checkに固定する。Check RunとApp IDの対応をAPIで確認できない場合は進めない。
3. 通常の review/CI/DoD と branch protection を満たしたことを確認してから Ready 化・merge する。固定 SHA が変わったら bootstrap Check Run は無効として再検証する。
4. merge直後に一時Check Runとbootstrap用設定を除去し、専用Appの `KRR / PR governance (trusted check)` とGitHub Actionsの `KRR / PR governance review latch` をApp ID固定のrequired checkとして有効化し、実PRでsmoke検証する。

専用 App、固定 SHA、required check のいずれかを用意できない場合は merge せず、PR内の bypass で代替しません。

```bash
just pr-ready-check "<pr-number>" && \
  gh pr ready "<pr-number>"
```

Ready 化後に merge 承認を依頼し、承認後は `gh pr merge` の直前に同じ `just pr-ready-check "<pr-number>"` を再実行してReady PRの最新Issue/marker/thread/CI/base/headを再検証した場合だけ通常の merge を実行します。承認前の merge、`--admin`、`--no-verify`、Draft なし PR 作成は禁止です。

```bash
gh pr merge --merge --delete-branch "<pr-number>"
```

CI が失敗した場合は修正して同じ gate に戻ります。

## 報告

- PR URL
- base/head
- 検証結果
- CI 状態
