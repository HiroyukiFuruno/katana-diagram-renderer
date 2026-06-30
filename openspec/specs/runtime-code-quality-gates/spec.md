# runtime-code-quality-gates Specification

## Purpose
TBD - created by archiving change typescript-diagram-runtime-bundles. Update Purpose after archive.
## Requirements
### Requirement: TypeScript runtime source は Biome の厳格 gate を通らなければならない

システムは、TypeScript runtime source と既存 TypeScript scripts を Biome の formatter / linter gate に含めなければならない（MUST）。Biome の設定は、`any`、暗黙 any、non-null assertion、`@ts-ignore` 相当の抑制、barrel file、default export、未使用 code、危険な global / eval を検出できる厳格設定でなければならない（MUST）。Biome または補助検査は、`unknown` と `Record<string, unknown>` も検出しなければならない（MUST）。

#### Scenario: Biome gate を実行する

- **WHEN** 開発者または CI が TypeScript 品質 gate を実行する
- **THEN** runtime TypeScript source と `scripts/**/*.ts` が Biome の対象になる
- **THEN** generated bundle と vendor asset は formatter / linter の修正対象から除外される
- **THEN** `any`、`unknown`、`Record<string, unknown>`、暗黙 any、non-null assertion、`@ts-ignore` 相当の抑制がある場合は失敗する
- **THEN** Biome rule を弱める ignore / suppression はユーザー確認なしに追加できない

### Requirement: TypeScript compiler gate は runtime source の型安全性を検証しなければならない

システムは、TypeScript runtime source を `strict` 相当の compiler 設定で検査しなければならない（MUST）。`noImplicitAny`、`strictNullChecks`、`noUncheckedIndexedAccess`、`exactOptionalPropertyTypes` 相当の設定を弱めてはならない（MUST NOT）。

#### Scenario: TypeScript type check を実行する

- **WHEN** 開発者または CI が type check recipe を実行する
- **THEN** runtime TypeScript source は strict compiler 設定で検査される
- **THEN** nullable でない値を `?` や `| undefined` で逃がした場合は検査で検出できる
- **THEN** vendor global 境界は明示 interface で表現される

### Requirement: AST lint は合意済み階層を検査しなければならない

システムは、合意済み階層である `shared` / `mermaid` / `drawio` / `zenuml` / `generated` の境界を AST lint または同等の構造検査で守らなければならない（MUST）。

#### Scenario: 階層境界を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** runtime TypeScript source は `diagram_runtime/source/shared` / `diagram_runtime/source/mermaid` / `diagram_runtime/source/drawio` / `diagram_runtime/source/zenuml` の責務別 directory に置かれている
- **THEN** generated bundle は `diagram_runtime/generated` 配下の runtime 別 artifact として置かれている
- **THEN** `shared` は runtime 固有 entrypoint に依存しない
- **THEN** Mermaid source が Draw.io source に直接依存する、または Draw.io source が Mermaid source に直接依存する場合は失敗する

### Requirement: AST lint は Rust 側 include 先を生成済み bundle に限定しなければならない

システムは、Rust 側の `include_str!` が TypeScript source や旧手書き runtime fragment を直接読み込まないことを検査しなければならない（MUST）。V8 に渡す runtime code は生成済み bundle だけでなければならない（MUST）。

#### Scenario: Rust include 境界を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** `js_runtime_scripts.rs` 相当の Rust file は生成済み `*-runtime.min.js` を参照する
- **THEN** TypeScript source を `include_str!` で読み込む場合は失敗する
- **THEN** 旧 runtime fragment の直接 include が残る場合は失敗する

### Requirement: AST lint は生成済み bundle の手編集と同期漏れを検出しなければならない

システムは、生成済み bundle が source から再生成される artifact であることを検査し、手編集または同期漏れを検出できなければならない（MUST）。

#### Scenario: 生成物同期を検査する

- **WHEN** bundle 同期検証と AST lint を実行する
- **THEN** generated bundle に対応する source entrypoint と checksum が存在する
- **THEN** generated bundle だけが変更され、対応する TypeScript source または checksum が変更されない場合は失敗する
- **THEN** checksum 更新だけで source / bundle 差分を隠す場合は失敗する

### Requirement: 既存 TypeScript scripts の緩い型境界を棚卸ししなければならない

システムは、runtime TypeScript source を追加する前に、既存 `scripts/**/*.ts` の `unknown` / `Record<string, unknown>` / Biome suppression を棚卸しし、runtime source へ同じ型境界を持ち込まない移行方針を確定しなければならない（MUST）。

#### Scenario: 既存 scripts を棚卸しする

- **WHEN** TypeScript 品質 gate を導入する
- **THEN** 既存 scripts の `unknown` / `Record<string, unknown>` / suppression comment の一覧を出す
- **THEN** JSON parse など外部入力境界は専用 validator または明示 interface へ移す
- **THEN** runtime source では `unknown` / `Record<string, unknown>` を許可しない

### Requirement: TypeScript import 境界は package imports 前提で検査されなければならない

システムは、runtime TypeScript source の import 境界を検査し、`package.json` `imports` に定義された `#shared/*`、`#mermaid/*`、`#drawio/*`、`#zenuml/*` 形式の subpath imports を正規経路として強制しなければならない（MUST）。`source/shared` から runtime 固有領域への依存、Mermaid / Draw.io / ZenUML の相互直接依存、領域またぎ相対 import は失敗扱いにしなければならない（MUST）。

#### Scenario: `#` import 境界を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** `diagram_runtime/source/**/*.ts` の領域またぎ import は `#shared/...`、`#mermaid/...`、`#drawio/...`、`#zenuml/...` のみ許可される
- **THEN** `../shared/...` のような領域またぎ相対 import がある場合は失敗する
- **THEN** `@shared/...` のような独自 alias がある場合は失敗する
- **THEN** `#/shared/...` のような slash あり subpath imports がある場合は失敗する

#### Scenario: Runtime 間の直接依存を検査する

- **WHEN** `just ast-lint` 相当の構造検査を実行する
- **THEN** `source/shared` は `source/mermaid`、`source/drawio`、`source/zenuml` に依存しない
- **THEN** Mermaid / Draw.io / ZenUML は相互に直接 import しない

### Requirement: Bundle toolchain 設定は品質 gate で検査されなければならない

システムは、bundle toolchain が ESM graph、package `imports`、TypeScript 変換、minify / mangle を扱う構成であることを `runtime-bundle-check` で検査しなければならない（MUST）。Terser 単体で bundle を構成している場合は失敗扱いにしなければならない（MUST）。

#### Scenario: Bundle toolchain を検査する

- **WHEN** `just runtime-bundle-check` を実行する
- **THEN** Rollup または同等の bundler が ESM graph を解決していることを確認できる
- **THEN** `@rollup/plugin-node-resolve` または同等の resolver が package `imports` を解決していることを確認できる
- **THEN** Rollup output が V8 通常 script として評価できる `iife` 形式であることを確認できる
- **THEN** Terser は minify / mangle stage として使われていることを確認できる
- **THEN** Terser 単体で `#` import 解決をしている構成は失敗する

### Requirement: Generated bundle は minify / mangle 済みであることを検査されなければならない

システムは、生成済み `*-runtime.min.js` が実際に minify / mangle された artifact であることを検査できなければならない（MUST）。検査は入口 I/F を壊さず、内部実装の整形済み未圧縮 bundle が `*.min.js` として混入することを検出しなければならない（MUST）。

#### Scenario: Minified bundle を検査する

- **WHEN** `just runtime-bundle-check` を実行する
- **THEN** 生成済み `mermaid-runtime.min.js`、`drawio-runtime.min.js`、`zenuml-runtime.min.js` は再生成結果と一致する
- **THEN** minify / mangle stage を通らない生成物との差分を検出できる
- **THEN** entry I/F の `katanaRunMermaidRuntime`、`katanaRunDrawioRuntime`、`katanaRunZenumlRuntime` は保持されている
- **THEN** `katanaInstallMermaidZenumlRuntimeAdapter` は Rust 側から呼ぶ外部 entry I/F として要求されない

### Requirement: Rust/V8 entry I/F の保護を検査しなければならない

システムは、Rust 側が呼ぶ runtime entry I/F が bundle と render script の両方で一致していることを検査しなければならない（MUST）。Entry I/F を変更する場合は公開 renderer API と同等の扱いで OpenSpec 更新を要求しなければならない（MUST）。

#### Scenario: Entry I/F を検査する

- **WHEN** Rust runtime tests または AST lint を実行する
- **THEN** Rust 側 render script が呼ぶ entry 名と generated bundle が公開する entry 名が一致する
- **THEN** Terser reserved name または `globalThis["..."]` により entry 名が保護されている
- **THEN** entry 名が暗黙の関数宣言だけに依存する場合は失敗する
- **THEN** Rust 側 render script が `katanaInstallMermaidZenumlRuntimeAdapter()` を直接呼ぶ場合は失敗する
