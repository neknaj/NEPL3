# 06. Math言語

## 方針

数式の表示対象を保ち、その一部に対する厳密計算を独立した操作として公開する。parse/renderが数式を簡約したり、等式の正しさを主張したりしない。

## 1. 表現

全constructorはmath-signatures参照。Numberは有限十進の原表記を持ち、意味値はBigRationalへ正確に変換する。整数p、分母qはq>0、gcd(|p|,q)=1、zero=0/1へ正規化する。Numberに格納できる値は、約分後のqの素因数が2と5だけの有理数に限る。これ以外をNumberとして構築・decodeする場合はNonFiniteDecimalNumberで拒否する。評価値のRationalはこの制限を持たない。

任意有理数から式を作るconstructor helper `expression_from_rational` は、有限十進ならNumber、それ以外なら整数Numberを子とするFrac(Number(p), Number(q))を返す。元の構文をlowerするときにこのhelperで著者のFracを折り畳まない。`frac 1 2` はFracのまま保持する。Numberのprintはcanonicalな整数または有限十進（指数表記なし、冗長な末尾zeroなし、zeroは0）で、意味値とsnapshotの一致を確認した場合には元lexemeを表示に利用できる。Number(1/3)をprint時だけFracへ変換する設計にはしない。

Identifier leafは数学記号。自由記号を許可し、free symbolはUnresolvedNameのエラーにしない。意味上の入力要求として列挙する。`symbol "..."` で予約語と同じ綴りや複数文字の名前を明示できる。裸のNameとsymbolの同じ綴りは同じ名前解決規則を使う。

letはinitを外側、bodyを新しい記号scopeで読む。sum/integralのindexはbodyだけで有効であり、lower/upperでは外側を参照する。自由記号に架空の定義位置を与えない。

subscript/superscript/scriptsは表示構造。一般の添字を配列アクセスや指数演算に暗黙変換しない。代数的な累乗にはpowを使う。`call`は数学の関数適用表現で、任意のFnコード実行ではない。

## 2. 構造検査

matrixは一つ以上のrow、各rowは同じ正の列数。vectorは一つ以上の要素。fenceは左右それぞれ0または1Unicode scalar。空文字で片側だけの括弧を表せる。

rootのdegreeが数値literalで0ならInvalidRootDegree。その他の定義域はevaluate時にも検査する。表示だけの式に実数/複素数の数値領域を勝手に割り当てない。

CheckedExpressionが保証するのは構造・binding・既知の局所制約であり、全記号の値や全演算の数値評価可能性ではない。

## 3. evaluate

入力はCheckedExpression、自由記号のBindingEnvironment、Limits。出力はExact(Value)またはSymbolic(expression, Requirements)。定義域違反・形状不一致は明確なEvalError。値の種類はScalar(Q)、`Vector(List<Q>)`、`Matrix(rows,cols,List<Q>)`、Truth(Bool)。

評価はsource順の左から右、純粋。letはinitを評価してからbodyを評価する。未解決の記号に依存する部分はSymbolicとし、独立な数値subtreeの計算結果を保持できるが元の構文を上書きしない。

正確に評価する演算:
- add/sub: Scalar同士、等しい長さのVector、等しい形状のMatrix。
- neg: Scalar/Vector/Matrixの全要素の符号反転。
- mul: Scalar×Scalar、ScalarとVector/Matrixの両側、Matrix×Matrix、Matrix×Vector。Vector×Vectorは意味が曖昧なのでOperandShapeMismatch。
- frac: Scalar同士、分母0はDivisionByZero。
- pow: Scalarの整数指数。0^0は空積として1。0の負指数はDivisionByZero。非整数指数はSymbolic(NonIntegralExponent)。
- sqrt: 非負Scalarの分子分母がともに完全平方ならExact。非平方はSymbolic(AlgebraicValueRequired)、負ならSymbolic(ComplexValueRequired)。
- root: 正の整数次数だけExact候補。奇数次数の負数を許し、分子分母が完全n乗ならExact。次数が不正ならInvalidRootDegree、非完全冪ならSymbolic。
- equal: 同じ種類・形状のExact値の等値。Scalar/配列の比較を混同しない。
- lt/le: Scalar同士だけ。
- transpose: Matrixの転置。Vectorは1×nのMatrixにする。
- det: 正方Matrixだけ。正確な有理数の消去法を用い、pivotは最初の非zero行。形状不正はNotSquare。
- sum: lower/upperが整数で有限、bodyがScalarとなる場合をexact domainとする。inclusive区間。upper<lowerならScalar 0。各indexはその整数のScalar。反復数はLimitsで制限。Vector/Matrix値の一般総和はSymbolic(UnsupportedExactDomain)であり、暗黙のscalar化はしない。
- fence/label: 数値意味は内部valueと等しい。annotationは数値評価しない。

integral、call、表示用text/sequence/subscript/superscript/scriptsは、評価専用規則がない限りSymbolic(NotationOnly)。これらの表示は完全に対応する。解析的な積分や任意関数評価を実装済みとしない。

## 4. MathML backend

MathML Coreの要素をtyped Markupで生成する。Number=mn、Symbol=mi、表示Text=mtext、加減乗・比較=mrow+mo、frac=mfrac、sqrt=msqrt、root=mroot、sub/sup/scripts=msub/msup/msubsup、vector/matrix=mtable/mtr/mtd、総和と積分=munder/munderoverまたは対応するscript形をdisplay modeから決定する。

暗黙のブラウザprecedence解釈へ依存しない。binding powerは比較10、add/sub20、mul30、neg40、pow50、atomic60。弱い子を強い親へ入れる際はmoによる可視括弧を挿入。subの右側、powの左側等、同じprecedenceでも非結合な位置に括弧を入れる。fracは分子分母の構造自体がgroupになる。

数学表示の2項演算は元の順序を保持する。mulのscalar/記号列でも、読み違いを避けるためreference backendは中央点を表示する。callはfunctionと括弧付きarguments。sequenceは指定順のmrowであり、勝手に演算を補わない。

letは「name := init ; body」のmrow。sumの下限は「index = lower」、上限はupper、bodyに必要な括弧を付ける。integralは積分記号と上下限、body、微分記号dとindex。equalは表示であって証明書ではない。

labelのDoc sentence annotationはsuiteがsafeなphrasing fragmentへ準備し、mtextを介した注記として出力する。MathMLの内容モデルに適合しないblock内容は受け入れない。

## 4.1. 公開arenaと原文保持

`interfaces/model.json` のMath record/unionはconstructorの論理的な意味展開であり、Rust enum順や別の再帰wire layoutではない。実値のschemaは `interfaces/math.json` の `MathSyntax` / `MathValue` とする。MathRootはExpr / Row / DocGuestの3種類。MathKindは29 formとNumber leafに対応し、bare SymbolNameは明示Symbolと同じ意味kindへlowerする。子はExprRef / RowRef / DocGuestRef、guestはEmbedRefで平坦なarenaを参照する。schemaの明示variant名とfield列がwire tagであり、入力由来の深さをnativeの再帰所有へ転写しない。

MathValueの構造検査はカテゴリ、参照、到達性、cycle、共有DAGの最大経路、Number有限十進制約、vector/matrix形状、fence幅、literal 0のroot degreeを検査する。単独Rowは空を表せるが、Matrixに取り込むrowの列数は正で全row同一でなければならない。この証明はsymbol解決済みCheckedExpressionや評価可能性の証明ではない。

MathSyntaxはsource宣言、元Origin表、tokenごとのowner headを持つMathView、SourceMapを所有する。Number.spellingは `Option<Span>` のまま保持し、存在する場合は宣言sourceとnode coverに整合する位置を指す。意味Rationalと原lexemeの一致を証明したときだけ元表記をprintへ利用でき、位置構造検査だけをその証明とみなさない。Symbol/Let/Sum/Integralの名前operandは閉じたMathFieldLocationで選択位置とOriginを保持する。本文の名前検索で位置を再発見せず、source-lessの位置はNoneとする。

LabelのDoc annotationと独立DocGuestは、Doc SentenceのForeignClosureを保持する。ownerの環境・Origin ID・source/map閉包とguest自身のID空間を混同せず、意味変換を行わない。元構文に意味的に不正なDoc annotationがあっても、Mathのsource構造検査を理由にDoc lowerや評価を呼び出してはならない。prepared表示へ渡す意味・内容モデル検査は別の要求として残す。

`lower::expression` はhostが選択済みparse/profileを確認したSyntaxBundleと明示Math表層SchemaRef/categoryを受け、現在のBudget/SourceAdmissionで再検査してMathSyntaxを返す。共有sourceは一度だけ計上し、原Frac・表示scripts等を簡約しない。局所constructor制約の失敗は元のsource NodeRefとShapeErrorへ帰属させ、破棄した出力arenaのindexだけを位置情報として返さない。停止は原StopReasonを保持し、元構文木を変更しない。

初回NDF受信はschema検査後に同じsource/Origin/View/guest閉包とarena制約を検査する。宣言sourceの欠落をreceiverのambient storeから補わない。raw MathSyntaxの受信はbinding・free symbol要求・評価結果のproofを発行しない。明示constructor helperは新しいsource-less式を作るためのもので、元式を置換する処理ではない。

## 5. 出力と資源

MathMLは独立した正式portable出力であり、ブラウザがfont/layoutを担当する。Doc・MathのHTML生成は[17章](17-math-html.md)のKaTeXPreferredを標準とし、生成環境でKaTeXを実行してCSS/fontと配布する。忠実変換不能・生成能力不足時はNEPL3 MathMLへ診断付きで切り替える。閲覧時にKaTeXを再実行せず、CLIがpixel描画まで行うとも広告しない。

独自layout/rasterizerを追加する場合は、CheckedExpressionまたはMathLayout入力modelを受ける別backendとする。OpenType MATH tableやglyph outlineの実装都合をMathの意味モデルへ持ち込まない。

## 6. 公開操作

lower、check、free_symbols、evaluate、print、render_mathml。評価結果、部分評価の新式、元の式を別値として返す。文書側のrender要求がevaluateを自動で要求しない。
