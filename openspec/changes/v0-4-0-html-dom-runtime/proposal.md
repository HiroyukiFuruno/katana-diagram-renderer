## Why

v0.22.33 の HTML preview は、HTML、CSS、JavaScript、form input、timer、page navigation を KatanA 上で動作・表示する必要がある。KRR の既存 Rust/V8 runtime を interactive session として強化し、static export とは別の persistent frame/input/navigation contract を提供する。

## Architecture Correction (2026-07-18)

Chromium、Chrome for Testing、WebView、external helper process を HTML viewer に採用する案は却下された。この変更での「browser-equivalent」は外部 browser binary ではなく、KRR が所有する in-process Rust/V8 runtime の interaction contract を指す。以降の Chromium-specific 記録は履歴であり、実装、検証、release asset に使用してはならない。

## What Changes

- KRR `0.4.0` に、Rust/V8 in-process runtime を保持する持続 page session API を追加する。KatanA source host が取得した raw HTML と完全な document URL origin を KDV 経由で受け、KRR が HTML parsing、CSS layout/paint、JavaScript event semantics、input/navigation を所有する。
- KRR は browser frame、frame 更新通知、pointer、keyboard、text input、focus、scroll、resize、navigation event を公開する。静的 HTML 文字列や一回限りの画像を HTML viewer contract にしない。
- KDV は KRR の browser surface を KatanA UI に接続し、入力と lifecycle を中継するだけにする。KDV は HTML/CSS/JavaScript/layout/hit-test を実装しない。
- KatanA は source host として local HTML file または user-entered `http/https` URL の主文書を取得し、raw HTML と完全な document URL origin を KDV surface へ渡す。KatanA は KDV surface の配置、フォーカス、描画、URL input/history/tab を担い、browser engine や DOM を持たない。
- KRR/KDV の target version は、未公開の `0.3.9` / `0.2.8` を最終公開対象にせず、最新公開版から一段だけ進む release guard に従って `0.4.0` / `0.3.0` を再検証する。

## Capabilities

### New Capabilities
- `html-browser-runtime`: in-process Rust/V8 session、CSS layout/paint、JavaScript event dispatch、pixel frame を KRR 内で一貫して扱う。
- `html-browser-event-dispatch`: KDV が browser input と lifecycle event を KRR session へ渡し、frame 更新または navigation event を受け取る。

### Modified Capabilities
- `renderer-runtime-interface`: KRR の中立 renderer API に browser session、frame、input、navigation contract を追加する。

## Impact

- `katana-render-runtime` の公開 API、runtime tests、version target、package verification。
- `katana-document-viewer` は公開済み KRR `0.4.0` を使って browser surface と event dispatch を実装する。
- KDV の既存 HTML→PDF/画像 export path は維持する。interactive viewer と export conversion を同一の renderer と誤認して統合しない。
- KatanA は公開済み KDV `0.3.0` から受け取る browser surface と navigation event を統合する。navigation event の次の主文書は KatanA が取得して raw HTML と完全な document URL origin を再度渡し、HTML interpreter、egui DOM、WebView を持たない。
- KRR は external browser binary を release artifact として配布しない。local workspace 内 dependency と `http/https` URL は KRR request policy で評価し、host filesystem escape、subprocess、unsupported scheme は拒否する。
