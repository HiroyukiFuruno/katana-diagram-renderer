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
