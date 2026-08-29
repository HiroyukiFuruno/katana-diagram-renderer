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
7. 最新HEAD、レビュー完了、未resolve thread 0、CI、Issue/DoDを次のゲートで機械確認する。ゲートはまず参照IssueがOPENであること、依存更新証跡が揃っていること、PR rangeのIssue契約が完全一致することを検査する。

```bash
just pr-ready-check 72
```

8. `pr-ready-check` 成功後にだけReady化する。Ready化後のmergeはユーザーの明示承認を得てから行う。

```bash
gh pr ready 72
```

### Governance bootstrapの限定例外

通常PRは前節の `pr-ready-check` 成功前にReady化またはmergeしてはならない。`.github/workflows/**` を追加、変更、rename、削除するgovernance bootstrap PRは、trusted PR range検証が意図どおりworkflow変更をblanket denyするため、PR内の例外や検証緩和では解けない。この場合だけ、PR外の専用GitHub Appが固定した最新HEADを独立に検証し、一時context `KRR / PR governance bootstrap` を成功として投稿する。branch protectionは、そのcontextを当該専用App IDに固定したrequired checkとして設定する。

bootstrap PRのReady化とmergeには、上記一時contextの成功に加えて、最新HEADのfinal review完了、未resolve thread 0、既存CI、DoDを全て要求する。PR内のallowlist、自己承認、`verify_push_issue.py` の緩和、PR由来workflowによるbootstrap statusの発行は禁止する。merge直後に一時contextをrequired checkから除去し、専用App IDに固定した `KRR / PR governance (trusted)` とGitHub Actions `app_id=15368` に固定した `KRR / PR governance review latch` をrequiredへ切り替える。使い捨てPRで両checkを実機smokeし、改変後のfinal review証跡が旧statusを失効させることまで確認して完了とする。

操作は次のCLIに固定する。`activate` はmerge前、`finalize` はmerge直後、`verify` はsmoke PR確認後に実行する。`--apply` はrequired checkを書き換える2操作だけに付け、App token/private keyを引数へ直書きしてはならない。PR checkoutのコードはbootstrap evidenceとして実行しない。activate/finalizeは別々の`KRR_GOVERNANCE_APP_JWT`と`KRR_GOVERNANCE_APP_TOKEN`を環境変数から受け取り、CLI引数・出力へ出してはならない。

```bash
SCRIPT=/Users/hiroyuki_furuno/.codex/skills/krr-pr-governance-bootstrap/scripts/bootstrap_pr_governance.py
bootstrap_args=(
  --repository HiroyukiFuruno/katana-render-runtime
  --pr <bootstrap-pr-number>
  --expected-base <40-character-base-sha>
  --expected-head <40-character-head-sha>
  --expected-app-id <governance-app-id>
  --allowed-workflow .github/workflows/pr-governance.yml
  --allowed-workflow .github/workflows/pr-governance-review-events.yml
  --allowed-workflow .github/workflows/release.yml
  --expected-diff-sha256 <64-character-diff-sha256>
)
export KRR_GOVERNANCE_APP_JWT="${KRR_GOVERNANCE_APP_JWT:?set the App JWT outside the command line}"
export KRR_GOVERNANCE_APP_TOKEN="${KRR_GOVERNANCE_APP_TOKEN:?set the installation token outside the command line}"
python3 "$SCRIPT" activate "${bootstrap_args[@]}" --apply
python3 "$SCRIPT" finalize "${bootstrap_args[@]}" --apply
python3 "$SCRIPT" verify "${bootstrap_args[@]}" --smoke-pr <smoke-pr-number>
```

このCLIも通常gateの代替ではない。固定HEAD、Issue OPEN、依存更新証跡、PR range契約、Draft/review/CIを検証し、workflow allowlistの完全一致に失敗したら停止する。PR内のworkflow/branch/Issueを条件にした自己例外、`verify_push_issue.py`の緩和、Actions tokenによるbootstrap status発行は禁止する。

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

bootstrap PRでは、merge前に専用Appをinstallし、一時context `KRR / PR governance bootstrap` を当該App IDに固定したrequired checkとして設定する。PR外の専用Appが固定HEADに成功statusを発行したことを確認してからだけmergeする。merge直後は一時contextを除去し、`KRR / PR governance (trusted)` を専用App IDに、`KRR / PR governance review latch` をGitHub Actions `app_id=15368` に固定したrequired checkへ即時切替する。専用Appが最初の正規statusを発行したら、そのREST `creator.id`（installation bot account IDでありApp IDとは別）をrepository variable `KRR_GOVERNANCE_STATUS_CREATOR_ID`へ固定する。strict status checks、conversation resolution、必要なadmin enforcementを維持し、使い捨てPRのsmoke完了まで公開運用を完了扱いにしない。

PRのopened/edited/synchronize/reopened/Ready/Draft転換、およびreview/review-comment変更は権限・secret・checkoutを持たないsensor workflowが受ける。server生成の`workflow_run(requested)`だけをtrusted publisherが検証して再評価する。publisherはsensorのrepository、workflow名、event、workflow path、PR番号をGitHub APIで再取得して一致しなければfail-closedにする。sensor runが古くても、そのPR番号から現在のbase/head/draftを再取得して現在HEADへstatusを投稿する。sensorはイベント時のPR headで自分のnonceをpollするため、同期後のcurrent headへ古いsource run IDが投稿されても旧sensorはsuccessにならず、新しいsynchronize sensorだけが新nonceで解放できる。source sensor run IDはpending/final statusのtarget URLへ記録され、sensorはそのnonceと固定creator IDを照合する。

CI と `release-preflight` の `workflow_run(requested/completed)` も trusted publisher がdefault branchから再評価する。sourceはAPIでworkflow名・固定path・repository・PR一件・head SHA・workflow ID・run number/attempt・状態・結論を検証し、PR上のworkflow blobがdefault branch blobと一致しない場合は拒否する。requestedは同一source generationを含むpending statusで旧successを失効させ、completedは最終API再照合後にsuccess/failureを反映する。Actionsだけでは既存checkのCAS更新はできないため、CI再実行中・失敗時はrequired CI context自体と専用Appのtrusted statusの二重境界でmergeを停止する。`review latch` はreview event世代だけの証明であり、CIごとに再生成しない。

Issueのopened/edited/deleted/transferred/closed/reopenedもtrusted default-branch workflowで受ける。resolverはeventの`issue_number`と`updated_at`を検証し、REST APIを完全ページングしてopen PRを列挙する。ページ、PR番号、state、Draft state、本文の型、重複はすべてfail-closedで検証する。`opened`/`edited`/`closed`/`reopened`は現在repositoryのIssue APIでも同じ`updated_at`を即時再確認する。一方`deleted`/`transferred`は旧repositoryで404または移動済みになることが正しいため、検証済みpayload世代を使い、旧Issue APIを読まない。

通常Issueへのcomment created/edited/deletedも同じ再評価起点である。created/editedはcurrent Issueとcomment ID・created/updated timestampをAPIで一致確認し、deletedは取得不能なcommentを信頼せずcurrent Issueの`updated_at`一致だけを用いる。いずれもclosing keywordが正確に一致する同一repositoryのopen non-Draft PRだけを全page列挙し、欠損・重複・型不正・256件超ではfail-closedにする。scheduleはnative conversation resolutionを置き換えず、イベント欠落の収束補助でありmerge根拠にはしない。

Issue eventの対象は、列挙したPRのうち非Draftで、本文にGitHub closing keyword（`close`/`closes`/`closed`、`fix`/`fixes`/`fixed`、`resolve`/`resolves`/`resolved`）と、変更Issueの`#N`または同一repositoryの完全Issue URLを組み合わせたclosing referenceを持つものだけである。通常の`verify_pr_ready.py --allow-ready`契約は、PR rangeのcommitが参照する同一repositoryのIssue集合と、PR本文のclosing referenceから得られる同一repositoryのIssue集合を完全一致させる（不足も余分も拒否）ため、Issue eventの再検証対象とcommit契約の対象が一致する。Ready targetは最大256件に制限する。256件超、ページ・型・重複不正はfail-closedである。publisherはpending statusのPOST応答から得た専用App status IDをfence基準にし、秒精度の時刻比較は使わない。fenceはIssue markerだけに限定せず、同一専用App・同一contextの自分より新しいstatus IDを全event種別で検出して旧workerのfinal POSTを停止する。対象URLのgeneration情報が重複・不正ならfail-closedにする。成功直前には現在のPR rangeとIssue contractに加え、`verify_pr_ready.py --allow-ready`とevent Issueの`updated_at`を再確認する。non-PR issue commentもcurrent Issue/commentを検証したうえで、closing referenceを持つ対象PRを再評価する。

final review markerとreactionだけで最終証跡が揃う場合も、Draftのまま`pr-ready-check`を通してからReady化する。`ready_for_review` eventが新しいsensor latchを作り、最新HEADとそのsensor runに結合したtrusted statusが成功するまでmergeは許可されない。Ready化後にmarkerを変更した場合は、後発のissue comment eventが同じPR concurrency groupで旧publisherをcancelし、statusをfail-closedにする。Draftへ戻し、新しいfinal review証跡と`pr-ready-check`を完了してから、再度Ready化しなければならない。Issue契約を復元した場合も旧final証跡を流用せず、必要なPRはDraftへ戻して新しいfinal review、`pr-ready-check`、Ready化を順にやり直す。issue commentまたはIssue revalidation起点のsourceなしstatusはsensor latchを解放しない。

GitHub Actionsではreview threadのresolve/unresolve変更を検知できないため、branch protectionの`required_conversation_resolution=true`を必須にする。新しいreviewまたは未解決threadが同一HEADに追加されても、trusted statusの再評価とGitHub native conversation gateの両方でmergeを拒否する。

bootstrap後は使い捨てPRで次を実機確認する。最新HEADに対する専用App statusとPR merge SHAに付くActions latchの両方がrequired checkとして評価されること、両方success後にfinal review marker commentを編集するとreaction証跡が無効化されpublisherがfailureを投稿してmergeが拒否されること、そして新しいfinal reviewとReady化で新sensor runだけが再びsuccessになることを確認する。

reactionの削除はGitHub Actionsのtrigger対象ではない。final markerの`+1`はreview bot自身だけを証跡として受理し、通常のPR actorは他者のreactionを削除できないため、第三者による削除はこのハーネスの信頼境界外である。review botの資格情報または本人がreactionを削除した場合は自動再評価されない残余制約があるため、operatorはDraftへ戻し、新しいfinal review証跡とReady化を実施して再評価する。
