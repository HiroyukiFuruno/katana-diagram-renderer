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
- trusted default-branch publisherがPRの最新HEADを対象に検査し、`KRR / PR governance (trusted)` というcustom statusを投稿する。statusは発行Appを固定して検証し、branch protectionではこのcontextと発行Appの組み合わせを必須化する。
- Draftではcustom statusをpendingとし、ReadyのPRだけを検査して成功または失敗を投稿する。publisherはPRブランチのworkflowをcheckoutまたは実行しない。
- status発行にはGitHub Actionsの共通`app_id`を使わず、専用のKRR governance GitHub Appを使う。protected environment `pr-governance` に専用App IDとprivate keyを保持し、Appの権限はcommit statuses writeとPR/Issues readに限定する。branch protectionのrequired contextも、この専用App IDから発行されたものに固定する。
- `KRR / PR governance review latch` はreview sensorの固有GitHub Actions jobである。sensorはread-onlyの`actions`/`statuses`権限だけを持ち、secret、checkout、write権限を持たない。Draftでは即時failure、Ready PRでは同一HEAD上の`KRR / PR governance (trusted)` statusをpollし、`target_url`の`source_run_id`が自身のrun ID、かつcreator IDが`KRR_GOVERNANCE_STATUS_CREATOR_ID`と一致するterminal successだけを受理する。pending、failure、API error、設定欠落、timeout、曖昧なterminal statusはfail-closedにする。
- branch protectionでは`KRR / PR governance (trusted)`を専用App IDで、`KRR / PR governance review latch`をGitHub Actions `app_id=15368`で、それぞれ必須化する。後者だけのsuccessではmergeできず、前者だけの旧successも別HEADまたは別sensor runには使えない。
- repository Actionsのdefault `GITHUB_TOKEN` はread-onlyに保つ。sensorのread-only pollingをstatus writeへ拡張してはならない。
- trusted PR range検証は`.github/workflows/**`配下の追加、変更、rename、削除をすべて拒否する。sensor workflowがPR merge refで実行されても、改変には新HEADが必要で、そのHEADには専用App statusの成功が存在しない。trusted publisherが完全なPR range検証後に発行するstatusだけがlatchを解放できる。

このハーネスを導入するbootstrap PRでは、GitHubのbranch protection（`KRR / PR governance (trusted)` contextと専用App ID、`KRR / PR governance review latch`とGitHub Actions `app_id=15368`の組み合わせ）はbootstrap PRのmerge後に設定する。専用Appが最初の正規statusを発行したら、そのREST `creator.id`（installation bot account IDでありApp IDとは別）をrepository variable `KRR_GOVERNANCE_STATUS_CREATOR_ID`へ固定する。続けてstrict status checks、conversation resolution、必要なadmin enforcementも有効化する。専用App、secret、creator ID、または保護設定が未設定の期間は、これらをrequired checkとして強制しない。

PRのopened/edited/synchronize/reopened/Ready/Draft転換、およびreview/review-comment変更は権限・secret・checkoutを持たないsensor workflowが受ける。server生成の`workflow_run(requested)`だけをtrusted publisherが検証して再評価する。publisherはsensorのrepository、workflow名、event、workflow path、PR番号をGitHub APIで再取得して一致しなければfail-closedにする。sensor runが古くても、そのPR番号から現在のbase/head/draftを再取得して現在HEADへstatusを投稿する。sensorはイベント時のPR headで自分のnonceをpollするため、同期後のcurrent headへ古いsource run IDが投稿されても旧sensorはsuccessにならず、新しいsynchronize sensorだけが新nonceで解放できる。source sensor run IDはpending/final statusのtarget URLへ記録され、sensorはそのnonceと固定creator IDを照合する。sensorはPR番号単位で旧runをcancelし、publisherはbound workflow_runとunbound issue_commentを別concurrency groupにして、unbound commentがbound publisherをcancelできないようにする。

final review markerとreactionだけで最終証跡が揃う場合も、Ready化は必ずその後に行う。`ready_for_review` eventが新しいsensor latchを作り、最新HEADとそのsensor runに結合したtrusted statusが成功するまでmergeは許可されない。issue comment起点のunbound statusはsensor latchを解放しない。

GitHub Actionsではreview threadのresolve/unresolve変更を検知できないため、branch protectionの`required_conversation_resolution=true`を必須にする。新しいreviewまたは未解決threadが同一HEADに追加されても、trusted statusの再評価とGitHub native conversation gateの両方でmergeを拒否する。

bootstrap後は使い捨てPRで次を実機確認する。最新HEADに対する専用App statusとPR merge SHAに付くActions latchの両方がrequired checkとして評価されること、両方success後にfinal review marker commentを編集するとreaction証跡が無効化されpublisherがfailureを投稿してmergeが拒否されること、そして新しいfinal reviewとReady化で新sensor runだけが再びsuccessになることを確認する。

reactionの削除はGitHub Actionsのtrigger対象ではない。final markerの`+1`はreview bot自身だけを証跡として受理し、通常のPR actorは他者のreactionを削除できないため、第三者による削除はこのハーネスの信頼境界外である。review botの資格情報または本人がreactionを削除した場合は自動再評価されない残余制約があるため、operatorはDraftへ戻し、新しいfinal review証跡とReady化を実施して再評価する。
