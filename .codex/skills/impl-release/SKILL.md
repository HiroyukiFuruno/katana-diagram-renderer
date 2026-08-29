---
name: impl-release
description: katana-diagram-renderer で指定バージョンの実装、品質確認、release branch PR 作成、自動リリース確認までを一気通貫で進めるときに使う。/impl-release vX.Y.Z と同等のリリース実装ワークフロー。
---

# impl-release

`/impl-release vX.Y.Z` として扱う、katana-diagram-renderer のリリース実装入口です。
この repository は `release/vX.Y.Z` から `master` へ取り込み依頼（Pull Request）を作り、merge 後に自動リリースします。
初回公開版は `v0.1.0` から開始します。

governance bootstrap はPR外の絶対path `/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py` の `activate` / `finalize` / `verify` を使用する。`--expected-base --expected-head --expected-app-id --expected-diff-sha256` と完全な `--allowed-workflow` を固定し、前2者だけ `--apply`、verifyだけ `--smoke-pr` を指定する。activate/finalizeの前に別々の `KRR_GOVERNANCE_APP_JWT` と `KRR_GOVERNANCE_APP_TOKEN` を環境変数へ設定し、CLI引数・出力へ出さない。PR checkoutのコードをevidenceとして実行せず、token/private keyを引数へ渡さない。通常の `pr-ready-check` は緩和しない。

## 実行ルール

1. ユーザー指定の version を対象にする。例: `v0.1.0`
2. 作業開始前に `git status --short --branch` と `git fetch origin --prune --tags` を実行する。
3. 既存差分がある場合、release 作業へ混ぜる前に関心事を分ける。
4. 作業ブランチは `release/vX.Y.Z` に統一する。
5. 直接 `cargo publish` や tag 作成で迂回しない。公開は merge 後の自動実行基盤（GitHub Actions）に任せる。
6. 秘匿値（secret）は `CARGO_REGISTRY_TOKEN` を使う。値の取得や登録はユーザーが行う。
7. 不自然な version 飛び番は停止し、`just VERSION=vX.Y.Z release-target-check` の結果を確認する。

## Phase 1: 準備

```bash
git switch master
git pull --ff-only origin master
just VERSION=vX.Y.Z release-target-check
git switch -c release/vX.Y.Z
```

対象 version の OpenSpec change や tasks がある場合は、先に読みます。
見つからない場合は、release 内容を差分と `docs/release.md` から確認します。

## Phase 2: 実装と検証

未完了 task を実装し、必要に応じて `tasks.md` を更新します。
実装後は次を通します。

```bash
just check
just VERSION=vX.Y.Z release-check
git diff --check
```

失敗した場合は、除外や allow で逃げず、設計またはテストを直して同じ gate に戻ります。

## Phase 3: commit と push

`lefthook` を通すため、通常の commit / push を使います。

```bash
git status --short --branch
git add <release に必要な files>
git commit -m "release: vX.Y.Z リリース準備"
git push -u origin release/vX.Y.Z
```

`git push --no-verify` は使いません。

## Phase 4: Draft PR 作成と cloud review

`release/vX.Y.Z` から `master` へ Draft の Pull Request を作成します。対象 version 以前の完了済み OpenSpec change は PR 前に archive へ移動せず、merge 後に移動します。

```bash
lefthook run pre-pr
pr_url="$(gh pr create --draft --base master --head release/vX.Y.Z --title "Prepare vX.Y.Z release" --body-file <pr-body-file>)"
gh pr view "${pr_url}" --json isDraft --jq '.isDraft'
head_sha="$(git rev-parse HEAD)"
review_body="<!-- krr-review phase=initial head=${head_sha} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

`gh pr view` の結果が `true` であることを確認してから、Draft PR 上で明示的に初回 `@codex review` を依頼します。
初回 review コメントには、依頼直前の `git rev-parse HEAD` で取得した SHA を `<!-- krr-review phase=initial head=${head_sha} -->` marker として含め、その直後に `@codex review` を置きます。
レビュー（review）はローカルの自己レビューではなく cloud review を正とし、指摘は GitHub 上の review comment から取得して対応します。

## Phase 5: PR gate

Draft 上の cloud review は最低2回、かつ最新 HEAD に対して実施します。

1. 初回 review: PR 作成直後に `@codex review` を投稿する。
2. 最終 review: 指摘修正を push した後（指摘がなく修正 push がない場合も merge 前）、同じ PR の最新 HEAD に対してもう一度 `@codex review` を投稿する。

指摘修正は、指摘対象ファイルと責務を分離して subagent へ移譲します。各 review thread について、修正内容を thread へ reply し、確認後に resolve します。2回目以降で指摘が出た場合も、修正 push 後に最新 HEAD への review を追加し、各 thread の reply / resolve を完了します。
完了条件は「最低2回実施」「最新 HEAD が review 済み」「未 resolve thread が 0」です。

指摘修正を push した後（指摘がなく修正 push がない場合も merge 前）は、依頼直前の最新 HEAD を marker に固定して最終 review を依頼します。

```bash
head_sha="$(git rev-parse HEAD)"
review_body="<!-- krr-review phase=final head=${head_sha} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

最終 review コメントには `<!-- krr-review phase=final head=${head_sha} -->` marker を含め、その直後に `@codex review` を置きます。以後、別の push を行った場合は旧 HEAD の最終 review を無効として扱い、再度 marker を取得して最終 review を依頼します。

次を確認します。

- `Test and Build (macos-latest)`
- `Test and Build (ubuntu-latest)`
- `Test and Build (windows-latest)`
- `preflight`
- `just VERSION=vX.Y.Z release-target-check`
- OpenSpec の tasks / DoD
- cloud review の未 resolve thread が 0 であること

```bash
gh pr checks --watch "${pr_url}"
just VERSION=vX.Y.Z release-target-check
```

CI green だけでは Ready または merge の条件を満たしません。DoD、release-target gate、最新 HEAD review、未 resolve 0 をすべて確認します。

## Phase 6: Ready 化と merge 承認

上記の全 gate が通った後、次の専用チェックを実行します。

```bash
just pr-ready-check "<number>" && gh pr ready "${pr_url}"
```

`pr-ready-check` は参照Issueが OPEN であること、依存更新証跡が揃っていること、PR range の Issue contract が完全一致すること（不足・余分を含む）を先に検証します。成功するまで `gh pr ready` は実行しません。Ready 化後に、ユーザーへ merge 承認を求めます。承認後だけ merge します。

`pr-ready-check` の前に、GitHub の review thread を全ページ取得して未 resolve が 0 件であることを確認します。未 resolve thread が 1 件でも残っている場合は Ready 化せず、対象 subagent に修正・reply・resolve を戻します。

### governance workflow の初回 bootstrap

PR が `.github/workflows/` の governance workflow 自体を追加・変更する初回 bootstrap に限り、通常の `pr-ready-check` の代替を曖昧に設けてはいけません。PR 外の専用 GitHub App が、対象 PR の固定 HEAD SHA、Issue OPEN、依存更新証跡、PR range の Issue contract、最新 review、未 resolve thread 0、既存 CI / DoD を独立検証し、同じ固定 SHA に一時 status `KRR / PR governance bootstrap` を App ID 付きで投稿します。PR 内の workflow、branch 名、Issue、status を自己承認の根拠にしてはいけません。

この一時 status を保護ブランチの required context に追加して Ready / merge を進める場合も、通常の review と CI gate を省略しません。merge 直後に bootstrap context を除去し、専用 App の `KRR / PR governance (trusted)` と GitHub Actions App ID `15368` の `KRR / PR governance review latch` を required に切り替え、使い捨て PR で smoke 検証します。bootstrap 以外の通常 PR は必ず `just pr-ready-check "<number>"` を通します。

## Phase 7: merge と自動リリース

承認後だけ merge します。merge 後に、対象 version 以前の完了済み OpenSpec change を archive へ移動します。

```bash
gh pr merge --merge --delete-branch "${pr_url}"
# merge 後に OpenSpec change を archive へ移動
```

merge 後、Release workflow と crates.io 公開結果を確認します。

```bash
gh run list --workflow Release --limit 5
```

## 完了条件

- [ ] `release/vX.Y.Z` の PR が作成されている
- [ ] Draft PR が確認され、初回 `@codex review` が投稿されている
- [ ] 指摘ごとに subagent で修正し、thread reply / resolve が完了している
- [ ] 最新 HEAD への `@codex review` を含め、最低2回 review 済みである
- [ ] 未 resolve thread が 0 件である
- [ ] `Test and Build (...)` と `preflight` が通っている
- [ ] OpenSpec の tasks / DoD と `release-target-check` が通っている
- [ ] `just pr-ready-check "<number>"`（Issue OPEN / 依存更新証跡 / PR range Issue contract を含む）が通った後に `gh pr ready` を実行している
- [ ] Ready 化前に全 review thread を確認し、未 resolve thread が 0 件である
- [ ] Ready 化後にユーザーの merge 承認を得ている
- [ ] merge 後に OpenSpec change を archive している
- [ ] merge 後に Release workflow が起動している
