# Issue起点の変更契約

## 適用範囲

`master` 以外のbranchにある各commitは、このrepositoryのOPEN Issueをcommit messageから参照する。
短縮形は `Refs #64`、完全形は `Refs https://github.com/HiroyukiFuruno/katana-render-runtime/issues/64` とする。

`pre-push` は次の順序を固定する。

1. repository固有の完全検査 `just check`
2. `scripts/hooks/verify_push_issue.py` によるIssue契約検査

Issueが存在しない、CLOSED、別repository、またはbranch固有commitの一部にIssue参照がない場合はpushを拒否する。

## 依存更新証跡

依存manifestまたはlockfileを変更する場合は、参照Issueへ両方の対象pathと次の節を記載する。
推移依存だけを更新してlockfileだけが変わる場合は、依存解決の起点となるmanifestをIssueへ記載すればよい。
API移行が不要な場合も、省略せず理由を記載する。

```markdown
## 依存更新証跡

- 上流公開版: `package-name 1.2.3` と公開URL
- API移行: 必要な変更、または移行不要の理由
- 依存manifest: `Cargo.toml` など変更した全path
- lockfile: `Cargo.lock` など変更した全path
- 検証証跡: 実行したcommandと成功結果
```

検証器は以下を拒否する。

- 上流公開版、API移行、manifest、lockfile、検証証跡の欠落
- 変更したmanifest / lockfile pathがIssue本文にない状態
- `TODO` / `TBD` のままの証跡

## Release cleanup

公開後cleanupは [リリース手順](release.md) の安全条件に従う。
作業中のworktreeを意図的に保持する場合は `git worktree lock <path>` でlocked状態にし、自動削除を拒否させる。

## Pull Requestレビュー運用

Pull Requestは、Issue契約とレビュー結果を同じ変更履歴として追跡できる状態にしてからReadyへ進める。main agentは実装担当ではなく司令塔として、要件、担当分け、ハーネス、統合判断、最終ゲートを管理する。

### 固定フロー

1. Pull Requestを必ずDraftで作成する。
2. Draftの初回レビュー依頼に、対象HEADを記録したinitial markerと `@codex review` を含める。
3. レビュー結果とreview threadを全件取得し、P0/P1/P2などの優先度と対応要否を分類する。CI greenだけではレビュー完了と判定しない。
4. 分離可能な指摘修正は、ファイルまたは非重複責務単位でsubagentへ並列委譲する。main agentは指摘を直列実装せず、各担当の変更範囲を重複させない。
5. 各指摘を修正したら、担当範囲の検証、全体検証、push、該当threadへのreply、threadのresolveを順に行う。P0/P1は必須対応とする。
6. pushでHEADが変わるたび、旧HEADのレビューを有効な最終レビューとみなさない。最新HEADに対してfinal markerと `@codex review` を付けて再レビューを依頼し、新しい指摘がなくなるまで4〜5を繰り返す。
7. 最新HEAD、レビュー完了、未resolve thread 0、CI、Issue/DoDを次のゲートで機械確認する。

```bash
just PR=72 pr-ready-check
```

8. `pr-ready-check` 成功後にだけReady化する。Ready化後のmergeはユーザーの明示承認を得てから行う。

```bash
gh pr ready 72
```

レビュー依頼markerは、対象HEADを曖昧にしないため次の形式にする。

```text
<!-- krr-review phase=initial head=<40文字のHEAD SHA> -->
@codex review
```

最終レビューでは `phase=final` と最新HEAD SHAを使う。レビュー指摘へのreplyとresolveを省略したままReady化してはならない。

### オーケストレーションとハーネス

変更ファイルを先に棚卸しし、利用可能な並列枠を確認する。実装、テスト、調査、文書化が互いに独立している場合は、それぞれを別subagentへ委譲する。利用不可のモデルを選ばず、限定実装はLuna、複雑な設計・統合分析はTerraへ切り替える。main agentは結果を鵜呑みにせず、差分、追加fixture、完全ゲートで再検証する。

レビュー運用を守れているかは、口頭確認ではなく次の仕組みで検査する。

- ローカルの `pr-ready-check` がDraft、marker、最新HEAD、レビュー完了、thread解決、CIを検査する。
- trusted base workflowがPRの最新HEADを対象に検査し、`KRR / PR governance (trusted)` というcustom statusを投稿する。statusは発行Appを固定して検証し、branch protectionではこのcontextと発行Appの組み合わせを必須化する。
- Draftではcustom statusをpendingとし、ReadyのPRだけを検査して成功または失敗を投稿する。PRブランチのworkflowをcheckoutして実行しないため、PR側の変更でゲート自体を無効化できない。
- status発行にはGitHub Actionsの共通`app_id`を使わず、専用のKRR governance GitHub Appを使う。protected environment `pr-governance` に専用App IDとprivate keyを保持し、Appの権限はcommit statuses writeとPR/Issues readに限定する。branch protectionのrequired contextも、この専用App IDから発行されたものに固定する。
- pre-pushはIssue契約と完全検査を通過しないpushを拒否する。

このハーネスを導入するbootstrap PRでは、GitHubのbranch protection（`KRR / PR governance (trusted)` contextと専用App IDの組み合わせを必須化）はbootstrap PRのmerge後に設定する。続けてstrict status checks、conversation resolution、必要なadmin enforcementも有効化する。専用App、secret、または保護設定が未設定の期間は、このstatusをrequired checkとして強制しない。

trusted workflowはreview eventを直接購読できないため、Ready化後に新規reviewまたはthreadが発生した場合は、直ちにDraftへ戻す。その後、最新HEADに対するfinal review、指摘対応、reply/resolve、`pr-ready-check`を再実行し、成功後にだけReadyへ戻す。
