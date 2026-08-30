---
name: commit_and_push
description: katana-diagram-renderer の変更を、検証、関心分離、自己レビューを済ませてから commit と push する。ユーザーが明示した場合だけ使う。
---

# Commit and Push

このスキルは、ユーザーが明示したときだけ使います。
ファイル編集とコミットは同じ流れで連続させず、検証結果を報告して承認を待ちます。

## 1. 最初に確認する

```bash
git status --short
git diff --stat
```

- 他者の差分を混ぜない。
- 未追跡ファイルを黙って含めない。
- `.serena/`、`target/`、一時ファイルを含めない。
- ユーザーが指定した範囲だけを扱う。

## 2. 検証する

変更内容に応じて `/lint-and-ast-lint` と `/self-review` を実行します。

標準:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

`just lint`、`just ast-lint`、`make lint` が存在する場合は、自己流コマンドではなくそれを優先します。

検証が失敗した場合は commit しません。

## 3. 関心ごとに stage する

```bash
git add <file1> <file2>
git diff --cached --stat
git diff --cached
```

1 commit は 1 つの関心にします。

良い例:

```text
feat: renderer の公開 API を追加
fix: Mermaid bundle の checksum 検証を修正
docs: OpenSpec タスクを更新
```

悪い例:

```text
fix: 色々修正
feat: API と CLI とテストと文書をまとめて追加
```

## 4. commit する

コミットメッセージは日本語にします。

```bash
git commit -m "<type>: <日本語の要約>"
```

`git commit --no-verify` は、コード変更を含む場合は使いません。
ドキュメントや OpenSpec のみで使う場合も、理由を報告します。

## 5. push する

```bash
git push
```

`git push --no-verify` は使いません。
hook 自体の不具合など例外が必要な場合は、理由、直前に通した検証、対象 commit を tasks.md または PR 本文に記録してからユーザーに確認します。

## 6. PR 紐付き変更の後続フロー

PR に紐付く変更では、push 成功や CI green だけで review 完了・Ready 条件成立とは扱いません。push 後も PR は Draft のまま維持し、次の review 循環へ戻ります。

initial review は `create_pull_request` スキルの担当とし、このスキルでは push 後の review 循環へ戻ります。markerは `krr-review phase=<initial|final> head=<40 lowercase hex> body-sha256=<64 lowercase hex>` のstrict形式とし、bodyはGitHub APIから取得したcurrent PR body文字列を正規化せずUTF-8 bytesとしてSHA-256化します。

1. 最新 push 後、依頼直前にGitHub APIからcurrent PRのhead/bodyを再取得し、body digestを計算してコメント本文へ次のmarkerと `@codex review` を含めてfinal reviewを依頼します。

   ```bash
   pr_json="$(gh api "repos/<owner>/<repo>/pulls/<pr-number>")"
   head_sha="$(jq -r '.head.sha' <<<"$pr_json" | tr '[:upper:]' '[:lower:]')"
   if ! body_sha256="$(printf '%s' "$pr_json" | python3 -c '
import hashlib
import json
import sys

payload = json.load(sys.stdin)
if not isinstance(payload, dict):
    raise SystemExit("PR response must be an object")
body = payload.get("body")
if not isinstance(body, str) or "\x00" in body:
    raise SystemExit("PR body must be a string without NUL")
try:
    encoded = body.encode("utf-8", "strict")
except UnicodeEncodeError as error:
    raise SystemExit("PR body must be valid UTF-8") from error
print(hashlib.sha256(encoded).hexdigest())
'); then
     exit 1
   fi
   gh pr comment "<pr-number>" --body "<!-- krr-review phase=final head=$head_sha body-sha256=$body_sha256 -->"$'\n@codex review'
   ```

2. final reviewで新規指摘が出たら、分離可能な修正をsubagentに委譲し、修正→push→最新HEAD/bodyの再取得→body digest付きfinal marker reviewを繰り返します。PR bodyを編集した場合は同じHEADでも旧markerと旧reviewを無効化し、initial marker→bot review→final marker→bot reviewをやり直します。旧HEADまたは旧body digestのreviewは完了扱いにしません。
3. 修正した各 review thread に、対応内容と検証結果を reply し、確認できた thread だけを resolve します。
4. review thread の未 resolve 数が 0 であることを確認します。CI green だけで review 完了・Ready 条件成立とは扱いません。
5. 最新HEADのbot review完了、未resolve 0、必要なCI/品質ゲート確認を満たしたら、mainが機械ゲートを実行します。markerのHEAD/body digestとtrusted evidenceのHEAD/external_idが一致し、trusted Check Run evidenceにcurrent PR body digestを示す`pr_body_sha256`がexactly one存在することを確認します。missing、duplicate、stale digestはfail-closedで拒否します。成功後もPRはDraftのまま維持し、ユーザーの明示承認後にmainがReady化を判断します。このスキルはReady化コマンドを実行しません。

P1 などの review 指摘修正を実装する場合、分離可能ならファイルまたは責務単位で subagent に委譲し、main はオーケストレーターとして要件・DoD・差分・検証を統合確認します。同じファイルや責務を重ねて委譲しません。

Ready 判断前の機械ゲートは `just pr-ready-check "<number>"` とします。この local gate は参照Issueが OPEN であること、依存更新証跡が揃っていること、PR range の Issue contract が完全一致すること（不足・余分を含む）を先に検証します。

## 報告

- commit hash
- push 先 branch
- 実行した検証
- 含めなかった既存差分
