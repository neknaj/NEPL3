# 15. 静的サイト・例・GitHub Pages

## 1. 公開構成

標準公開先を `https://neknaj.github.io/NEPL3/` とするGitHub Pages project siteを最終成果物に含める。独立HTMLのトップ・docsと、ブラウザWasmで動くPlaygroundを配布する。言語処理serverは設けない。現在のsource artifact配布は維持し、サイト実装前に空のPages siteや架空のWasmを公開しない。

予定する出力はトップのindex.html、playground/index.html、docs/index.htmlとtutorials/reference/design、api/rust、examples/manifest.json、assets内のJS/CSS/Wasm/Worker、build.json。`site/` は配置・template・固定assetを所有し、`tools/src/site/` が生成と検査を行う。

T19はUI/runtimeのタスクへ依存せず、既存の固定版Markdown生成器を使うhost側の静的生成経路を先に完成できる。独自Markdown parserを追加しない。能力状態をdocs-only/interactiveで明示し、docs-onlyにはWasmや動くPlaygroundを要求しない。本文はJSを無効にしても読め、静的文書全pageに巨大Wasmを強制loadしない。未実装の操作入口は未提供と説明する。T19のscope付き完了はS01等のinteractive部分のpassedを意味しない。

T20でT15/T18の実Wasm/Worker/UIを統合して全Web受入とPages公開を完成させる。T21はDoc rendererを使う文書移行を担当する。初期docs-only公開と全体完成を分け、language compilerが未実装なことだけで静的文書生成を止めない。

SiteConfigをbase pathの唯一の設定元にする。既定 `/NEPL3/` と受入用 `/acceptance/project/` の両方で、全HTML、CSS、JS、Worker、Wasm、rustdoc、例manifest、画像、検索、リンクを検査する。root絶対の `/assets/` を混ぜない。baseは先頭・末尾slash付きの正規化済みpathで、query、fragment、`..`、origin混入を拒否する。

トップ/docsは静的なdirectory/index.htmlのroute、Playground内部はhash routeを使う。例は `/NEPL3/playground/#/doc?example=doc.sentence&revision=<commit>`。直接アクセス・再読み込みで復元でき、存在しないpage/例/版を別のものへ黙ってfallbackしない。未知routeは404または型付きのMissingRouteとして示す。

## 2. 正本とmanifest

正式文書はdoc/、文法とreferenceはlanguages/とschema/form表、実行例はexamples/、Rust APIは同じcommitのrustdocを入力とする。サイト専用にコピーした別仕様や別サンプルを手で保守しない。MarkdownはDoc移行完了まで正本として利用できるが、最終Doc移行は必須のT21で管理する。

page registryは安定page ID、source正本の形式とpath、公開URL、見出し/anchor ID、旧URL aliasを保持する。表示見出しやファイル移設でIDを暗黙に再生成しない。例manifestは安定例ID、language/category、source path、byte digest、必要profile、revisionを持つ。「この例を試す」は同じmanifestから取得した同じbyte列を開く。存在しない例、digest不一致、異なるruntime/profile版を拒否する。

例manifestには実行前提と期待する操作/結果も記録する。4言語それぞれの入門、完全なreference、典型例、診断の読み方、成果物取得、CLI/Web比較を用意する。rustdocだけを生成して利用者文書が完成した扱いにはしない。

build.jsonにはsource commit、設計revision、schema/package/providerの解決済みdigest、例manifestと各assetのdigest、renderer/toolchain識別を記録する。`design/profile.json` はsource manifestであり、そのままruntime Profileと呼ばない。R009では解決済みProfileの閉じた型・検査を先に定め、T05/T11で実際のpackage/providerから生成して差分検査する。架空digestや未解決aliasでdispatchしない。R006のschema閉包をUI専用の文字列signatureで迂回しない。

docs-onlyではruntimeの解決済みidentityを未存在として明示し、架空値で埋めない。interactive起動時にはHTML/JS/Worker/Wasmのidentity一致を検査する。配信更新や古いcacheで混在を検出したら、新schemaを旧Wasmへ送らず、再取得またはreloadを案内する。公開済みの全commitについてWeb版を永久保存する義務は設けない。

## 3. 文書buildと利用品質

固定toolchain・依存・asset・入力snapshotで決定的にbuildする。buildで意図しないnetwork取得、時刻埋込、例の評価を行わない。必要なdownloadは明示した取得段階でdigest固定し、生成段階は取得済みresourceだけを使う。欠けたresource、未解決参照、重複ID、unsupported content、失敗したrendererをエラーとする。

検索index、navigation、見出し階層、tableのheader、図の代替説明、keyboard操作、focus、contrast、拡大時の読み順、診断からの移動を検証する。自動アクセシビリティ検査に加え、指定browserでkeyboardと日本語IMEを手動確認する。未実行のdevice/screen reader確認を自動試験から推定しない。

## 4. 配布

T20でWasm/JS/Worker/docs/rustdoc/例manifest/build.jsonを一つのartifactにまとめる。実際の非root baseでbuild・browser・リンク検査を通し、quality成功後に**同じartifact**を配布する。検査後の別rebuildを配信しない。deploy jobだけに `pages: write` と `id-token: write`、`github-pages` environmentを与え、PR jobは公開権限を持たない。

Pagesのworkflow公開sourceを実状態で確認し、fork PRをprivileged workflowで再実行しない。配信は共通concurrency groupで直列化し、公開直前にmainとartifactのfreshnessを確認して古いrunの後勝ちを防ぐ。gateは実行したcode/schema/site checksとartifactに基づき、T16 completeの文字列をdeploy前提にしない。公開smoke→T20証拠→T16完了の順序を成立させる。

公開後はHTTPSでdeep link・Worker/Wasm・例実行・asset digestとbuild identityを照合し、対象URL、commit、artifact ID、検査結果を保存する。失敗ログも保存し、壊れた新runtimeで正常な公開物を置き換えない。docs-only更新は動作確認済みruntimeをdigest固定して利用できるが、異なる版を混ぜて同一runtime版と称さず、build.jsonで各版と互換性を示す。runtime更新には当該runtimeとbrowserの受入条件を要求する。

公開後smokeが失敗した場合はworkflowを失敗として維持し、配信成功の記録を付けない。実Pages deployなしのローカルHTTP確認は公開確認の代わりにならない。失敗を記録するだけで緑の成功statusにする実装は禁止する。

Doc移行に使う既知のrendererと同revision runtimeの試験を分ける規則は次章に従う。公開URLの準備だけでT20、T21、T16をcompleteにしない。

GitHub Pagesの静的公開とproject siteの仕様は [GitHub公式資料](https://docs.github.com/en/pages/getting-started-with-github-pages/what-is-github-pages)、workflowの権限・artifact・environmentは [custom workflows公式資料](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages) に従う。
