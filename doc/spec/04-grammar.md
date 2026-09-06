# 04. Grammar言語

## 方針

Grammarはreader・prefix形状・束縛・表示の定義を同じpackageへまとめる。任意プログラムを小さなGrammar IRへ必ず還元することは要求しない。call/map/thenで登録済みproviderへ委譲できる。

## 1. 文書の根と名前解決

根は `language name revision root declarations`。declarationsはcons/nil列。全constructorは `grammar-signatures.md` に定義する。宣言の順番はmetadata解決に影響しない。category/mode/reader/namespace/extension aliasの各名前空間で同名を拒否し、参照先をcompile時に解決する。formとleafの宣言はそれぞれcategoryとkindの組で一意とする。

一つのkindを複数categoryで受け入れることは、field schemaが完全に一致する場合に限り許可する。grammar自身のField宣言とSelectorのfield等、同じ綴りでshapeが異なる場合は異なるkind IDを付ける。modeごとのspellingとschema kindを混同しない。

共有kindのsurface descriptorは一つであり、categoryごとのbinding/style/read宣言は別に保持する。DeclarationOriginはform/leafに限ってcategoryを持ち、同じkind名の各宣言位置を区別する。その他の宣言のcategoryはNoneである。この出自の区別はexecution identityへ含め、意味上の宣言順序は引き続き無関係とする。

`builtin Name/Text/Nat/Lang` は配布された基礎readerを直接参照する。`local C` は現在言語のcategory。`foreign Alias C` はprofileで固定した別schemaのcategory。`withmode M R` はその引数だけのmode切替え。`listof R` は専用list categoryを特殊化し、cons/nilの既知shapeへ展開する。

foreign aliasはpackage sourceからURLをfetchして解決しない。profile manifestのSchemaRefを要求し、未登録はMissingLanguage、digest不一致はSchemaMismatch。

## 2. formとleaf

`form Kind Category "spelling" fields binding styles`。
fieldsの長さがarity。各field名はform内で一意。head自体はfieldでない。source範囲は共通parserが記録する。

`leaf Kind Category TokenKind binding styles`。
明示的なtoken kindだけをarity 0として受け入れる。Category内の既知formのspellingとの照合を先に行う。形状に一致しないWordを参照leafとして受け取るかは、そのCategoryのleaf宣言に従う。

読み取っただけではdomainの評価器を呼ばない。compileはGrammarの意味操作として明示的に呼ぶ。

## 3. 束縛plan

planはsource fieldとscopeの関係を記述する。評価順やメモリへの代入命令ではない。

none: このnode自身は何も追加しない。記述し忘れを隠すために子を自動visitしない。fieldのうち語義解析不要のliteral以外には、visit/import/propagate/専用plan/customのいずれかで処理方針が必要。

visit field: 現在scopeでその子のplanを適用。子内部のscopeは外へ漏れない。
group plans: 現在scopeを使ってplanをまとめる。
scope plans: 子scopeを作ってplanを適用。bindの位置より後のvisitだけに新しい宣言を見せる。
bind namespace field: 一つの名前fieldから新しいEntityを導入。名前を作れるfieldはbuiltin Name/Textか、schemaでTextを返すと検査されたleaf。Textのescapeはdecode後の綴りで比較し、元位置はSourceMapで保持する。一般の式を名前に変換しない。
reference namespace field: 名前field（leafではself）を現在scopeに対する参照として登録。
export namespace field: 名前を導入候補として親へ返す。単独ではglobal scopeを変更しない。
import field: 子がexportした候補を現在scopeへ導入。
propagate field: 子のexportをこのnodeのexportとして返す。child内の参照も、そのchildに割り当てたscopeで登録する。
sequential declarations body: 各宣言を、それ以前のexportだけが見えるscopeで処理し、そのexportを追加した新しいscopeで次へ進む。最後のscopeでbodyを処理。
recursive declarations body: 各宣言のexport headerを先に収集して共通scopeへ導入し、全宣言の本体とbodyをそのscopeで処理。
custom provider: 同じScope/Entity/Occurrence/Relation契約を返す専用処理。providerの出力も範囲・ID・scope edgeを検査する。

二相処理として、scopeと宣言を作り、参照を後で解決してよい。ただし上記の可視性を変えない。scope graphは有限。候補が複数ならAmbiguousを返し、最初に見つかった候補へ勝手に決定しない。

lexical namespaceは内側scope優先。openは同じlexical探索を行い、見つからない名前を自由入力要求として返す（架空の定義位置を作らない）。globalは指定されたroot scope内で同名の重複を拒否する。別articleのDocLabelや別moduleのSignalを同じglobalへ入れない。rootの割当てはそのform/packageのplanまたはfacts providerが指定する。

## 4. 非再帰letの完全な規則例

```text
form Let Expr "let"
  cons field name builtin Name
  cons field init local Expr
  cons field body local Expr
  nil
  group
    cons visit init
    cons scope
      cons bind Value name
      cons visit body
      nil
    nil
  cons style head "marker"
  cons style field name "name.definition"
  nil
```

lambdaはparameter/bodyの2fieldとbodyだけのscope。letrecは同じnameをinit/body双方のscopeへ導入する。letrecの名前解決成功は初期化の妥当性や停止性を意味しない。

## 5. style

style selector class-nameを登録する。head/self/field/captureからsource領域を選ぶ。captureはreaderが宣言したcapture名。未定義field/captureはcompile error。

class名はschema所有ID。共通roleへfallbackできる。実際の色をgrammarに固定しない。binding metadataからdefinition/reference修飾を自動で導出する。

## 6. compileの出力と検査

compileは `LanguagePackage{schema, readerPlans, categoryShapes, bindingPlans, stylePlans, extensionRequirements, provenance}` を返す。意味データに加えてどのgrammar宣言から作ったかを保持し、grammar自身にもdefinition jumpを提供する。

ここでschemaはsurface schemaであり、Doc:Sentence等の意味schemaと区別する。標準profileは `nepl3.syntax.grammar`、`nepl3.syntax.doc`、`nepl3.syntax.math`、`nepl3.syntax.circuit` をsurfaceの所有packageとし、意味packageはそれぞれ `nepl3.grammar`、`nepl3.doc`、`nepl3.math`、`nepl3.circuit` に分ける。LanguagePackageはpayloadSchemasとして必要な意味・provider schemaの実SchemaRefも宣言する。sourceのform名・token名・view名を同一descriptorで衝突させず、compiled metadataでそれぞれのkind identityへの対応を持つ。

SyntaxNode.schemaとForeignSyntax.schemaは該当するsurface schemaを指す。Token.kindはWord等のlexical分類なので、Let等のSyntaxNode.kindと同一であるとは限らない。form/leaf宣言が両者の対応とpayload型を定める。SentenceLiteralの外側shapeはarity 0のまま、Token.payloadに入るDoc:Sentenceは意味schemaを指す。engineはsurface構造とpayloadの宣言型を照合するが、読むだけで意味操作を実行しない。

sourceから読むarity 0のleaf（builtin Name/Text/Nat等も含む）はfields=[]とし、値はnode.tokenが指すToken.payloadだけに所有する。builtin引数を親formのAtomへ直接縮約せず、token/head/cover/Originを持つliteral child nodeとして保持する。leafのsurface descriptorは空record、token descriptorのpayload fieldが実際の値型を宣言する。Binding/Styleのfield selectorは該当childのtokenとSourceMapを明示的にたどる。source-lessの意味constructor/printerはdomain意味モデルで提供し、架空のToken.head/Spanで生成構文を埋めない。

surface descriptorの型名はForm:/Token:/View:/Builtin:等の役割を持つ名前空間で分け、同じsource kind名に異なるfield shapeを割り当てない。LanguagePackageの意味identityはsurface SchemaRef.digestとは別で、reader/mode/binding/style/extension等の実行metadataを含める。名前で参照する宣言の順序は除き、skip/take/choice/field/binding列の意味順を保存する。readerの直接ReaderId edgeはpackage内ではDAGとし、再帰は名前付きRefで表す。これは完全Grammar構文の有限ReaderExprと一致する。standalone ReaderPlanの直接cycleとは区別し、rule名順の根からDAGを展開する正準形によって共有・同じ式の複製・arena配置の違いを除く。Refは名前を保持するため再帰の正準形も有限である。

extension requirementは既知の型付きoperation descriptorと署名を照合するが、実行callbackの登録とは別である。name-v1/trivia-v1のreader/v1 adapterはReadRequestを受け、対応するbuiltinを予約なしで実行してReadReplyを返す。予約を持つBuiltinRequestとreader/v1を暗黙互換にはしない。Text builtinは明示した予約付き入口を使う。facts/v1等のcustom bindingもparse/compileだけで自動実行せず、実行操作をhostが選んだ時にcallback未登録ならMissingProviderで拒否する。compiler自身の宣言名・selector検査をfacts callbackへ丸投げしない。

解決済みProfileはsurface、意味、reader/providerの全descriptorと計算済みdigestを登録する。未生成のpackageへ架空のdigestを置かず、同じSchemaRefに異なるfield shapeを割り当てない。Grammar compileはsurface descriptorを作る責務を持ち、Doc/Math/Circuitの意味schemaを勝手に再生成・上書きしない。

必須検査: 未定義category/mode/reader/namespace/provider、重複kind/field/spelling、field型の不一致、readerの空反復、進捗なし再帰、未読fieldへの構文context依存、bindingで非名前fieldを使用、範囲外のstyle selector、foreign alias不足、provider署名不一致。

Grammarが生成したdescriptorと、同じ契約をRustで直接構築したdescriptorは同じengineで動く。埋め込まれたproviderをdescriptorの固定IRへ変換できなくても、その呼出し参照を保持する。

## 7. 構文環境を更新する宣言

通常のvalue bindingは名前解決環境だけを変える。関数値をbindしたからといって、その名前のarityを自動変更しない。

外部HeadProviderは `shape(head, existingContext)` と `context_for_child(head, index, completedChildren)` を提供できる。後者は既に読了した子だけを参照する。schemaを変更する宣言の作用範囲は、その宣言が導入したbodyの部分木とし、復帰時に元へ戻す。

今回の配布4言語はschemaをソース本体の途中で自己変更しない。Grammar sourceをcompileして別の入力に適用する順序を標準経路にする。動的HeadProvider経路は契約試験用の局所構文例で実装・検証する。単にAPIだけ残して未実装にしない。

### 永続する解析選択と検査範囲

ParseTreeはProfile digest、SyntaxBundle、bundleごとのNodeSelectionとRecoveryEntryを保持する。foreignへのpathの各NodeRefはその段階の所有bundleに属し、field名で次のForeignSyntaxを選ぶ。各到達nodeには選択がちょうど一つ必要で、static form/leaf/readのindexはEntryContextのaliasとpackage executionDigestで所有を固定する。既知formの原文spellingをleafへ付け替えてarityを変えたり、親ReadSpecと異なるalias/category/modeの子を置いたりしてはならない。ListOfのtailは同じspineのcontextとReadSpecを保持する。

Dynamicの完成選択は固定HeadShape、providerの操作参照、field順のchildContextsを保存する。childContextsは完成treeではfieldsと同数であり、各実childの選択と一致する。callbackは固定arityやNodeRef/ForeignSyntaxのslot形状を変更できず、解決済みProfileの実package/aliasからcontextを選ぶ。進行中frameは既読または開始済みchildまでの確定prefixを保持し、完成treeと同じ完全性条件を先取りしない。

現在のParseTree.validateは静的選択・参照所有・回復構文・payload型を検査する。HeadProviderの投影要求と操作署名の実装が接続されるまでは、DynamicをUnvalidatedDynamicで明示拒否し、allowlistとshapeだけで成功proofを発行しない。この検査は、任意のreader/providerを再実行して全payloadがその出力であることを証明するものではない。readerの実行等価性、native/portable provider比較、全体のP03完成は別の実行検証で確認する。

Unparsedの先頭をtokenとして既に読んでいる場合は、そのTokenRefとheadを保持し、Unparsed coverがheadを包含することを検査する。未知arityを推測して通常leafへ変えることと、既読tokenのpayload/view/triviaを保存することは別である。tokenを得られないNoMatch等ではtoken/headをともにNoneとする。どちらの場合も不明範囲と理由はRecoveryEntryに残る。

## 8. Grammar自身のbootstrap

完全な文法表からseed descriptorを生成し、通常engineで自分のlanguage定義を読む。seedのarity手書き表とsourceを独立に二重管理しない。生成した全表・source・seedの対応を機械検査する。

Grammar packageの利用者が機能を拡張しても共通engineを書き換えない。編集対象の文法を変更したら依存package/queryを無効化し、schema digestの異なる結果を混用しない。


### WithMode の所属と復帰

`withmode M R` は R の構文rootを所有するpackageのmode Mを選ぶ。Builtin・Local・ListOfのrootは現在packageに属し、Foreignのrootはaliasが指すguestに属する。入れ子のWithModeは内側のrootまで所属をたどり、同じrootに複数overrideがある場合は最内側を使う。無効な外側mode宣言も黙殺せず、対応ownerのmodeとして検査する。

`withmode Code (listof (foreign Guest Sentence))` はhostのcons/nil spineをCodeで読み、各guest要素はSentenceの既定modeを使う。`listof (withmode GuestCode (foreign Guest Sentence))` はhost listの既定modeを保ち、guest要素のrootだけGuestCodeを使う。hostの同名modeはguest modeの代用にならない。Builtinは選択modeのskipを使い、値のreaderは指定builtinを直接使う。

overrideはそのrootへ適用する。通常formの各childは自身のReadSpecで新しいcontextを選び、親のoverrideを暗黙継承しない。同じlistのtailはspine modeを継続する。foreign終了時は保存したhost contextへ戻る。実prefix試験ではhostとguestに同名で異なるreaderのmodeを置き、所属・list tail・child・復帰を検査する。

現在の`LanguagePackage::check`は局所metadataとshapeの検査であり、Foreignのalias/category/modeは解決済Profileで検査する対象として残る。package意味digest、解決済EntryContext/Profile、実parse/Grammar bootstrapをこの局所proofから推定しない。
