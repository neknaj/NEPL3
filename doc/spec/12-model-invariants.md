# 12. 意味モデル・中間表現の補足不変条件

## 方針

`interfaces/model.json` はsourceのform表と別の、lower後の意味値・実行IRの構造を記述する。surfaceとmeaningのSchemaRefは別にする。Rustで型を分け、wireで両者を混同しない。

## 1. 値と参照

JSON内の `record` は `[fieldName, typeExpression]` の順序付きarrayであり、その順序をNDF recordのfield順とする。`sum` はvariant名をkeyとするmapで、各payloadも同じ順序付きfield array。空record/payloadは空arrayとする。field名は各array内で一意。`union` はvariant名からrecord型へのmap。sum/unionおよびcontractsのvariantsのkey順には意味を持たせず、NDFではVariantNameの文字列で識別する。数値ordinalをJSON objectの列挙順から割り当てない。List/OptionはNDFの対応tag。Naturalはnonnegative Integer。Name/LanguageTagはTextに対して各domainの制約を追加する。

Source由来の意味値はOriginを関連付けられる。各Recordが共有できるようにnative実装をarenaにしてもよいが、公開値の意味をpointerに依存させない。wireは参照table付きbundleを使い、全NodeRef/OriginRefが有効であることを検査する。意味上の構造木と回路graphのcycle制約は異なる。

DocのSentenceLiteralはlower後にDoc:Sentenceとなる。raw Textには注釈構文を再適用しない。MathのNumberは有限十進で表現できるRational（約分後の分母の素因数が2と5だけ）と元の表記範囲を持ち、SymbolNameはMath:Symbolへ統合する。違反はNonFiniteDecimalNumber。Numeric spellingを表示に使う場合は、そのsnapshotと値が一致していることを検査する。生成Numberはcanonicalな整数または有限十進でprintする。任意有理数からの式構築と著者のFracの保存はMath章の規則に従う。

ForeignSyntaxはguestのopaque bundleを持ち、host coreはguestの意味型をimportしない。suiteで登録済みschemaに検査してからguest操作に渡す。Doc:DocGuestもForeignSyntaxを保持する。Codeの準備はbundleの安全性を検査してsourceとviewを表示し、guestのlower・意味check・evaluateを呼ばない。

## 2. Checked値の境界

Rust内部のChecked/Preparedのconstructorはprivateにする。wireでCheckedEnvelopeを受信しただけで検査済みと信用しない。外部providerからの結果にはschema検査・構造不変条件検査を行い、利用する操作に必要な意味検査を再実行する。digestは同一性の情報であって証明書ではない。

process/session内で検査結果を再利用する場合は、provider・input・schema・environmentを固定したhost所有handleを使える。別processへ生のpointerやprivateなhandleを送らない。

## 3. 回路IR

NetNode.idはnodesのindexに一致し、orderは組合せDAGの全nodeを一度ずつ含む順序。Input/State/Constantのinputsは空、Not/Sliceは1、And/Or/Xor/Nor/Add/Concatは2、Muxは3。InputとStateは有効なport/slotを参照する。Slice/Mux/Concatのwidth条件はCircuit章の通り。

outputNodesの長さはoutputsと等しく、各nodeの幅がportと一致する。StateSlot.nextは有効nodeで同じ幅。初期値は幅内。state次値へのedgeを現在state readの依存へ戻さない。

NorNetlistはbit順をport宣言順、その中をLSB→MSBとする。stateもslot順、その中をLSB→MSB。nextBits/outputBitsの長さは対応幅の和。Nor2の参照はtopologicalに前のnodeだけ。InputBit/StateBitは有効範囲。originsはnodesと同じ長さ。

## 4. Markup安全性

Markup modelがname/attributeをTextとして運べることは、任意のtagを許可することを意味しない。nepl3-markupは目的categoryに対して検査する。

HTMLの許可要素: article, section, div, p, span, h1〜h6, ruby, rt, rp, em, strong, br, pre, code, figure, figcaption, a。
MathML: math, mrow, mi, mn, mo, mtext, mfrac, msqrt, mroot, msub, msup, msubsup, munder, mover, munderover, mtable, mtr, mtd, mspace。
SVG: svg, g, rect, line, path, polyline, circle, text, title, desc。

全要素・属性・属性値制約の正本は `design/markup.json`。属性を受け入れる集合はglobal、namespace、elementの各allowlistの和とし、列挙されていない属性は拒否する。SVGのpath/points等は自由な文字列として受けず、記述した型付き構造からserializerが綴りを生成する。userから任意のon*、style、script、foreignObject、image、external href、任意のnamespace URLを受けない。hrefはarticle内の生成済みanchorだけ。CSSはbackendが所有する固定assetであり、本文文字列をCSSに埋め込まない。

MathMLのmspaceのwidth/height/depthはNonnegativeMathLengthとする。非負のcanonical有限十進にemを付け、zeroは0em、百分率・指数表記・他単位・負値・冗長なzeroを拒否する。これはMathML Coreのlength-percentageのうち本profileが使用する部分集合であり、SVGの座標用Decimalとは区別する。違反はInvalidMarkupAttribute。

MarkupのTextと属性値は、UTF-8の妥当性に加え、XML 1.0 Charの集合（U+0009、U+000A、U+000D、U+0020..D7FF、U+E000..FFFD、U+10000..10FFFF）を共通の許可集合とする。それ以外はInvalidMarkupCharacterで拒否し、削除や置換文字による黙殺をしない。Doc/Mathの一般Text値をこの出力用制約で狭めるのではなく、Markup構築・検査境界で適用する。

serializerはHTML5とXMLを区別し、namespace、void element、属性escapeを適切に出す。属性順は固定。Textと属性のescapeおよび改行保存は再現性章に従う。文書テキストを文字列置換でHTMLへ挿入しない。

## 5. operationの具体型

`interfaces/contracts.json` のDomainSyntax/CheckedDomain等はoperationごとに本ファイルの対応domain型へ特殊化するための表記。例えばmath.lowerの結果はMath/Expr、circuit.elaborateの結果はCircuit:PreparedNetlist、doc.prepareの結果はDoc:PreparedArticle。異なるdomainのTypedValueを同じ入力として受け付けない。

Grammar packageのReaderExpr/ReadSpec/Binding/StyleのpayloadはGrammarのschemaで定義したADTを使用できる。参照を解決した索引tableを追加してよいが、意味正規形は参照先の識別子と契約に従って比較する。無限再帰のRust型やSerde表現をwireへ押し付けない。

HTMLのdocument shell（html/head/meta/style/body）は固定のshell生成処理が作り、userが与えるMarkupFragmentの要素として受け取らない。内部のDoc/Math/SVG namespace遷移とphrasing/block制約もmarkup.jsonに従う。最適化や資源追加の都合でこのallowlistを迂回しない。
