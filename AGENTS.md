# katana-diagram-renderer Agent Rules

## Branch Policy

- 公開配布（crates.io）、release tag、公開CLI、公開API、package metadata に影響しない変更は `master` 直接作業でよい。
- 公開配布や release に影響する変更は、作業前に branch 方針を確認する。
- ユーザーが push を明示した場合は、ローカル commit で止めず、通常の `git push` まで実行する。
- pre-push が失敗した場合は回避せず、失敗した検査を修正してから再度 push する。

## Release Inclusion Gate

- ユーザーが特定の修正を指定versionへ抱き合わせるよう指示した場合、release対象commitを `scripts/release/verify-release-target.py` の `REQUIRED_RELEASE_COMMITS` に固定する。
- `release-target-check`、PR作成、mergeの各時点で、release branchのHEADが全必須commitを含むことを `git merge-base --is-ancestor` で機械検証する。
- 別release branchのversion bump、tag、GitHub Release、crates.io公開が成功していても、必須commitを含まない場合は指定releaseの完了として扱わない。

## PR Review Gate

- Pull Request は必ず Draft として作成する。Ready PR を直接作成してはならない。
- Draft 上で初回 cloud review を依頼し、review thread を全件取得・分類する。review依頼コメントには krr-review phase=initial と対象 HEAD SHA を記録する。Cloud review は GitHub の approving review ではないため、approval として数えない。
- 分離可能な指摘修正は subagent へファイルまたは責務単位で並列委譲し、main agent はハーネス、統合判断、検証を担当する。
- 各指摘は修正、push、thread への reply、resolve まで完了させる。修正後は最新 HEAD を対象に krr-review phase=final と HEAD SHA を記録して最終 cloud review を依頼する。push 後は旧 HEAD の最終 review を無効として扱う。
- CI green だけを review 完了や merge 準備完了の根拠にしない。最新 HEAD の review 完了、未resolve thread 0、CI / DoD PASS を just PR=<number> pr-ready-check で機械確認する。
- pr-ready-check が成功した後だけ gh pr ready で Ready 化する。直接の Ready 化やUI操作でgateを迂回しても PR governance checkがmergeを拒否する。Ready 化後にユーザーへ merge 承認を求め、承認前に merge しない。

## Orchestration Gate

- main agent は司令塔として、設計、ハーネス、担当分離、統合レビュー、最終ゲートを担う。分離可能なreview指摘修正をmainが直列実装しない。
- 変更ファイルと責務を先に棚卸しし、同時実行枠の範囲で1ファイルまたは非重複責務ごとにsubagentへ並列委譲する。空き枠を放置して直列化しない。
- subagent起動前に、最新のユーザー指示と利用可能モデルを確認する。利用不可と明示されたモデルを選ばず、限定実装はLuna、複雑な設計・統合分析はTerraへ切り替える。
- main agent はsubagent結果を鵜呑みにせず、追加fixture、差分レビュー、完全ゲートで統合判定する。
