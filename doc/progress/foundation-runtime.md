# Foundation runtimeの実装記録

## 現在状態を確認する入口

実装・受入の現在状態は[実装状態の正本](../../implementation-status.json)、成果物と依存関係は[タスク定義](../../design/tasks.json)を参照する。公開APIを使う最小の実行経路は[外部Hello consumer](../../conformance/extensions/hello/README.md)にある。今回の作業範囲と担当は[開発規約](../../AGENTS.md)に従って決める。

以下は開始commitからの実装履歴である。試験件数、担当分担、依存version、未完了範囲は各段階の記録として保存する。現在の依存versionはCargo.lock、実行可能な検査手順は[開発手順](../development.md)で確認する。

## 初期実装からの履歴

最初の実装区切りはcore 45件、reader 27件、wire 18件のproduction API試験を独立実行し、予算停止時のreport保持、長いSourceIdの割当前検査、Contextのsource閉包・同一性衝突、typed requestのNDF往復を確認した。統括の検査コマンド・source/spec identity・実行target・結果・未検証範囲は [固定revisionの検証記録](https://github.com/neknaj/NEPL3/blob/fd8057199f2f32fb36455210df36e78015b77fab/conformance/results/foundation-slice/validation.json) に記録した。これは当時の開発範囲に対する検証記録である。

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

## 次段: builtin・tokenizerと構文のsource対応

開始commitは`3bccd48d49ee1a7564335c4fa703960791ea4bc3`、作業branchは`feat/reader-builtins`。直前節までの記録は最初のsliceの状態である。過去のpathと取得方法は[履歴索引](../../conformance/history.md)に示す。

SyntaxBundleにSourceMapを接続し、別snapshotのviewが全対応経路を通じて所有tokenの範囲へ帰着することをnativeとwireで検査した。欠損・一部だけ外部の出自・guest source未宣言・空のdecode結果も検査する。readerは全terminalの正式reportが参照する生成source/mapsを保持し、対応するrequestの宣言tableと合わせて閉じる。無関係なhost storeから不足を補わない。

この進行中treeで`cargo test --locked -p nepl3-core --test maps`の2件、wire syntaxの6件、engine packageの4件が成功した。engineは局所shape、binding selectorとvisit、extension署名、provenance参照、直接arena cycleと予算を検査する実APIである。意味digest計算、解決済Profile/EntryContext、prefix実行、Grammar bootstrapはまだ含まない。`PackageIdentity`等の型宣言を検査済みidentityとして利用する公開proofも提供していない。

`python tools/audit/allocation/run.py`はWindows nativeでpositive control 4096 bytesと長いSourceId・ゼロ予算のSourceMapで0 bytesを確認した。独立レビューは同じ測定器で旧sliceが100000 bytesを割り当てて失敗することも確認した。CIのnative必須検査へ追加し、WASI実行/browser向けcompileにはengineを含めた。現在treeの確定identityと全体試験結果は統合時に別途採取する。受入55群の状態はこの部分試験から変更しない。

この区切りの統合ではWindows native 173件（core 50・reader 44・wire 19・engine 4・tools 56）を実行した。builtinはName・Nat・Number・Lang・Text・Triviaを提供し、ordered tokenizerは非同期providerとText生成sourceの予約・再開・取消を扱う。reader停止後の正式診断と生成sourceの保持、予算のリセットを伴う再開の拒否、停止後のsession再利用をproduction APIで検証した。

統合試験では、複数の制約IDを持つSyntaxBundleを追加したことでhost生成器とproduction canonical writerの順序不一致を検出した。制約は名前の集合なのでhost側も正準sortし、重複・不正IDの拒否と順序交換の回帰試験を追加した。field・choice・binding子列の意味順序は維持する。独立レビュー担当も元の失敗と修正後の3試験を実行して確認した。

コマンド、target、実行ログとhash、対象source/spec、未検証範囲は [builtin/tokenizerの統合記録](https://github.com/neknaj/NEPL3/blob/fd8057199f2f32fb36455210df36e78015b77fab/conformance/results/reader-builtins/validation.json) に記録した。直前のmainへ統合された最初のsliceの3 OS・WASI CIとsource artifactの確認は [過去commitのCI記録](https://github.com/neknaj/NEPL3/blob/fd8057199f2f32fb36455210df36e78015b77fab/conformance/results/foundation-slice/main-ci.json) にある。当時の後続課題はprefix engine、Profile解決、Grammar compilerとbootstrapである。現在の到達点は状態索引を参照する。

## 次段: prefix engineとGrammar compilerの途中checkpoint

`feat/prefix-engine`の開始点は`4550e63f`。この節は変更中のtreeの記録であり、確定commitの統合試験identityは統括で別途保存する。packageの意味digestと具象arena・provenanceを固定するexecution digest、aliasを保持する解析Profile、静的prefix実行と回復tree、owned tokenizer待機を実装した。統括は現在のparser 13件をnativeとWASIで実行して成功を確認した。source上の綴りとtriviaを構造順に返すprinterは、source-less意味printerやwhole-file出力とは別である。

Grammarの全constructorを持つ型付きAST、source/参照/Nodes予算の検査、全ReaderExprのlowering、host用の初回seed JSON importerを追加した。初回adapterは実sourceのUTF-8 byte spanを保持する。`cargo test --locked -p nepl3-grammar-core`の2件、`cargo test --locked -p nepl3-tools bootstrap`の2件、Python seed入力の3件が成功した。全constructorの実行網羅やbootstrapの成功を、この件数から推定しない。

package assemblerはsymbolicなreader出力型から実surface descriptorを生成・登録し、reader/mode/form/leaf/namespace/binding/style/extensionを組み立て、productionのLanguagePackage検査へ接続した。`cargo test --locked -p nepl3-tools --test grammar`の1件で、旧Angle例の異なるchoice出力型とbinding例の非Textな名前leafを実際に拒否した。Angleの各枝を明示discardにした正式例はpackage検査に成功した。旧Angleは理由を持つnegative fixtureとして保存し、seqを暗黙文字列連結へ変更していない。

このcheckpointでは`cargo clippy --locked -p nepl3-tools --all-targets -- -D warnings`、生成物の一致検査、repository checkが成功した。assemblerの一般的な全失敗系・位置付き診断、facts/v1の共通値と実descriptor、binding例の訂正、Angleの実ReaderSession境界試験、ParsedTreeからのlowerの実往復、完全seedからP1/P2を生成するbootstrapは未検証または未実装として残る。R006/R009等やT05の完成を宣言せず、次のcommitで継続する。

## 次段: 共通factsと実Grammar bootstrap

`feat/grammar-bootstrap`ではcoreにFactSet/FactDeltaとhostが明示する変更権限を追加し、型付きwire交換でもsource・Origin・scope・namespace・予約IDの検査を共有する。native facts 2件、wire facts 1件が成功し、追加source/Originを持つdeltaの往復とsource宣言欠落の拒否も含む。これは構造・権限・参照閉包の検査であり、名前解決の正しさ、delta適用、Custom binding callbackとFactsRequest/Reply全体のcodec成立を意味しない。

標準catalogは実reader/facts descriptorを登録する。正式Binding例はname-v1からTextを取得するよう訂正し、旧例をnegative fixtureへ分けた。packageのpayloadSchemasは実際に参照する型と操作の閉包を保持する。toolsのgrammar 4件で、完全Grammarのpackage組立、正式例、共有kind、実schema依存を検査した。

SourceMapのsnapshot DAGによる十分条件と従来の厳密なpointwise fallbackを併用し、reader成果物のmap検査をbatch化した。map 3件と既存の精密cycle等4件が成功した。Textの連続Exact対応をまとめる変更と合わせ、実engineで正式Grammar sourceを解析・lower・compileするP0/P1/P2の意味identity一致が初めて成功した。最初のdebug観測は約105秒、累積Work約91億、Allocation約598億、深さ96であり、コピー費用の最適化と全失敗系・全reader constructorのcompiler経由検査は残る。facts callbackは実行していない。確定treeのコマンド・測定値・mutationと停止試験・cross-target結果は統括の今回の検証記録へ分離し、過去commitのCI成功を現在treeへ流用しない。

## 次段: Custom binding と keyed query

前節までの記録は過去sliceの固定範囲である。現在は通常のCustom bindingを明示hostへdispatchし、同じvalidatorを使う借用FactsRequest、正式Report emitter、権限付きdeltaの原子的適用、疎ID、解決更新履歴を実装した。元Occurrenceの発行stageを保存し、before/afterの連鎖と最終FactSetをnative/CBORで確認する。recursive Custom header/bodyのphase別契約、独立processの実Facts呼出しと遠隔予算精算は未完として残す。

AnalysisKeyに束縛した実定義ジャンプ・参照検索を追加した。元の7種類の入力に対するbyte位置、shadowing、Foreign、曖昧な全候補、escaped Text、Unicode境界と半開終端を管理対象にし、source-lessなCustom Entity、疎ID、過去Occurrenceの明示更新後のqueryも実行する。通常の名前探索をquery側でやり直さない。QueryRequest初回受信と返信source閉包のCBOR往復、改変URI/ID/key/source表の拒否、停止理由の保持を検査する。一般field/view region selector、rename、全editor機能やT06全体の完成はこの部分証拠から推定しない。

## 次段: Recursive Custom と単体Foreign閉包

Recursive CustomのHeader/Body phaseと受理header IDのprivate memoを実装した。実compiler→parser→bindingにより相互再帰、静的宣言との混在、同plan反復、同名別Entity、Foreign独立root、Unicodeを確認し、同期NDF/CBOR初回受信から同じcallbackを呼ぶ結果も比較した。phase/receipt変異と7資源の停止prefixは通常のBindingReply codecで検査する。外部Facts providerへの排他的quota貸出・精算は後続に残す。

ForeignClosureはguest syntaxと選択owner環境・元Origin/source/map表を保持する。owner/guestで同数値の環境IDとOriginRefが別内容を指す実CBOR往復、宣言source欠落、環境digest偽造、外側schema/root不一致と停止を管理対象へ追加した。非empty EnvironmentBindingで実証されたNamespaceRef descriptorの混同をEnvironmentNamespaceRefへ訂正した（R046）。これは共通閉包の検証であり、Doc/Mathのlower・意味処理全体の完了を示さない。

## 次段: 実再parseに束縛したrename

名前Occurrenceを選択する二段renameを追加した。元と候補の両方で実ParseSession完了proofと同じimmutable treeに対するkeyed Binding実行を要求し、private draftの候補source・同Profile/実装/options・構文形状・全確定resolutionをacceptで照合する。raw treeを変更してprepareしただけの値、別parse proofとの組替え、stale revision、未使用のmap target末尾bytesやURIの変更では編集を公開しない。候補を作る段階で元SourceStoreへ編集を適用しない。

管理対象はLetのinitializer除外、shadowing/free nameの捕捉拒否、同scope衝突、予約headの所有schema、Foreign独立Entity、Unicode/CRLF、Custom疎IDと更新後の最終resolution、位置なし、変換型/複数source/被覆穴の拒否を含む。隣接Exactを一意な連続被覆へ結合し、全区間・scalar分割・逆表順とUnicode長さ変更を実再parse→解析→accept→CBOR往復で比較する。完了proofのread/resume/reserve/resume_head入口は通常のraw結果・Report・Usageと比較し、非Complete枝からproofを発行しない。受信検査ではold digest・書込元・key・非重複・宣言source欠損をschema-valid変異で検査し、停止理由を保持する。

このrename APIのReportはrename自体の観測Usageだけで、診断/eventは元parse/analyzeの正式Reportをcallerが保有する。全段階のReportを集約する汎用外部Rename operation包絡が完成したとの主張ではない。初回raw decoderは書込許可・parse実行・意味対応のproofを発行せず、独立processの認証済み実行/費用統合は後続に残る。任意の逆encoder、一般field/view region selector、増分再解析、T06全体の完成をこの範囲から推定しない。最終cross-targetと統括gateの結果は当該checkpoint記録を正本とする。

## 次段: field/View/reader sidecarのregion選択

Grammarの既存style arityを保ち、独立したselection宣言からU64 priorityを生成する。Form/Leaf/HeadShapeの型・package identity・source位置付き失敗を同期した。実元GrammarのP0/P1/P2と独立source変異を従来の10B Work上限内で検査する。

prepared tree/Profileと明示ReaderFactBatchをRegionKeyへ束縛し、最小range・priority・包含深さ・宣言順で選択する実APIを追加した。field、token内部View、Capture/Presentation、Foreign、回復nodeを区別し、UnknownHeadのarityを捏造しない。表示用SourceMapはowner局所で、Exactの分割同値、部分被覆、Transformed、多義性を保持する。token.leadingTriviaの再掲とCaptureのmapped containmentは既存reader契約で検査する。

管理対象は実parserからのfield priority、nested View、生成source Capture、消費範囲外の正式Presentation、改変trivia/owner/revision、Foreign/escaped Text/Unicodeの独立CBOR初回受信、要求/返信変異、回復・UTF-8境界・stale key・停止を含む。最終native/WASI/統括gateの実行結果はcheckpoint記録に分離する。この最初のregion単位の出力は元overlapとclassを保持したhighlight材料であり、一般regionから既存definition/referencesへの接続は次節に分けた。LSP向け非重複・単行化、completion、増分解析は後続である。raw sidecarやRegionReplyからprovider実行や完了Bindingのproofを発行しない。

## 次段: 選択regionと確定Bindingのquery接続

RegionQueryはpriorityで選択したlogical regionと正準bundle所有を保持し、実Bindingが発行したbundle/root ScopeId対応から候補Occurrenceを絞る。Foreign fieldの内容はguest rootへ対応し、同source上のhost宣言を誤参照しない。owner局所のExact/Transformed双方向関係で位置を対応付け、非一意な対応の候補を保持する。各Definition/Referencesは既存の確定resolution処理を共有し、Custom更新後の疎IDやsource-less targetも維持する。

Customの受理mapはemitter/deltaの受理時点でbundle所有indexを同時に保存する。実host/guestが同じ生成sourceを共有する入力で各定義への到達とguest mapのhostへの非漏出を検査し、初回CBOR要求から実providerを再実行したBinding結果でも一致する。途中停止でmapと所有の片方だけを公開しないこと、raw indexの範囲外・重複拒否も管理対象に含む。

既存renameが平坦なmap表を探索し、guest-only mapによってhost名の編集先をaux sourceへ選べた問題も同じ所有契約で修正した。各Occurrence/Entityの逆写像、各元編集とownerの組に対する前向き導出、old/new側の位置比較を局所化する。実sealed `lambda x guest x` のguest reader返信にだけaux→host Nameを宣言した反例はaux-only writableで拒否し、host writableの正常編集と、Customが返した生成名からのemitter/delta両経路の正常renameを実再parse・再Binding・CBORで検査した。

原Letのinitializer/body、nested Lambda、Foreign field priority、日本語位置、実providerが宣言した複数Exact/Transformed候補、停止・stale key・source/候補改変、bundle/root対応の不正受信を管理対象へ追加した。元Rust要求を受け取らないCBOR初回受信から実Bindingを行い、nativeの選択と候補・sourceを比較する。raw metadataや返信codecは意味proofを発行しない。LSP用highlight正規化、completion、増分解析、外部process認証・費用統合は別項目に残り、この接続からT06全体の完成を推定しない。


## 2026-10-08: 保存済みReader・Tokenizerの境界

checkpoint `7a5ba3d287081b83e4d9356969a689b8b7cdc3a8` では、Reader・Tokenizerのcontinuation全体と、Reader Await／Tokenizer Await・Reserve全体のcanonical echoを、同じsessionのprivateな保存状態へ照合する経路を検査した。Tokenizerから借用するterminal reply contextも、外側へコピーしたcontinuationから作らず、実際の内側Readerが保持する要求・source閉包・Usage・depthを参照する。

[provider側の統合試験](../../apps/provider/tests/tokenizer.rs)は、実builtinNameのnative子実行からOperationReplyのCBOR往復、保存済みcontextでの検証、両echo方式でのTokenizer再開を接続する。実WorkLimit、改変拒否後の同じ実結果による再試行、trivia保持、最終Reportの親Usageとの一致、新operationへの再入場を含む。validationの実測費用を成功後に一度計上する経路と、親・validation・子の別々のSourceAdmissionを区別した。

このcheckpointの通常native workspace試験はRust 1,468件とprocess 17ケースが成功し、21件は除外された。正式acceptanceや新しいcross-target実行の成功を意味しない。任意continuationのdecode／新session復元、共通operation輸送、Tokenizer terminal codec、remoteの計量・要求対応・終了回収、validation失敗時も含めた一般的な費用統合は未完了として維持する。現在のAPI・試験入口・保証範囲は[開発手順](../development.md#saved-reader-and-tokenizer-echoes)にまとめた。

## 2026-10-08: 親Budgetでの入力検証と限定cross-target確認

checkpoint `aee9e00a37d8c616774f35118371f1b492316253` は、任意選択の `IssuedInvocation::issue_with_parent_validation` を追加した。入力検証とcontext digest算出を実際の親Budgetで行い、成功後だけ変更していない子grantを予約する。検証途中までの成功した課金をエラー後も保持し、拒否された課金を加算しない。資源停止はstickyなままとし、検証失敗だけでは未精算予約を作らない。旧separate-budget APIの挙動は維持した。

[専用試験](../../apps/provider/tests/transport/reply/delegation/parent.rs)は、全Usageの直接実行oracle、7加算資源の残容量ぴったり／超過、現在depth・祖先上限、入力／context段階の途中停止、停止しないcontextエラー後の再試行、API返却後の外側scopeのunwindを検査する。検証内部panicの一般保証ではない。Tokenizerの実builtinName子実行は旧経路を残し、親直接検証でも成功／実WorkLimitと両echo方式を通す。Grants構築・認可もこのfixtureでは親で計量し、別Budgetのoracleを二重に加算しない。

このcheckpointの通常native workspace試験はRust 1,476件とprocess 17ケースが成功し、21件はignoredだった。全workspace Clippy、警告をエラーとするAPI文書生成、fmt、repository check、task文書整合検査も成功した。同じ実装のreader・wire・provider・suiteをWASIで実行し、runtime 321件と別枠のcompile-fail doctest 1件が成功した。host計測3件はignored、OS processは明示skip、native unwind試験はcfg除外である。

これより前のcheckpoint `498fc8f9355e4b2bb425042664e34c6216e0f9e7` では、CIの基礎12 crateとsuiteのsentence／sentence_html targetをWASIで確認し、runtime計856件と別枠のcompile-fail doctest 10件が成功した。同時点のportable 11 crateは、CI指定featureを用いるbrowser WasmとARMv6-Mのcompile-only検査に成功した。これらを新APIの同範囲再実行と扱わず、実browser描画・firmwareリンク・実機動作・全WASI CIの成功とも扱わない。

remoteの継承会計・要求対応・信頼できる実測・輸送費用・終了回収、全host作業の費用統合、正式acceptanceは未完了として維持する。現在の入口と保証範囲は[開発手順](../development.md#parent-budget-invocation-validation)を参照する。

## 2026-10-09: 要求照合・返信費用・process取消の接続

checkpoint `dc015bc0daa00c62a43bafecb4f6d9d07af34b02` は、試験専用Dependent handlerで明示source閉包を検査し、先行Scalarの後に実builtinNameを実行して、親への精算・OperationReply CBOR・保存済みTokenizer echo再開を結合した。親直接検証、成功／実WorkLimit、二つのecho方式の追加4経路と従来8経路を保持する。通常native workspaceはRust 1,477件＋process 17ケースが成功、21件ignoredだった。これは標準Dependent executorの実装ではない。

checkpoint `804951c3b1a069704bff58f75ce1342f58be0f95` は、Invokeの全fieldと順序を持つsource/resource表を先課金で照合する `check_saved` と、親の未予約容量で照合してから精算する任意入口 `settle_saved_request` を追加した。既存Error enumと従来のsettleを維持し、新入口には別のSavedSettlementErrorを使う。同じcontextでもinput・operation・ID・grantの異なる要求を拒否し、比較費用を保持する。完全一致する要求でも、実測対象や同じ内容の別attemptを認証するわけではない。通常native workspaceはRust 1,486件＋process 17ケースが成功、21件ignored。core・reader・wire・provider・suiteのWASIではruntime 496件＋別枠のcompile-fail 1件が成功、3件ignored、OS processは明示skipだった。portable 11 crateのbrowser Wasm／ARMv6-Mはcompile-only確認に留まる。

checkpoint `a495410cb8c0b8d73cce4211c6a1ed8ec1e1a6a1` は、受信・decodeから意味検査まで一つのBudgetで順次行う `receive_reply_with_budget` を追加した。旧入口の独立Budget引数・エラー区別・失敗時closeは維持する。実送信、受信/codec、意味検査の三段階の停止、非停止拒否と実I/O失敗、照合後精算を親予約へ結び、同じceilingの直接経路と全Usageを比較した。wire Reportの極大Workを独立観測として加算しない。通常native workspaceはRust 1,491件＋process 17ケースが成功、21件ignoredだった。

checkpoint `b8c46c68f5c32a890256666c1df581b279864bb3` は、実processのSilent/Partial期限切れに親予約とlifetime取消を結合した。実Invoke送信後に応答待ちへ入り、main側で終了・回収と終了statusの再取得を確認する。workerは実read回数によるWork差だけを補正したoracleと受信Usageを照合し、未精算drop後のCancelledと再予約拒否を確認した。専用native process harnessは既存17ケースを保持した19ケースで成功した。この時点の全workspace検証は進行中であり、専用harnessの成功で置き換えない。強制終了はremote消費0の証拠ではなく、この試験では精算しない。

### 継承会計transportで保持する区別

通常のnative dispatcher／schedulerは、Invoke.limitsを同じ累積Budgetへ適用する絶対ceilingとして扱う。一方、IssuedInvocation／IssuedBudgetの明示的な委譲入口は、7加算資源の相対grantと絶対depth上限を扱う。既存Invoke全体を相対grantへ再解釈しない。trusted local childのbasisは実行開始時点の親Usageであり、予約発行時の固定値でも、current depthを歴史的peakから復元した値でもない。

共通wireのInvoke／Resume／ProviderFrameには、継承basis・active depth・会計方式・attempt認証のfieldがない。通常dispatcherのceiling規則を変えず、親側の送受信費用をどの時点のbasisへ反映するか、Await/Resumeでどの会計状態を保持するか、明示source再受入と保存済みReaderへどう接続するかは未解決である。完全Invoke照合、Report codec、process回収の成功をremote観測認証へ昇格させない。R006/R009/R014/R017と正式acceptanceの状態は、この部分進捗から変更しない。
