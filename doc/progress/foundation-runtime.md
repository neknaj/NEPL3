# Foundation runtimeの実装記録

最初の実装区切りはcore 45件、reader 27件、wire 18件のproduction API試験を独立実行し、予算停止時のreport保持、長いSourceIdの割当前検査、Contextのsource閉包・同一性衝突、typed requestのNDF往復を確認した。統括の検査コマンド・source/spec identity・実行target・結果・未検証範囲は [区切りの検証記録](../../conformance/results/foundation-slice/validation.json) に保存する。この記録は開発途中のscope付き検証で、task完了や受入群全体のpassedを示さない。

開始点は `6c9dd5f07376a3920a52ef61022d0adc011cca9f`。統括agentがcleanなworktree `C:/projects/NEPL3-runtime` とbranch `feat/foundation-runtime` を作成した。目標はNEPL3の最終仕様全体であり、最初の実装段階としてT01 core、T02 wire、T03 reader、T04 engine、T05 Grammar bootstrapを進める。

実装agentが `crates/foundation/core/` のRust実装を担当し、契約担当agentが `interfaces/`、共通仕様、workspace依存を担当する。独立レビューagentが元仕様・差分・失敗系・実行証拠を確認する。メインagentは統括・統合・検証結果の確認を行う。

## 確認した不整合と方針

- r3のSourceRefはURI/revision/digestであったが、同じURIを持つ二つの独立したmemory documentのSourceIdを区別できず、spec02のsnapshot同一性とspec13のbijectionが両立しなかった。r4ではhostが与えるopaqueなTextのSourceIdをportable identityに含め、URIをSourceContentのlocatorへ分離する。coreはIDを乱数や時計から生成しない。
- Limitsは許容最大値であり、観測したUsageとは別型にする。depthは現在の再帰深さと最大到達深さを区別し、backtrackingで消費済みworkを返却しない。
- intrinsic NDF値の閉包から、domain descriptor・登録・値検査・具体的な操作とframeへ進める。型名が解決しただけではR006を完了しない。
- 文書inventoryのDG01〜DG09を確認した。表・list・link・任意言語code等をTextへの黙った縮約やRawHtmlで代用せず、Doc schema拡張前に具体化する。共通値やsourceの実装をこの未実装domainの成功stubで埋めない。

## 現在の実行状態

Rust coreとその型・交換契約を実装中。ここに書かれた設計判断やCargo memberの追加を、T01〜T05の受入成功や全体完成とは扱わない。試験コマンド・対象・結果は実行後に追記し、未実行の受入群はnot-runを維持する。

coreのsource/value/schema/Origin/SourceMap/report/syntax/viewと、wireのNDF 12-tag codecが実装された。wireはraw intrinsic codecと、finalize済みregistryを使うchecked境界を区別する。foundation生成物は実際のregistryで検査し、通常SchemaRef recordのfield型違反を拒否する試験も実行した。

統括・独立レビューでcore 42件、wire 7件のnative試験が成功した。wireの既知byte列、非canonical値、全切断prefix、予算超過、registry照合、32,000階層の値とcleanupを含む。独立した100,000階層のdecode/encode/drop/clone/equality/debug再現も修正後に成功した。

統括がRust 1.97.0・Wasmtime 44.0.1で実行したWASIのcore 41件とwire 7件が成功した（以後追加した試験は次のfreezeで再実行する）。同じ2 crateの `--no-default-features` を付けた `wasm32-unknown-unknown`、`thumbv6m-none-eabi` のcompileも成功した。後者はcompileだけの確認であり、新しい必須hardware targetや実機実行を広告しない。

これらは進行中のtreeに対する部分的な実行記録である。reader/engine/Grammar bootstrap、typed操作adapter全体、最終CLI/Web/全domainと55受入群の達成を意味しない。freezeしたsource identityに結び付く正式証拠は統合時に改めて採取する。

続いてproductionのfoundation descriptorを明示生成し、toolsへの逆依存なしでcoreから登録できるようにした。`cargo test --locked -p nepl3-tools contract::` の13件が成功した。`cargo test --locked -p nepl3-wire --test source` の3件はWindows nativeと、統括による `--target wasm32-wasip2 -- --test-threads=1`（runner `wasmtime run`、44.0.1）の両方で成功した。source/Spanのtyped往復、同じURIの別文書、digest偽装、部分失敗後の再試行、locator衝突、UTF-8境界、nativeとwireのSourceLimit、CBOR validationが先になる失敗順を含む。いずれも上記開始点から変更中のtreeでの結果であり、確定commitの証拠ではない。

T03ではreader arena・静的検査・実行VMを実装中。Cargo memberとstatusを実態に合わせてin-progressとした。Token payload/view/triviaをSyntaxBundleへ接続するschemaを追加し、native型とwire adapterの実装を続けている。

typed wire境界はtoken/view、Origin、環境、SyntaxBundleへ拡張した。nodeのroot-first再採番、guest内の独立再採番、到達不能node拒否、256階層bundleの変換と失敗時cleanupを追加し、wireは18試験となった。統括は新しいsyntax 5件とOrigin 1件もWasmtime 44.0.1で実行し、成功を確認した。readerは実環境digestを検査するCheckedReaderContextを導入中で、別sourceを参照するOriginの閉包・無関係なsourceの除外・偽digest拒否のproduction境界試験1件が成功した。

lockfileの実解決はnum-bigint 0.4.8、num-integer 0.1.47、num-traits 0.2.19、sha2 0.11.0、unicode-ident 1.0.18である。`cargo metadata --locked --format-version 1` と取得済みcrateのCargo.toml/README/sourceでfeature構成を確認し、productionはdefault-features=falseとする。unicode-identの生成tableはUnicode 16.0.0のUCDを記録している。no_stdの実成立は上記targetでのcompile/実行結果と区別せず照合し、依存宣言だけから推定しない。

readerのtyped request converterをcore境界trait経由で実装し、`cargo test --locked -p nepl3-reader --test portable` の2件が成功した。checked NDF往復後の宣言source tableだけでcontextを作り、nativeと同じReaderSessionを実行して意味値が一致する。host側に存在しても要求で未宣言のsource、偽の環境digestを拒否する。全reply/continuationのcodec、別process provider比較、builtin reader/tokenizer、engineとGrammar bootstrapはこの段階の実装済み範囲に含めない。

統括はWASIのreader Context 1件・Runtime 14件も実行し成功を確認した。CIのWASI実行とbrowser向けcompileへnepl3-readerを追加した。readerのwire依存はこれらの実境界試験用dev-dependencyのみで、production readerはcoreだけへ依存する。

readerはReadReplyとcheckpointに生成sourceとSourceMapを保持するが、現SyntaxBundleにはSourceMapの所有tableがない。この段階ではdecode対応が構文bundle全体のportable経路を通るとは主張しない。次のengine/builtin段階で所有field・codec・Originとの不変条件を具体化し、Text escapeごとのsource対応、foreignの局所所有、再採番後の往復を検証する。R017のsource対応の残範囲およびR006の操作境界具体化として扱い、元sourceの再解析で代用しない。
