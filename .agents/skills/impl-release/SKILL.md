---
name: impl-release
description: katana-diagram-renderer で指定バージョンの実装、品質確認、release branch PR 作成、自動リリース確認までを一気通貫で進めるときに使う。/impl-release vX.Y.Z と同等のリリース実装ワークフロー。
---

# impl-release

`/impl-release vX.Y.Z` として扱う、katana-diagram-renderer のリリース実装入口です。
この repository は `release/vX.Y.Z` から `master` へ取り込み依頼（Pull Request）を作り、merge 後に自動リリースします。
初回公開版は `v0.1.0` から開始します。

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

`release/vX.Y.Z` から `master` へ Draft PR を作成します。OpenSpec change の archive は merge 後に行います。

```bash
lefthook run pre-pr
pr_url="$(gh pr create --draft --base master --head release/vX.Y.Z --title "Prepare vX.Y.Z release" --body-file <pr-body-file>)"
gh pr view "${pr_url}" --json isDraft --jq '.isDraft'
head_sha="$(git rev-parse HEAD)"
review_body="<!-- krr-review phase=initial head=${head_sha} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

Draft が `true` であることを確認してから初回 review を依頼します。cloud review を正とし、全 review thread を取得して指摘を分類します。各指摘は責務単位で subagent に委譲し、修正・検証・push 後に該当 thread へ reply して resolve します。

## Phase 5: PR gate

初回指摘への対応後、または指摘が無い場合でも、merge 前に最新 HEAD へ最終 review を依頼します。

```bash
head_sha="$(git rev-parse HEAD)"
review_body="<!-- krr-review phase=final head=${head_sha} -->"$'\n@codex review'
gh pr comment "${pr_url}" --body "${review_body}"
```

別の push 後は旧 HEAD の review を無効とし、marker を更新して再レビューします。最低2回の review、最新 HEAD の review 完了、全 thread の reply/resolve、未 resolve 0 を満たすまで Ready に進みません。

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
just PR=<number> pr-ready-check && gh pr ready "${pr_url}"
```

Ready 化前に merge 承認を求めず、Ready 化後にユーザーの merge 承認を得ます。承認前に merge してはいけません。

## Phase 7: merge と自動リリース

承認後だけ通常の merge を実行し、merge 後に archive と Release workflow の結果を確認します。

```bash
gh pr merge --merge --delete-branch "${pr_url}"
gh run list --workflow Release --limit 5
```

## 完了条件

- [ ] Draft PR 作成と初回 marker 付き review
- [ ] 指摘の修正、検証、thread reply / resolve
- [ ] 最新 HEAD の final marker review と未 resolve 0
- [ ] CI / DoD / `release-target-check` / `pr-ready-check` PASS
- [ ] `pr-ready-check` 後に Ready 化
- [ ] Ready 化後に merge 承認を得て merge
- [ ] merge 後に OpenSpec archive と Release workflow を確認
