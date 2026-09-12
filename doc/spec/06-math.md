<!-- Generated from doc/spec/06&#45;math.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page math; source SHA-256 850f15eda21d689bda96f5a93b8595e515f81ea41484636d21f6482aacb088c7; alias input SHA-256 cae137023719df65f4fdf356ff25c6fac5c3aaa15c90b6b91c82b313d5078aaf; document digest ad90e7eb0a99a8848b4b513ac6f3487cb30a048e1d71fbcbe788b544832d1390; input PageSet digest e183ffa10a6be3c7c3f1df322f8f76f73ee8e1380319d27de66706bc790fe5c0; input context SHA-256 01a71e3b6feac889d7a4b61f3b24e66b223a4b97f9c2ed41db3cbf4bbb46541d. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="06-math言語"></a>

# 06\. Math言語\[げんご\]

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

数式\[すうしき\]の表示対象\[ひょうじたいしょう\]を保\[たも\]ち、その一部\[いちぶ\]に対\[たい\]する厳密計算\[げんみつけいさん\]を独立\[どくりつ\]した操作\[そうさ\]として公開\[こうかい\]する。parse\/renderが数式\[すうしき\]を簡約\[かんやく\]したり、等式\[とうしき\]の正\[ただ\]しさを主張\[しゅちょう\]したりしない。

<a name="n-65787072657373696f6e"></a>

<a name="1-表現"></a>

## 1\. 表現\[ひょうげん\]

全\[ぜん\]constructorはmath\-signatures参照\[さんしょう\]。Numberは有限十進\[ゆうげんじっしん\]の原表記\[げんひょうき\]を持\[も\]ち、意味値\[いみち\]はBigRationalへ正確\[せいかく\]に変換\[へんかん\]する。整数\[せいすう\]p、分母\[ぶんぼ\]qはq\>0、gcd\(\|p\|\,q\)\=1、zero\=0\/1へ正規化\[せいきか\]する。Numberに格納\[かくのう\]できる値\[あたい\]は、約分後\[やくぶんご\]のqの素因数\[そいんすう\]が2と5だけの有理数\[ゆうりすう\]に限\[かぎ\]る。これ以外\[いがい\]をNumberとして構築\[こうちく\]・decodeする場合\[ばあい\]はNonFiniteDecimalNumberで拒否\[きょひ\]する。評価値\[ひょうかち\]のRationalはこの制限\[せいげん\]を持\[も\]たない。

任意有理数\[にんいゆうりすう\]から式\[しき\]を作\[つく\]るconstructor helper `expression_from_rational` は、有限十進\[ゆうげんじっしん\]ならNumber、それ以外\[いがい\]なら整数\[せいすう\]Numberを子\[こ\]とするFrac\(Number\(p\)\, Number\(q\)\)を返\[かえ\]す。元\[もと\]の構文\[こうぶん\]をlowerするときにこのhelperで著者\[ちょしゃ\]のFracを折\[お\]り畳\[たた\]まない。`frac 1 2` はFracのまま保持\[ほじ\]する。Numberのprintはcanonicalな整数\[せいすう\]または有限十進\[ゆうげんじっしん\]（指数表記\[しすうひょうき\]なし、冗長\[じょうちょう\]な末尾\[まつび\]zeroなし、zeroは0）で、意味値\[いみち\]とsnapshotの一致\[いっち\]を確認\[かくにん\]した場合\[ばあい\]には元\[もと\]lexemeを表示\[ひょうじ\]に利用\[りよう\]できる。Number\(1\/3\)をprint時\[じ\]だけFracへ変換\[へんかん\]する設計\[せっけい\]にはしない。

Identifier leafは数学記号\[すうがくきごう\]。自由記号\[じゆうきごう\]\{free symbol\}を許可\[きょか\]する。自由記号\[じゆうきごう\]はUnresolvedNameのエラーにしない。意味上\[いみじょう\]の入力要求\[にゅうりょくようきゅう\]として列挙\[れっきょ\]する。`symbol "..."` で予約語\[よやくご\]と同\[おな\]じ綴\[つづ\]りや複数文字\[ふくすうもじ\]の名前\[なまえ\]を明示\[めいじ\]できる。裸\[はだか\]のNameとsymbolの同\[おな\]じ綴\[つづ\]りは同\[おな\]じ名前解決規則\[なまえかいけつきそく\]を使\[つか\]う。

letはinitを外側\[そとがわ\]、bodyを新\[あたら\]しい記号\[きごう\]scopeで読\[よ\]む。sum\/integralのindexはbodyだけで有効\[ゆうこう\]であり、lower\/upperでは外側\[そとがわ\]を参照\[さんしょう\]する。自由記号\[じゆうきごう\]に架空\[かくう\]の定義位置\[ていぎいち\]を与\[あた\]えない。

subscript\/superscript\/scriptsは表示構造\[ひょうじこうぞう\]。一般\[いっぱん\]の添字\[そえじ\]を配列\[はいれつ\]アクセスや指数演算\[しすうえんざん\]に暗黙変換\[あんもくへんかん\]しない。代数的\[だいすうてき\]な累乗\[るいじょう\]にはpowを使\[つか\]う。`call`は数学\[すうがく\]の関数適用表現\[かんすうてきようひょうげん\]で、任意\[にんい\]のFnコード実行\[じっこう\]ではない。

<a name="n-737472756374757265"></a>

<a name="2-構造検査"></a>

## 2\. 構造検査\[こうぞうけんさ\]

matrixは一\[ひと\]つ以上\[いじょう\]のrow、各\[かく\]rowは同\[おな\]じ正\[せい\]の列数\[れつすう\]。vectorは一\[ひと\]つ以上\[いじょう\]の要素\[ようそ\]。fenceは左右\[さゆう\]それぞれ0または1Unicode scalar。空文字\[からもじ\]で片側\[かたがわ\]だけの括弧\[かっこ\]を表\[あらわ\]せる。

rootのdegreeが数値\[すうち\]literalで0ならInvalidRootDegree。その他\[ほか\]の定義域\[ていぎいき\]はevaluate時\[じ\]にも検査\[けんさ\]する。表示\[ひょうじ\]だけの式\[しき\]に実数\[じっすう\]\/複素数\[ふくそすう\]の数値領域\[すうちりょういき\]を勝手\[かって\]に割\[わ\]り当\[あ\]てない。

CheckedExpressionが保証\[ほしょう\]するのは構造\[こうぞう\]・binding・既知\[きち\]の局所制約\[きょくしょせいやく\]であり、全記号\[ぜんきごう\]の値\[あたい\]や全演算\[ぜんえんざん\]の数値評価可能性\[すうちひょうかかのうせい\]ではない。

<a name="n-6576616c75617465"></a>

## 3\. evaluate

入力\[にゅうりょく\]はCheckedExpression、自由記号\[じゆうきごう\]のBindingEnvironment、Limits。出力\[しゅつりょく\]はExact\(Value\)またはSymbolic\(expression\, Requirements\)。定義域違反\[ていぎいきいはん\]・形状不一致\[けいじょうふいっち\]は明確\[めいかく\]なEvalError。値\[あたい\]の種類\[しゅるい\]はScalar\(Q\)、`Vector(List<Q>)`、`Matrix(rows,cols,List<Q>)`、Truth\(Bool\)。

評価\[ひょうか\]はsource順\[じゅん\]の左\[ひだり\]から右\[みぎ\]、純粋\[じゅんすい\]。letはinitを評価\[ひょうか\]してからbodyを評価\[ひょうか\]する。未解決\[みかいけつ\]の記号\[きごう\]に依存\[いぞん\]する部分\[ぶぶん\]はSymbolicとし、独立\[どくりつ\]な数値\[すうち\]subtreeの計算結果\[けいさんけっか\]を保持\[ほじ\]できるが元\[もと\]の構文\[こうぶん\]を上書\[うわが\]きしない。

正確\[せいかく\]に評価\[ひょうか\]する演算\[えんざん\]\:

- add\/sub\: Scalar同士\[どうし\]、等\[ひと\]しい長\[なが\]さのVector、等\[ひと\]しい形状\[けいじょう\]のMatrix。
- neg\: Scalar\/Vector\/Matrixの全要素\[ぜんようそ\]の符号反転\[ふごうはんてん\]。
- mul\: Scalar×Scalar、ScalarとVector\/Matrixの両側\[りょうがわ\]、Matrix×Matrix、Matrix×Vector。Vector×Vectorは意味\[いみ\]が曖昧\[あいまい\]なのでOperandShapeMismatch。
- frac\: Scalar同士\[どうし\]、分母\[ぶんぼ\]0はDivisionByZero。
- pow\: Scalarの整数指数\[せいすうしすう\]。0\^0は空積\[くうせき\]として1。0の負指数\[ふしすう\]はDivisionByZero。非整数指数\[ひせいすうしすう\]はSymbolic\(NonIntegralExponent\)。
- sqrt\: 非負\[ひふ\]Scalarの分子分母\[ぶんしぶんぼ\]がともに完全平方\[かんぜんへいほう\]ならExact。非平方\[ひへいほう\]はSymbolic\(AlgebraicValueRequired\)、負\[ふ\]ならSymbolic\(ComplexValueRequired\)。
- root\: 正\[せい\]の整数次数\[せいすうじすう\]だけExact候補\[こうほ\]。奇数次数\[きすうじすう\]の負数\[ふすう\]を許\[ゆる\]し、分子分母\[ぶんしぶんぼ\]が完全\[かんぜん\]n乗\[じょう\]ならExact。次数\[じすう\]が不正\[ふせい\]ならInvalidRootDegree、非完全冪\[ひかんぜんべき\]ならSymbolic。
- equal\: 同\[おな\]じ種類\[しゅるい\]・形状\[けいじょう\]のExact値\[ち\]の等値\[とうち\]。Scalar\/配列\[はいれつ\]の比較\[ひかく\]を混同\[こんどう\]しない。
- lt\/le\: Scalar同士\[どうし\]だけ。
- transpose\: Matrixの転置\[てんち\]。Vectorは1×nのMatrixにする。
- det\: 正方\[せいほう\]Matrixだけ。正確\[せいかく\]な有理数\[ゆうりすう\]の消去法\[しょうきょほう\]を用\[もち\]い、pivotは最初\[さいしょ\]の非\[ひ\]zero行\[ぎょう\]。形状不正\[けいじょうふせい\]はNotSquare。
- sum\: lower\/upperが整数\[せいすう\]で有限\[ゆうげん\]、bodyがScalarとなる場合\[ばあい\]をexact domainとする。inclusive区間\[くかん\]。upper\<lowerならScalar 0。各\[かく\]indexはその整数\[せいすう\]のScalar。反復数\[はんぷくすう\]はLimitsで制限\[せいげん\]。Vector\/Matrix値\[ち\]の一般総和\[いっぱんそうわ\]はSymbolic\(UnsupportedExactDomain\)であり、暗黙\[あんもく\]のscalar化\[か\]はしない。
- fence\/label\: 数値意味\[すうちいみ\]は内部\[ないぶ\]valueと等\[ひと\]しい。annotationは数値評価\[すうちひょうか\]しない。

integral、call、表示用\[ひょうじよう\]text\/sequence\/subscript\/superscript\/scriptsは、評価専用規則\[ひょうかせんようきそく\]がない限\[かぎ\]りSymbolic\(NotationOnly\)。これらの表示\[ひょうじ\]は完全\[かんぜん\]に対応\[たいおう\]する。解析的\[かいせきてき\]な積分\[せきぶん\]や任意関数評価\[にんいかんすうひょうか\]を実装済\[じっそうず\]みとしない。

<a name="n-6d6174686d6c"></a>

## 4\. MathML backend

MathML Coreの要素\[ようそ\]をtyped Markupで生成\[せいせい\]する。Number\=mn、Symbol\=mi、表示\[ひょうじ\]Text\=mtext、加減乗\[かげんじょう\]・比較\[ひかく\]\=mrow\+mo、frac\=mfrac、sqrt\=msqrt、root\=mroot、sub\/sup\/scripts\=msub\/msup\/msubsup、vector\/matrix\=mtable\/mtr\/mtd、総和\[そうわ\]と積分\[せきぶん\]\=munder\/munderoverまたは対応\[たいおう\]するscript形\[けい\]をdisplay modeから決定\[けってい\]する。

暗黙\[あんもく\]のブラウザprecedence解釈\[かいしゃく\]へ依存\[いぞん\]しない。binding powerは比較\[ひかく\]10、add\/sub20、mul30、neg40、pow50、atomic60。弱\[よわ\]い子\[こ\]を強\[つよ\]い親\[おや\]へ入\[い\]れる際\[さい\]はmoによる可視括弧\[かしかっこ\]を挿入\[そうにゅう\]。subの右側\[みぎがわ\]、powの左側等\[ひだりがわなど\]、同\[おな\]じprecedenceでも非結合\[ひけつごう\]な位置\[いち\]に括弧\[かっこ\]を入\[い\]れる。fracは分子分母\[ぶんしぶんぼ\]の構造自体\[こうぞうじたい\]がgroupになる。

数学表示\[すうがくひょうじ\]の2項演算\[こうえんざん\]は元\[もと\]の順序\[じゅんじょ\]を保持\[ほじ\]する。mulのscalar\/記号列\[きごうれつ\]でも、読\[よ\]み違\[ちが\]いを避\[さ\]けるためreference backendは中央点\[ちゅうおうてん\]を表示\[ひょうじ\]する。callはfunctionと括弧付\[かっこつ\]きarguments。sequenceは指定順\[していじゅん\]のmrowであり、勝手\[かって\]に演算\[えんざん\]を補\[おぎな\]わない。

letは「name \:\= init \; body」のmrow。sumの下限\[かげん\]は「index \= lower」、上限\[じょうげん\]はupper、bodyに必要\[ひつよう\]な括弧\[かっこ\]を付\[つ\]ける。integralは積分記号\[せきぶんきごう\]と上下限\[じょうかげん\]、body、微分記号\[びぶんきごう\]dとindex。equalは表示\[ひょうじ\]であって証明書\[しょうめいしょ\]ではない。

labelのDoc sentence annotationはsuiteがsafeなphrasing fragmentへ準備\[じゅんび\]し、mtextを介\[かい\]した注記\[ちゅうき\]として出力\[しゅつりょく\]する。MathMLの内容\[ないよう\]モデルに適合\[てきごう\]しないblock内容\[ないよう\]は受\[う\]け入\[い\]れない。

<a name="n-6172656e61"></a>

<a name="41-公開arenaと原文保持"></a>

## 4\.1\. 公開\[こうかい\]arenaと原文保持\[げんぶんほじ\]

`interfaces/model.json` のMath record\/unionはconstructorの論理的\[ろんりてき\]な意味展開\[いみてんかい\]であり、Rust enum順\[じゅん\]や別\[べつ\]の再帰\[さいき\]wire layoutではない。実値\[じつち\]のschemaは `interfaces/math.json` の `MathSyntax` \/ `MathValue` とする。MathRootはExpr \/ Row \/ DocGuestの3種類\[しゅるい\]。MathKindは29 formとNumber leafに対応\[たいおう\]し、bare SymbolNameは明示\[めいじ\]Symbolと同\[おな\]じ意味\[いみ\]kindへlowerする。子\[こ\]はExprRef \/ RowRef \/ DocGuestRef、guestはEmbedRefで平坦\[へいたん\]なarenaを参照\[さんしょう\]する。schemaの明示\[めいじ\]variant名\[めい\]とfield列\[れつ\]がwire tagであり、入力由来\[にゅうりょくゆらい\]の深\[ふか\]さをnativeの再帰所有\[さいきしょゆう\]へ転写\[てんしゃ\]しない。

MathValueの構造検査\[こうぞうけんさ\]はカテゴリ、参照\[さんしょう\]、到達性\[とうたつせい\]、cycle、共有\[きょうゆう\]DAGの最大経路\[さいだいけいろ\]、Number有限十進制約\[ゆうげんじっしんせいやく\]、vector\/matrix形状\[けいじょう\]、fence幅\[はば\]、literal 0のroot degreeを検査\[けんさ\]する。単独\[たんどく\]Rowは空\[から\]を表\[あらわ\]せるが、Matrixに取\[と\]り込\[こ\]むrowの列数\[れつすう\]は正\[せい\]で全\[ぜん\]row同一\[どういつ\]でなければならない。この証明\[しょうめい\]はsymbol解決済\[かいけつず\]みCheckedExpressionや評価可能性\[ひょうかかのうせい\]の証明\[しょうめい\]ではない。

MathSyntaxはsource宣言\[せんげん\]、元\[もと\]Origin表\[ひょう\]、tokenごとのowner headを持\[も\]つMathView、SourceMapを所有\[しょゆう\]する。Number\.spellingは `Option<Span>` のまま保持\[ほじ\]し、存在\[そんざい\]する場合\[ばあい\]は宣言\[せんげん\]sourceとnode coverに整合\[せいごう\]する位置\[いち\]を指\[さ\]す。意味\[いみ\]Rationalと原\[げん\]lexemeの一致\[いっち\]を証明\[しょうめい\]したときだけ元表記\[もとひょうき\]をprintへ利用\[りよう\]でき、位置構造検査\[いちこうぞうけんさ\]だけをその証明\[しょうめい\]とみなさない。Symbol\/Let\/Sum\/Integralの名前\[なまえ\]operandは閉\[と\]じたMathFieldLocationで選択位置\[せんたくいち\]とOriginを保持\[ほじ\]する。本文\[ほんぶん\]の名前検索\[なまえけんさく\]で位置\[いち\]を再発見\[さいはっけん\]せず、source\-lessの位置\[いち\]はNoneとする。

LabelのDoc annotationと独立\[どくりつ\]DocGuestは、Doc SentenceのForeignClosureを保持\[ほじ\]する。ownerの環境\[かんきょう\]・Origin ID・source\/map閉包\[へいほう\]とguest自身\[じしん\]のID空間\[くうかん\]を混同\[こんどう\]せず、意味変換\[いみへんかん\]を行\[おこな\]わない。元構文\[もとこうぶん\]に意味的\[いみてき\]に不正\[ふせい\]なDoc annotationがあっても、Mathのsource構造検査\[こうぞうけんさ\]を理由\[りゆう\]にDoc lowerや評価\[ひょうか\]を呼\[よ\]び出\[だ\]してはならない。prepared表示\[ひょうじ\]へ渡\[わた\]す意味\[いみ\]・内容\[ないよう\]モデル検査\[けんさ\]は別\[べつ\]の要求\[ようきゅう\]として残\[のこ\]す。

`lower::expression` はhostが選択済\[せんたくず\]みparse\/profileを確認\[かくにん\]したSyntaxBundleと明示\[めいじ\]Math表層\[ひょうそう\]SchemaRef\/categoryを受\[う\]け、現在\[げんざい\]のBudget\/SourceAdmissionで再検査\[さいけんさ\]してMathSyntaxを返\[かえ\]す。共有\[きょうゆう\]sourceは一度\[いちど\]だけ計上\[けいじょう\]し、原\[げん\]Frac・表示\[ひょうじ\]scripts等\[など\]を簡約\[かんやく\]しない。局所\[きょくしょ\]constructor制約\[せいやく\]の失敗\[しっぱい\]は元\[もと\]のsource NodeRefとShapeErrorへ帰属\[きぞく\]させ、破棄\[はき\]した出力\[しゅつりょく\]arenaのindexだけを位置情報\[いちじょうほう\]として返\[かえ\]さない。停止\[ていし\]は原\[げん\]StopReasonを保持\[ほじ\]し、元構文木\[もとこうぶんき\]を変更\[へんこう\]しない。

初回\[しょかい\]NDF受信\[じゅしん\]はschema検査後\[けんさご\]に同\[おな\]じsource\/Origin\/View\/guest閉包\[へいほう\]とarena制約\[せいやく\]を検査\[けんさ\]する。宣言\[せんげん\]sourceの欠落\[けつらく\]をreceiverのambient storeから補\[おぎな\]わない。raw MathSyntaxの受信\[じゅしん\]はbinding・free symbol要求\[ようきゅう\]・評価結果\[ひょうかけっか\]のproofを発行\[はっこう\]しない。明示\[めいじ\]constructor helperは新\[あたら\]しいsource\-less式\[しき\]を作\[つく\]るためのもので、元式\[もとしき\]を置換\[ちかん\]する処理\[しょり\]ではない。

<a name="n-7265736f7572636573"></a>

<a name="5-出力と資源"></a>

## 5\. 出力\[しゅつりょく\]と資源\[しげん\]

MathMLは独立\[どくりつ\]した正式\[せいしき\]portable出力\[しゅつりょく\]であり、ブラウザがfont\/layoutを担当\[たんとう\]する。Doc・MathのHTML生成\[せいせい\]は[17章\[しょう\]](<17\-math\-html\.md>)のKaTeXPreferredを標準\[ひょうじゅん\]とし、生成環境\[せいせいかんきょう\]でKaTeXを実行\[じっこう\]してCSS\/fontと配布\[はいふ\]する。忠実変換不能\[ちゅうじつへんかんふのう\]・生成能力不足時\[せいせいのうりょくぶそくじ\]はNEPL3 MathMLへ診断付\[しんだんつ\]きで切\[き\]り替\[か\]える。閲覧時\[えつらんじ\]にKaTeXを再実行\[さいじっこう\]せず、CLIがpixel描画\[びょうが\]まで行\[おこな\]うとも広告\[こうこく\]しない。

独自\[どくじ\]layout\/rasterizerを追加\[ついか\]する場合\[ばあい\]は、CheckedExpressionまたはMathLayout入力\[にゅうりょく\]modelを受\[う\]ける別\[べつ\]backendとする。OpenType MATH tableやglyph outlineの実装都合\[じっそうつごう\]をMathの意味\[いみ\]モデルへ持\[も\]ち込\[こ\]まない。

<a name="n-62696e64696e6773"></a>

## 6\. 束縛解析\[そくばくかいせき\]の交換値\[こうかんち\]

MathBindingsは構造検査後\[こうぞうけんさご\]のMathValueに対\[たい\]する束縛解析\[そくばくかいせき\]であり、CheckedExpression全体\[ぜんたい\]の証明\[しょうめい\]ではない。MathBindingは束縛\[そくばく\]するformのoccurrenceとnodeを、MathSymbolUseは記号\[きごう\]のoccurrence・node・bindingを保持\[ほじ\]する。bindingがNoneなら自由記号\[じゆうきごう\]であり、Someなら最\[もっと\]も近\[ちか\]い束縛\[そくばく\]formのoccurrenceを指\[さ\]す。

occurrenceはrootを0として、意味\[いみ\]field順\[じゅん\]に子\[こ\]をたどる先行順\[せんこうじゅん\]の番号\[ばんごう\]である。共有\[きょうゆう\]nodeも経路\[けいろ\]ごとに別\[べつ\]のoccurrenceを持\[も\]ち、RowとDocGuestにも番号\[ばんごう\]を割\[わ\]り当\[あ\]てる。DocGuestの内部\[ないぶ\]は走査\[そうさ\]せず、名前\[なまえ\]は文字列\[もじれつ\]の完全一致\[かんぜんいっち\]で比較\[ひかく\]する。

definitionsとusesはそれぞれoccurrence順\[じゅん\]である。sourceとOriginは入力\[にゅうりょく\]nodeのfield locationを参照\[さんしょう\]し、自由記号\[じゆうきごう\]に定義位置\[ていぎいち\]を作\[つく\]らない。NDF境界\[きょうかい\]では入力\[にゅうりょく\]を指定\[してい\]して再解析\[さいかいせき\]し、参照先\[さんしょうさき\]・出現順\[しゅつげんじゅん\]・過不足\[かふそく\]の不一致\[ふいっち\]をBindingMismatchとして拒否\[きょひ\]する。全出現\[ぜんしゅつげん\]と名前比較\[なまえひかく\]に共通予算\[きょうつうよさん\]を適用\[てきよう\]し、停止\[ていし\]した結果\[けっか\]を部分成功\[ぶぶんせいこう\]として返\[かえ\]さない。

<a name="n-66726565696e70757473"></a>

## 6\.1\. 自由記号\[じゆうきごう\]の入力要求\[にゅうりょくようきゅう\]

free\_symbolsはCheckedExpressionからMathFreeSymbolsを生成\[せいせい\]する。symbolsの各要素\[かくようそ\]MathFreeSymbolはnameとoccurrencesを持\[も\]ち、束縛\[そくばく\]されていない出現\[しゅつげん\]だけを同\[おな\]じ名前\[なまえ\]にまとめる。nameはUTF\-8の辞書順\[じしょじゅん\]、occurrencesは先行順\[せんこうじゅん\]とし、重複\[じゅうふく\]・欠落\[けつらく\]を認\[みと\]めない。

名前\[なまえ\]は完全一致\[かんぜんいっち\]で比較\[ひかく\]し、大文字\[おおもじ\]と小文字\[こもじ\]の同一視\[どういつし\]やUnicode正規化\[せいきか\]は行\[おこな\]わない。定義位置\[ていぎいち\]や評価値\[ひょうかち\]を補\[おぎな\]わず、guestの内部\[ないぶ\]も解析\[かいせき\]しない。NDFの受信\[じゅしん\]では指定\[してい\]された入力\[にゅうりょく\]から再計算\[さいけいさん\]し、不一致\[ふいっち\]をFreeSymbolsMismatchとして拒否\[きょひ\]する。共通予算\[きょうつうよさん\]の停止\[ていし\]は部分成功\[ぶぶんせいこう\]へ変\[か\]えない。

<a name="n-7075626c6963"></a>

<a name="6-公開操作"></a>

## 7\. 公開操作\[こうかいそうさ\]

lower、check、free\_symbols、evaluate、print、render\_mathml。評価結果\[ひょうかけっか\]、部分評価\[ぶぶんひょうか\]の新式\[しんしき\]、元\[もと\]の式\[しき\]を別値\[べつち\]として返\[かえ\]す。文書側\[ぶんしょがわ\]のrender要求\[ようきゅう\]がevaluateを自動\[じどう\]で要求\[ようきゅう\]しない。
