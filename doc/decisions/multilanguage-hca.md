# 複数言語・NEPL3h/C/Aと構文コメントの統合案

識別子: `nepl3-multilanguage-hca-design-20260913-r4`。
状態: **設計草案、実装未着手、Draft PRで保持する**。
2026-09-13の統合提案r1と、その後のコメント設計訂正を統合する。
最後の訂正を優先し、対象を明示する`annotate Sentence target`を標準とする。
独立comment、commentedという別名、`#:`による保存付きtrivia、旧コメントの恒久互換を撤回する。
本文の「要求する」「拒否する」は将来の契約であり、現在の処理系の挙動ではない。

ユーザー提案の調査基準はmain `bea06fa8755eda0faf534072bf4f4c3677b11920`、
NEPL3h草案はPR #158の `8ff534b387d8c1f8eea439db65ed1b6759c353e2`。
今回のローカル実装照合はmainの `d8ca89264ca17ebe6c4aba3874cac2775c15b98d` を用いた。
この文書を置くbranchの元基準や、ユーザーが行った外部調査と混同しない。
GHC/CircuitGameの再build・外部ソースの再監査は今回行っていない。

## 1. 所有する意味と依存方向

| 所有者 | 責務 |
| --- | --- |
| Foundation / reader / engine | 言語中立のSource、Origin、schema、構文、診断、Budget、交換と解析 |
| 共通Interop package | 操作の意味契約、producer binding、生成slot、有限の型付き値 |
| NEPL3h | 独立GHC frontend、Haskell固有の型・評価・module意味 |
| 別のNEPLプログラミング言語 | その言語の意味論と、実装すると宣言したInterop操作 |
| NEPL3c | typed signal bundle、primitive、部品合成、一般回路グラフ、帰還・伝搬 |
| NEPL3a | Sentence/Inline、Ruby/Anno、構文コメント、付与の共通surface pattern |
| NEPL3d | Article/Section/Paragraph/Table等の文書構造 |
| NEPL3hdl | clock/reset/register/memory等の同期RTL・合成。新Cと別domain |
| Adapter / Composition | 言語間変換、型付き受渡し、runner、資源・権限・出力 |

Hは必須マクロ言語、universal evaluator、中間言語にしない。C/A/D/Interopのcore、
生成器、既定testを不要なGHC依存へ結合しない。複数の言語のproducerを同時に利用できる。
全言語へcompile/evaluate/renderを強制する巨大traitや中央の言語enumは追加しない。
native typed fast pathとNDF/1のportable pathを維持する。
parse・表示・文書化から任意のproducer、Haskell、回路testを暗黙実行しない。
前方の構文拡張の準備にproviderが必要な場合は、後述の明示操作・権限・停止境界を使う。

この整理は[外部拡張契約](../spec/22-external-extensions.md)の責務分離を維持する。
新規Hは独立repoを予定し、既存domainの抽出は公開API・外部consumer・配布の条件に従う。
repo分割だけを契約・権限・ライセンス問題の解決としない。

## 2. 操作単位の互換性と差し替え

構文shape、データschema、操作の意味、deployment capability、checkpointの互換性を
それぞれ判定する。同じ前置構文、field形状、純粋な型、署名やdigestは意味等価の証明ではない。
Hの非正格式を別言語へalias変更だけで渡さない。直接ソースを共用する場合は、対象subset、
評価順、bottom/error、型・束縛、整数幅、effectまで別のsource互換契約で規定する。
不適合なら明示source移行と新snapshotを要求する。

| identity / 契約 | 内容 |
| --- | --- |
| SurfaceIdentity | 言語、category、reader、形状、binding |
| OperationContract | 正確な入出力descriptor、意味、失敗、effect、比較条件、受入suite |
| ProducerExport | 言語中立の公開名と操作契約への参照 |
| ImplementationBinding | 公開名に割り当てるprovider asset、runner、承認済みcapability |
| ImplementationIdentity | 実asset、toolchain、build、SDKと依存lock |
| InvocationIdentity | 元source集合、Profile、操作、資源、options、epoch、request |

既存SchemaRef、OperationRef、Report、Complete/Invalid/Stopped/Awaitを再利用し、
既存fieldを重複した巨大recordや別の輸送protocolへ写し直さない。
初版のABIはdescriptor完全一致を要求する。変換は方向と版を持つ明示adapterにする。
同じ公開操作を複数実装が提供できるが、一つの解決済みProfileのbindingは一意に固定する。
未選択の複数候補は曖昧さとして拒否し、失敗後の無断fallbackはしない。

境界の値は有限の型付き値と、owner/scope/schema/epoch/許可操作に束縛したhandleである。
Rust pointer、closure、GHC AST/HValue、StablePtrを共通ABIにしない。
型identityは共有契約packageが所有し、実装言語・alias・assetの変更で変えない。

差し替えは候補検査、比較試験、旧要求の完了またはcancel、新Profile/epoch発行、cache失効、
再実行、完全な応答検査の順で行う。旧continuationを別providerへ渡さない。
失敗時は部分採用せず明示rollbackとする。旧結果の表示には旧identityを示す。
稼働中heap/stack/sessionの移送は保証しない。checkpointには独立したschemaと復元検査が必要で、
未対応なら元入力からreplayする。

実artifact cacheは実装・依存・権限まで含み、domainの意味fingerprintとは分ける。
意味結果が一致しても古いSource/Originを新snapshotへそのまま付け替えない。
UIの採用は[TEA契約](../spec/14-web-ui.md)の全request identityを照合する。

## 3. 言語中立の構造生成

以下は新しい提案構文であり、production forms catalogへ登録済みではない。

```text
generate Adder8
  signature
    cons port operands named Operands8 nil
    cons port result named AddResult8 nil
  call ripple
    cons argument width natural 8
    cons argument stage component FullAdder
    nil
```

`generate(name, signature, invocation)`はarity 3、`signature(inputs, outputs)`は2、
`call(producerName, arguments)`と`argument(parameterName, value)`は2。
値はnatural/integer/text/boolean/component/type/values等の型付きconstructorで表す。
`ripple`は公開producerを指し、Hや別言語の実装はcompositionが選ぶ。
例の型・component・producerは事前に解決済みであることを要求する。
関数の引数数・import結果・後からの生成結果で既読formのarityを変えない。

生成slotはC body、宣言、category付きsyntax、A Sentence、D block、test列を分ける。
入力には挿入先、期待category/interface、明示環境・資源、予約した生成identity領域を持たせる。
body生成からambient import、primitive定義、全体Profile変更は返せない。
宣言・reader生成は別の明示phase・capabilityで行う。検査済みの更新は、定義されたscopeの
後続位置へ適用できる。必ず次のparse sessionまで待つという以前の制約は撤回する。
通常のbody生成slotからProfileを更新する権限は与えない。

生成名は展開scopeとlocal identityを持つ。文字列一致で外側をcaptureせず、明示した
EnvironmentProjectionだけを許可する。wire/schema、Source/Origin閉包、権限、slot、
型、driver、有限instance化、primitive policyを検査してから原子的に挿入する。
source-less生成はSynthetic Originを使い、架空Spanを作らない。
生成後の回路stepからproducerを呼ばず、生成器runtimeなしでCを検査・実行できるようにする。

## 4. H固有の契約

[NEPL3h草案](nepl3h-ghc-frontend.md)のGhcPs入口、独立adapter、Cabal双方向import、
診断対応、SDK固定、finite NDF境界を維持する。Hの型検査器・runtime・標準libraryを自作しない。
`apply apply f x y`はApply(Apply(f,x),y)。関数ごとの構文arity登録は不要である。
QName、literal、Expr/Pattern/Type/Declaration/Import/Export/Moduleを区別し、
既知headと衝突する名前には明示参照を設ける。具体的form表は実装前にschemaと確定する。
Haskellの非正格性、再帰let、overloaded literalを保存し、未対応surfaceを黙って簡約しない。

通常Haskell A→NEPL3h B→通常Haskell Cの混在buildを要求する。
native、cross Wasm、事前compile済みproducer、browser bytecode、browser独立Wasm出力、
browse-onlyは別capabilityである。GHCのwasm32-wasiとNEPL3のwasm32-wasip2を同一ABIにしない。
compiler不在でもC/Aと代替producerは成立する。原稿を無断でserverへ送らない。
通常関数によるDSL生成、H自身のsyntax macro、THのstagingは別機能とする。
GHC build・第2言語・実descriptor digestは未選定で、架空の固定値を与えない。

## 5. 新Cと旧Circuitの境界

現行[07章](../spec/07-circuit.md)は二値・同期state/nextとDAGを持つ別契約である。
新Cは一般の有限信号グラフを扱い、旧意味を同じschema/digestへ上書きしない。
旧同期RTLの責務はNEPL3hdlの起点として分ける。拡張子.neplcだけで旧新を推測しない。
現設計からのC/HDL移行方針はコメント撤去と別であり、コメント専用の互換runtimeは残さない。

| 新Cの領域 | 契約 |
| --- | --- |
| 型 | Bit / Array(N,T) / nominal Struct(StructId, ordered fields) |
| layout | field宣言順、array index昇順で再帰flatten。数値のLSB/MSB意味は別adapter |
| 空束 | 空structとゼロ長arrayを許可。ゼロ長を介する再帰型も初版では拒否 |
| 配線 | ref/get/index/slice/record/items/concatはgate追加なし。範囲・overflowを検査 |
| record | 全fieldをちょうど一度指定、指定順から型のfield順へ正準化 |
| driver | sink leafにちょうど一接続、netに外部入力かprimitive出力の一driver。fan-out可 |
| graph | aliasのみの無driver循環を拒否。primitiveを通るfeedbackは許可 |
| 依存 | 型・source import・module instance化は初版DAG。宣言順の制約とは分ける |
| primitive | 通常packageの有限Boolean関数、完全真理値表。名前norだけで認証しない |
| basis検査 | Entry内の明示dead instanceを含む実primitive closureを許可ID集合と照合 |
| flatten | moduleを展開して使用primitiveを保存。別basisへの置換は別操作 |
| 同値 | Boolean関数、gate構造、unit-delay traceの保証を分ける |

Signal<T>は接続、Value<T>は時点の観察値であり、Haskell Bool/list/recordと暗黙変換しない。
structの型identityと公開layout閉包は、別言語のSDKでも同じ契約packageに解決する。
module/source unit/fileを区別し、manifestでnamespaceとsource集合を明示する。
各fileを独立parseしてunit宣言を結合し、列挙順で意味を変えない。
通常importとpublic/privateを用い、parser stateを引き継ぐtext includeを既定にしない。

simulationは全primitiveが更新前snapshotを参照する同時unit-delayとする。
wire/struct/module境界の遅延は0。初期値はLogic3のX、zeroedは明示seedだけである。
X入力のBoolean補完が全て同じ出力なら既知、異なればX。未知値間の相関を追う保証はしない。
observeは進めず、driveは保持入力を更新、advanceは段数だけ進める。
settleはStableKnown/StableUnknown/PeriodicKnown/PeriodicUnknown/Limitを分け、
内部stateを完全比較する。Limitを発振、X固定点を物理的安定と呼ばない。
truth testは組合せ性を検査して安定出力を比較し、trace testはseedと操作列を明示する。
analog遅延、metastability、FPGA合成の保証ではない。

## 6. Aの文章モデルと正式なコメント構文

DocのSentence/InlineをA所有へ抽出する。Docは文章位置にAを使い、Math注記もAへの
adapterへ移す。A coreへDoc/Math/C/GHCのruntime依存を入れない。
Sentence、Text、Concat、Ruby、Anno、inline code/emphasis/break/link/asset等を扱い、
Article/Paragraph/Tableや全体link解決はDが所有する。
foreign inlineは登録済みbridgeで扱い、全言語enumをAへ追加しない。

literalと前置構築は同じ意味モデルにlowerする。Ruby/Annoの境界とnote順、Break、
escapeとTextの違いを維持し、文章対応は[authoring](../authoring.md)どおり文単位とする。

```text
annotate
  "前段からの[桁上/けたあ]がり。"
  ref carry
```

| form | fields | arity | domainへの寄与 |
| --- | --- | ---: | --- |
| annotate | annotation: A/Sentence, target: Host/T | 2 | 子Tと同じ寄与、子との付与関係を保存 |

`annotate`を唯一の標準wrapper名にする。独立したA.Commentと`commented` aliasは設けない。
文章内部の`anno`は別機能として維持する。
Annotated<T>は共通surface patternであり、generic category機構の新設を必須にしない。
必要なhost categoryごとの具体schemaへ特殊化し、field schemaが異なる型に同じKindIdを使わない。

各packageは注釈可能categoryを宣言する。Cのunit/module/struct/field/port/instance/connect/
signal expression/test case、HのModule/Declaration/Expr等を候補にする。
list constructorや内部delimiterへ一律に適用せず、`annotate "..." nil`は標準では拒否する。
hostが受理を宣言していないcategory、literal内部、raw code内部を横取りしない。
`call ripple ...`は中立Interop/Invocationであり、H/Exprではない。H製producerを指していても
注釈の対象categoryはInvocationで、そのcategoryへの登録を要求する。

複数のbody itemを説明するときは、module/body/struct/unit等、hostに実在する所有nodeを対象にする。
注釈の都合だけでtransparentな`group`を追加する案は撤回する。
groupingが必要な言語は、その言語自身のscope・可視性・export・前方参照・順序を正式に定義し、
検査してから対象categoryとして登録する。意味が同じという宣言だけでbinding保存を推定しない。
moduleやfile全体は対応するmodule/unit rootを包む。root categoryも明示登録を必要とする。
metadata、license、pragmaは必要になった時に用途別の契約を定義し、説明注釈に実行指令を隠さない。
formatterの見た目を行末コメント風にすることと、保存する正式source構文は区別する。
source printerは`annotate Sentence target`を出力し、旧#コメントを復活させない。

```text
Source → Reader / prefix parser → Annotated<T>を含むParseTree
                                  ↓ host lowering
                     domain contribution + annotation relations
```

parserはwrapperを剥がさず、Annotated<T>自体のnode、Sentence、Source/Originを保持する。
host loweringだけが注釈の実行意味への寄与なし／子と同じ寄与を明示する。
無効入力や未対応nodeを寄与なしとして捨ててよいという意味ではない。
wrapper/target node、位置、解決可能なEntity/field、生成由来をannotation relationへ保存する。
名前解決が成立していない注釈に架空Entityを付けない。annotationNode→targetNodeと、
解決できた場合のtargetEntity、Source/Originを明示する。近接位置から対象を推測しない。

E(annotate(a,x)) = E(x) を、妥当な注釈について各host categoryで検査する。
構文・source・文書・hoverは異なり、同じartifact hashになる保証ではない。
source-sensitiveな反射/TH等には別capabilityと入力identityを与える。
標準producerへannotationやambient sourceを暗黙に渡さない。
生成Annotated<T>も手書きと同じschemaで検査し、呼出箇所・生成器・local nodeのOriginを保持する。
syntax生成slotは文章だけを返して近くのnodeへ付けさせず、対象を含むwrapperを返す。
注釈付き生成は正式なAnnotated構文とtargetを保持し、binding/lowering後にrelationを派生する。
semantic fragmentとsidecarだけで元の注釈syntaxを代替する経路は設けない。

## 7. 撤回する機構と移行の原子性

`#:`導入子、保存付きコメントtrivia、bare commentの全token位置横取り、
旧#コメントを恒久legacy profileとして維持する案は撤回する。
最終状態ではcomment-as-triviaを仕様・production schema・enum・codec・readerから除く。
Whitespace/Bom/必要なSkippedとleadingTriviaの位置保存は別責務として残せるが、
Skippedへの改名で同じコメント消去を隠して残さない。

現時点の確認箇所は次の通り。これは検索で見つかった入口で、影響範囲の全数監査完了ではない。

| 入口 | 実装時に一緒に直すもの |
| --- | --- |
| [03章](../spec/03-reader.md) | standard # trivia、skip lexeme分類、継続・sidecar契約 |
| [core view](../../crates/foundation/core/src/view.rs) | TriviaKind::Commentと説明 |
| [tokenizer](../../crates/foundation/reader/src/tokenizer/session.rs) | raw.starts_with('#')による分類 |
| [wire view](../../crates/foundation/wire/src/view.rs) | Commentのencode/decode |
| [engine sidecar](../../crates/foundation/engine/src/portable/region/sidecar.rs) | portable mapping |
| [contracts](../../interfaces/contracts.json) | TriviaKindの公開variantと派生foundation descriptor |
| languages / builtin providers | trivia readerの実認識、各skip設定、configuration identity |
| Doc/Math reader・printer | A抽出後のschema、payload、Source閉包と文書生成 |
| tests / examples / fixtures | 構文コメントへの移行、位置期待値、旧構文拒否 |

tokenizerの分類分岐だけ消しても、元のskip readerが#を消費すれば解決しない。
実認識、native/portable mapping、descriptor生成、関連digest・revisionを同時に更新する。
旧foundation revisionを新schemaとして受理せず、productionに旧Comment decoder、
deprecated variant、feature flag、warning-only acceptanceを残さない。

実装の統合単位は、新A schemaと必要host対応→source移行→文書・生成物・試験更新→
旧skip/variant/codec撤去→全体検査までである。途中段階を移行完了としてmainへ統合しない。
一時移行toolが必要なら統合前に役割を終え、最終treeに恒久migration subsystemを残さない。

source変換は内容・説明対象・categoryを確認する。子、既存の所有node、rootのどれを包むかは
同じ置換規則では決められない。内容をTextとして保存し、旧本文の括弧をRuby/Annoへ再解釈しない。
対象が曖昧なら執筆判断を行い、適切な受入位置がなければcategory設計を先に確定する。
直後の式へ自動付与したり、移行用の独立Comment nodeを残したりしない。
Rust/Python等のhost言語のコメント、Markdown見出し、文字列中の#、raw code、歴史資料を
NEPL lexical commentとして一括置換しない。履歴の破壊やGit rewriteは本作業に含めない。

通常の公式NEPL profileで旧#コメントはsyntax errorになる。負例として旧入力byte列を残す。
これは互換実装ではない。他言語の通常source入口や明示opaque領域内の#まで禁止する規則ではない。
削除完了の検索はproductionと現行公式NEPL入力を対象にし、撤回理由を説明する本文や負例に
旧語があることを残存実装と誤判定しない。

## 8. 移行台帳・検査・実装順

| 領域 | 変更する契約 | 成功の根拠 |
| --- | --- | --- |
| A / Doc / Math | Sentence/Inline所有、payload、printer、HTML準備、hover | literal/prefix同値、参照閉包、旧文章内容の保存 |
| reader / foundation / wire | comment-as-trivia削除、schema版更新 | enum/descriptor/codec一致、旧revision拒否、#負例 |
| 各host category | Annotated<T>の明示受理と意味射影 | parse treeに残る、寄与不変、annotation関係と位置保存 |
| C / HDL | 新Cの型・graph・遅延と旧同期意味の分離 | struct往復、driver、basis、帰還trace、移行保証subset |
| Interop | 中立操作とbinding、生成slot、権限 | 二つの独立言語、GHC不在、同時利用、slot不正拒否 |
| runner / UI | 全identity・停止・失効・原子的採用 | cancel後旧応答拒否、continuation移送拒否、部分結果未採用 |
| planning / docs | tasks/dependencies/acceptanceと正本整合 | catalog検査、生成projection差分、未実行状態維持 |

正式仕様へ採用するときは、03/04/05/07/09/10/14/22章、forms、interfaces、
language grammar、builtin reader、全影響codec・printer・consumerを同じ設計修正に含める。
[canonical登録](../canonical.json)済みページは.nepldを編集してprojectionを再生成する。
この草案は未登録Markdownのため、Markdown一つを正本として編集する。

実装順は契約固定、Aと基本C、中立生成slot、実際の二言語producer、H固有経路、
文書化等の統合、外部package・配布の順とする。各段階は実依存が成立する範囲で進める。
Hのbrowser完成をC/Aのbuild条件にしない。次の受入はすべて未実行である。

1. head/category/arity、literal・raw境界、NoMatch/NeedMore/Stoppedとrollbackで構文情報を失わない。
2. Annotated<T>がparse tree、Source/Origin、editor/document projectionに残り、host意味は不変。
   非対応categoryとnilへの注釈を拒否し、targetのbinding/export/可視性と対象identityを保つ。
3. 旧skip規則・Comment variant・codec・互換flagがなく、旧#入力と旧schemaを拒否する。
4. schema/native/portable往復、Unicode位置、生成注釈のOrigin、参照・slot不正を検査する。
5. 同じ操作を二つの独立したNEPLプログラミング言語で実装し、同時利用とbinding差し替えを実証する。
   同じ実装を二つのwrapperで呼ぶ試験では代替しない。第2言語の選定は未決である。
6. GHC不在のC/A・代替producer、hygiene、型identity、0/巨大整数/空束、原子的失敗を検査する。
7. Cのlayout往復、重複driver、alias cycle、dead instance、source順不変trace、set/hold/reset、
   X固定点・周期・Limit、Boolean/構造/trace保証の区別を検査する。
8. Hの非正格性・recursive let、Cabal相互import、native/cross/browserを個別に実行する。
9. compile・lazy serialization・decode/checkを含む非停止/巨大出力、権限・cache/epochを検査する。
10. C/AからDへの保存済み結果の文書化ではproducerやsimulationを起動しない。

既存test/conformanceと共通の証拠収集を使う。新しい受入判定framework、review script群、
source snapshot複製は作らない。[ADR 0008](0008-evidence-and-publisher-boundaries.md)を維持する。
自作MITと同梱componentの条件は別に管理し、実配布物ごとの依存・通知・必要な材料を確認する。
今回、正式schema・runtime・tasks・acceptanceの状態は更新しない。

## 9. 今回の訂正の範囲

統合提案r1のD09/D22、14章、19章の保存付きtrivia、AC22の#:/旧#互換は、
第6–7節の対象付きannotateと完全撤去へ置き換える。続く独立Comment/Commented案も撤回する。
D26/AC30の旧consumer維持も、
旧Commentを現runtimeでdecodeする保証を含まない。過去の調査件数や補助検査は実証に転記しない。
旧Cと新Cの意味を同じschemaにしないこと、Hを任意の一言語にすることは変更しない。

この草案の独立レビュー・文書整合検査は、A/C/Hの実装受入ではない。
PR #158はDraftを維持し、本書作成をcompiler実装・新repo作成・依存追加の許可としない。

## 10. 前方確定contextと関数適用

解析開始前に全headを固定することは要求しない。各出現は、その前に確定したcontextと
token自身から字句境界・category・head identity・arityを決める。先行するimport、宣言、
型・component定義、macro準備が、後続で使えるform/category/reader/名前を導入してよい。
通常の値のbindは構文head登録を自動的には意味しない。登録する場合は明示した操作契約を使う。

親headのfield数と各子contextの決定規則は親を識別した時点で確定する。
子iの具体的contextは、その規則に従い既読の子0..i-1を使って選べる。
将来のsource、未読の子、当該headの子の評価結果で親のarityを遡って決め直さない。
token readerが現在tokenを読むための先読みやNeedMoreを行うことと、後続宣言で既読境界を
再解釈することは別である。新readerは更新後の開始位置から使い、更新前のtokenは変えない。

先行importの解析→package/provider解決→schema/操作検査→context更新→後続解析の順にする。
取得・生成の実行が必要なら承認された能力と同じ親Budgetの下でAwait/再開する。
未解決headを推測せず停止し、失敗時は更新を部分公開しない。
provider実行は構文原則上排除しないが、通常のparse権限だけで任意workspace codeを実行しない。
能力不足なら明示的に未解決/停止を返し、無断のnetwork取得・guest評価はしない。

contextは単調追加またはscope局所更新とし、同名headのshadowing、終了時の復帰、曖昧さ拒否を
宣言する。同じ綴りが別contextで別arityでも、各出現で一意ならよい。
固定した入力・依存asset・初期Profileから更新履歴を追跡できるようにし、可変global registryにしない。
各context revisionと適用位置・scope・入力identityを継続/cacheへ束縛する。
backtracking/NeedMore/cancelでcontextをcheckpointへ戻し、外部effectを巻き戻せるとは仮定しない。
準備済みassetの再利用と結果の採用を分け、再開による無断二重実行を避ける。

現[04章](../spec/04-grammar.md)はHeadProviderのshapeと既読子context選択を定義している。
一方、現行の宣言作用範囲は導入bodyの部分木、childContext返信は登録済みEntryContextの選択である。
同一sourceの後続領域へ新schema/readerを入場させる一般経路は、この範囲との差分を仕様・
registry・provider・継続・権限・再現性の契約へ反映して実装する対象である。
この草案だけで既存ResolvedProfileの可変化や動的入場の実装完了を主張しない。

### 構文arity・callability・saturation

構文arityはparserが読む子の数、関数の引数型とcallabilityは意味論、saturationは適用状態である。
先行する宣言/importから固定shapeを得たheadは直接使える。componentのport数やstructのfield数を
後続constructorのshapeに対応させることもできるが、その登録・型・出力・名前衝突規則を明示する。
port数や型情報を発見しただけで現在のform shapeを暗黙変更しない。

`apply`は固定arity 2の適用constructorで、適用構造を対象の値と分離する。
`apply apply f x y`はApply(Apply(Reference(f),x),y)である。
対象が後方定義、高階値、部分適用、異なる構文arityの未解決候補等で直接headにできない場合にも、
値参照を許すcategoryなら固定shapeで解析できる。callability・型・saturationは後段が検査する。
Haskellの型クラスoverloadがそのまま異なる構文arityのoverloadだと仮定しない。

未知headを万能のarity 0へ自動fallbackしてはならない。categoryが参照leafを明示受理するか、
明示参照constructorを用いる。例えば提案表記`apply ref add x`では、`ref`自身はarity 1、
その子Name tokenがarity 0で、得られる意味値がReference(add)である。
既知の2-arity headとして登録された裸のaddを、引数不足だから値参照へ読み替えない。
Haskell名の後方解決も、その言語のbinding規則が認める場合に限る。

| 出現位置の条件 | 解析方法 |
| --- | --- |
| 先行contextでhead shapeが一意 | 直接headを使用できる |
| 対象の構文arityが未確定、参照leafが明示的に許可される | 参照とapplyで適用構造を明示 |
| 既知headを部分適用・高階引数として渡す | 明示値参照または値参照専用categoryを使用 |
| 関数を返す式へさらに適用 | applyを重ね、型と飽和は後段で検査 |
| headも参照leafも認められない、またはshape候補が曖昧 | 構文上拒否。都合のよい候補を推測しない |

Hが通常の関数参照とbinary applyを選ぶのは言語固有の判断であり、全NEPL言語の必須方式ではない。
別言語は、先に解決した関数・型・componentを直接headとして登録できる。
本節の説明例は新しいproduction grammarではなく、各language packageで具体化・検証する。

## 11. 上位契約の再監査と追加の移行条件

### skip / discard

標準NEPL profileのskipは空白・BOM等のlexical separationに限定する。
annotation/directive/pragma/metadata/documentationを消すreaderを登録してはならない。
Skippedへ分類名を変えてcomment消去を残す方法も禁止する。discardはtoken内部delimiter等に
用い、prefix構文に相当する情報を隠さない。外部readerの意味が名前や型だけで分かるとはしない。
公式reader/profileの実認識と適合試験を検査し、動的導入時も同じ契約・承認を要求する。

### 具体化formとbinding透過性

Annotated<T>は説明上の型記法であり、現Grammarにparametric categoryを新設したことではない。
各host surface packageに具体的な固定shapeを登録する。

```text
annotate : A/Sentence × C/ModuleItem → C/ModuleItem
annotate : A/Sentence × C/SignalExpr → C/SignalExpr
annotate : A/Sentence × C/Field → C/Field
annotate : A/Sentence × H/Expr → H/Expr
annotate : A/Sentence × H/Declaration → H/Declaration
```

共通化するなら実際に重複するGrammar declaration生成だけを共有する。
実行時genericや未定義のT解決でarityを決めない。文章内部のA/Inline.Annoとhost wrapperは別kindである。

Domain(annotate(a,x)) = Domain(x)に加え、hostのBinding/Export/Visibilityもtargetと同じにする。
`visit target`だけでは子のexportを外へ返す保証にならない。
現binding planのvisit/import/export/propagateと各categoryのscope規則に従い、targetの参照・
導入・exportを保存する具体planを定義する。recursive header収集でもwrapper越しに対象を認識する。
source追加でarena番号や生のFactSet IDが同一になるとは要求せず、対応付けたhost Entity・参照・
scope・可視性・export候補が同じであることを検査する。注釈側のfactsは別に保存する。

Aの文章は独立namespace/rootで解析する。文章内の同じ名前文字列からhost Entityを暗黙captureしない。
明示参照とEnvironmentProjectionによるgrantを使う場合だけ外側候補へ接続する。
AnnotationRelationは元の正式syntaxから派生する結果であり、syntaxの代用品ではない。

### domain依存とforeign identity

表層はforeign A/Sentenceを保持し、hostは自分のwrapperとtargetの投影規則を所有する。
文章意味の検査・render・検索・Doc取込みはAを解決するadapter/compositionで行う。
C/Hのdomain coreへA coreを直接依存させない。DがAの文章意味を利用する場合も、
明示foreign参照とadapterを基本にする。typed直接依存を選ぶならdependencies契約を先に改訂し、
no_std・DAG・責務を検査する。抽出という名称だけでd-core→a-coreを無断追加しない。

現DocのGuestLanguage列挙、MathGuest/CircuitGuest、dev hostの既知言語一覧とpackage文字列dispatchは
一般的な外部言語登録経路と同一視しない。Docに新言語を追加するたびenumを増やす構造を解消する。
DocはInlineMath/DisplayMath/CircuitFigure/Code等の文書上の役割を持ち、guestは正確な
schema/category/package identityを持つforeign syntaxとして保持する。
役割への意味適合はadapterが検査し、同じschema形や表示名から互換性を推定しない。
dev hostをgeneral suiteへ昇格させる際は、明示compositionとResolvedProfileの要件から
ProviderImplementationを選び、完全なOperationRef/実装identity・capabilityを照合する。

Struct fieldは順序付き宣言列を正本とし、名前indexは派生物にする。unordered mapへ戻さない。
旧Doc Sentence、新A、旧Circuit、新C/HDLは所有者・意味変更に対応した別schema identityを持つ。
同じSchemaRefに違う意味を割り当てない。旧schemaの恒久実装を残す義務とは区別する。

### 既存open findingsとの接続

R006（operation/transport）、R009（source/resolved Profile）、R014（Doc表現gap）、
R017（portable source/syntax/reporting）を本設計と切り離さない。
それぞれproducer schema、動的context/差し替え、A/D抽出、生成syntax/注釈/Originの検証へ結び付ける。
草案追加でcloseしない。各実装変更で正式task/acceptanceと対応する失敗・境界試験を更新する。

追加の受入は、前方import後だけ新headが有効、後方宣言で既読shapeを変更しない、
親arity不変と既読子context選択、reader更新位置、曖昧head拒否、scope退出とrollback、
Await中のcontext固定と失効、provider権限不足、参照leafとheadの区別、部分適用と型エラーの分離、
annotateのexport/recursive binding保存、注釈namespace隔離、生成syntaxからのsidecar再構成である。
すべて未実行の設計要求であり、現CI成功から推定しない。
