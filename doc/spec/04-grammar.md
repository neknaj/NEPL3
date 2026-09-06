# 04. Grammar言語

## 方針

Grammarはreader・prefix形状・束縛・表示の定義を同じpackageへまとめる。任意プログラムを小さなGrammar IRへ必ず還元することは要求しない。call/map/thenで登録済みproviderへ委譲できる。

## 1. 文書の根と名前解決

根は `language name revision root declarations`。declarationsはcons/nil列。全constructorは `grammar-signatures.md` に定義する。宣言の順番はmetadata解決に影響しない。category/mode/reader/form kind/namespace/extension aliasの各名前空間で同名を拒否し、参照先をcompile時に解決する。

一つのkindを複数categoryで受け入れることは、field schemaが完全に一致する場合に限り許可する。grammar自身のField宣言とSelectorのfield等、同じ綴りでshapeが異なる場合は異なるkind IDを付ける。modeごとのspellingとschema kindを混同しない。

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

解決済みProfileはsurface、意味、reader/providerの全descriptorと計算済みdigestを登録する。未生成のpackageへ架空のdigestを置かず、同じSchemaRefに異なるfield shapeを割り当てない。Grammar compileはsurface descriptorを作る責務を持ち、Doc/Math/Circuitの意味schemaを勝手に再生成・上書きしない。

必須検査: 未定義category/mode/reader/namespace/provider、重複kind/field/spelling、field型の不一致、readerの空反復、進捗なし再帰、未読fieldへの構文context依存、bindingで非名前fieldを使用、範囲外のstyle selector、foreign alias不足、provider署名不一致。

Grammarが生成したdescriptorと、同じ契約をRustで直接構築したdescriptorは同じengineで動く。埋め込まれたproviderをdescriptorの固定IRへ変換できなくても、その呼出し参照を保持する。

## 7. 構文環境を更新する宣言

通常のvalue bindingは名前解決環境だけを変える。関数値をbindしたからといって、その名前のarityを自動変更しない。

外部HeadProviderは `shape(head, existingContext)` と `context_for_child(head, index, completedChildren)` を提供できる。後者は既に読了した子だけを参照する。schemaを変更する宣言の作用範囲は、その宣言が導入したbodyの部分木とし、復帰時に元へ戻す。

今回の配布4言語はschemaをソース本体の途中で自己変更しない。Grammar sourceをcompileして別の入力に適用する順序を標準経路にする。動的HeadProvider経路は契約試験用の局所構文例で実装・検証する。単にAPIだけ残して未実装にしない。

## 8. Grammar自身のbootstrap

完全な文法表からseed descriptorを生成し、通常engineで自分のlanguage定義を読む。seedのarity手書き表とsourceを独立に二重管理しない。生成した全表・source・seedの対応を機械検査する。

Grammar packageの利用者が機能を拡張しても共通engineを書き換えない。編集対象の文法を変更したら依存package/queryを無効化し、schema digestの異なる結果を混用しない。
