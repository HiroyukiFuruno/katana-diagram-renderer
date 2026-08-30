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

対象 version より前の完了済み OpenSpec change は、`release-check` と `pre-pr` の前に archive へ移動します。archive の変更も release の正式な commit に含め、merge 後まで先送りしません。未完了の change は完了条件を満たすまで archive せず、対象 version の release gate を通してはいけません。

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

`release/vX.Y.Z` から `master` へ Draft PR を作成します。対象 version 以前の完了済み OpenSpec change が archive 済みであることを確認してから、PR 前の gate と Draft PR 作成へ進みます。

```bash
lefthook run pre-pr
pr_url="$(gh pr create --draft --base master --head release/vX.Y.Z --title "Prepare vX.Y.Z release" --body-file <pr-body-file>)"
gh pr view "${pr_url}" --json isDraft --jq '.isDraft'
pr_number="$(gh pr view "${pr_url}" --json number --jq '.number')"
pr_json="$(gh api "repos/<owner>/<repo>/pulls/${pr_number}")"
head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
body_sha256="$(printf '%s' "$pr_json" | python3 -c 'import hashlib, json, sys
payload = json.load(sys.stdin)
body = payload.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("PR body must be a valid string without NUL or surrogate characters")
print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())')"
review_body="<!-- krr-review phase=initial head=${head_sha} body-sha256=${body_sha256} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

Draft が `true` であることを確認してから初回 review を依頼します。cloud review を正とし、全 review thread を取得して指摘を分類します。各指摘は責務単位で subagent に委譲し、修正・検証・push 後に該当 thread へ reply して resolve します。

## Phase 5: PR gate

初回指摘への対応後、または指摘が無い場合でも、merge 前に最新 HEAD へ最終 review を依頼します。

```bash
pr_json="$(gh api "repos/<owner>/<repo>/pulls/${pr_number}")"
head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
body_sha256="$(printf '%s' "$pr_json" | python3 -c 'import hashlib, json, sys
payload = json.load(sys.stdin)
body = payload.get("body")
if not isinstance(body, str) or "\x00" in body or any(0xD800 <= ord(char) <= 0xDFFF for char in body):
    raise SystemExit("PR body must be a valid string without NUL or surrogate characters")
print(hashlib.sha256(body.encode("utf-8", "strict")).hexdigest())')"
review_body="<!-- krr-review phase=final head=${head_sha} body-sha256=${body_sha256} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

別の push 後は旧HEADのreviewを無効とし、current PRのhead/bodyを再取得してbody digest付きmarkerで再レビューします。PR bodyを編集した場合は同じHEADでも旧markerと旧reviewを無効とし、initial marker→bot review→final marker→bot reviewをやり直します。最低2回のreview、最新HEADのbot review完了、全threadのreply/resolve、未resolve 0を満たすまでReadyに進みません。

次を確認します。

- `Test and Build (macos-latest)` / `ubuntu-latest` / `windows-latest`
- `preflight`
- `just VERSION=vX.Y.Z release-target-check`
- OpenSpec の tasks / DoD
- 最新 cloud review の未対応指摘 0、未 resolve thread 0

```bash
gh pr checks --watch "${pr_url}"
just VERSION=vX.Y.Z release-target-check
```

CI green だけでは Ready 条件を満たしません。指摘が出た場合は修正→通常の commit/push→reply/resolve→最新 HEAD の final review を繰り返します。

## Phase 6: Ready 化と merge 承認

全 gate とレビューを確認した後、Draft のまま専用ゲートを実行し、成功後だけ Ready 化します。

```bash
just pr-ready-check "<number>" && gh pr ready "${pr_url}"
```

`pr-ready-check` は参照Issueが OPEN であること、依存更新証跡が揃っていること、PR range の Issue contract が完全一致すること（不足・余分を含む）を先に検証します。Ready 化前と `gh pr merge` 直前の両方で、review markerのHEAD/body digestとtrusted Check Run evidenceのHEAD/external_idを同一境界に一致させる。trusted evidence の query にある `pr_body_sha256` は **ちょうど1個** の64桁小文字hexで、GitHub APIから再取得した current PR本文の strict UTF-8 SHA-256 と完全一致しなければならない。missing、duplicate、old digest、または異なるdigestは fail-closed である。Ready 化前に merge 承認を求めず、Ready 化後にユーザーの merge 承認を得ます。承認後、`gh pr merge` の直前に同じ `just pr-ready-check "<number>"` を再実行し、Ready PRの最新Issue/marker/thread/CI/base/headとこの一意なtrusted digest bindingを再検証します。承認前に merge してはいけません。

### governance workflow の初回 bootstrap

PR が `.github/workflows/` の governance workflow 自体を追加・変更する初回 bootstrap に限り、通常の `pr-ready-check` の代替を曖昧に設けてはいけません。PR 外の専用 GitHub App が、対象 PR の固定 HEAD SHA、Issue OPEN、依存更新証跡、PR range の Issue contract、最新 review、未 resolve thread 0、既存 CI / DoD を独立検証し、同じ固定 SHA に一時 Check Run `KRR / PR governance bootstrap` を App ID 付きでcompleted/successにします。PR 内の workflow、branch 名、Issue、Check Runを自己承認の根拠にしてはいけません。

この一時 Check Runを保護ブランチのApp ID固定required checkに追加してReady/mergeを進める場合も、通常のreviewとCI gateを省略しません。merge直後にbootstrap Check Run設定を除去し、専用Appの `KRR / PR governance (trusted check)` とGitHub Actions App ID `15368` の `KRR / PR governance review latch` をrequiredに切り替え、使い捨てPRでsmoke検証します。bootstrap以外の通常PRは必ず `just pr-ready-check "<number>"` を通します。

## Phase 7: merge と自動リリース

承認後は、直前の `just pr-ready-check "<number>"` が成功した場合だけ通常の merge を実行し、release 前に archive 済みの OpenSpec change と Release workflow の結果を確認します。

```bash
gh pr merge --merge --delete-branch "${pr_url}"
gh run list --workflow Release --limit 5
```

## 完了条件

- [ ] Draft PR 作成と初回 marker 付き review
- [ ] 指摘の修正、検証、thread reply / resolve
- [ ] 最新 HEAD/body digest の final marker bot review と未 resolve 0
- [ ] CI / DoD / `release-target-check` / `pr-ready-check` PASS
- [ ] `just pr-ready-check "<number>"`（Issue OPEN / 依存更新証跡 / PR range Issue contract / current `pr_body_sha256` exactly one を含む）後に Ready 化
- [ ] Ready 化後に merge 承認を得て、`gh pr merge` 直前の `just pr-ready-check "<number>"` 成功後に merge
- [ ] release-check / pre-pr の前に対象 version 以前の完了済み OpenSpec change を archive し、Release workflow を確認
