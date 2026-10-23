<!-- Generated from doc/progress/math&#45;runtime.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page math&#45;runtime; source SHA-256 5ce4c8ac75a563d630a08e35bb06ac914638d45099e4f7d7d90aa5951502e14c; alias input SHA-256 22530d23413550491608f92d2c85b0d77e8d39ed56866174501e7a48d80b87a9; page input SHA-256 712608e75ab3c1152c4741cea0d51a8a6054f19700994de1033ef6f9652b455f. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="math-runtime-の段階実装"></a>

# Math runtime の<ruby>段階実装<rt>だんかいじっそう</rt></ruby>

[正本（NEPL3d）](<math-runtime.nepld>)

<a name="n-737461747573"></a>

<a name="現在状態を確認する入口"></a>

## <ruby>現在状態<rt>げんざいじょうたい</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する<ruby>入口<rt>いりぐち</rt></ruby>

T08の<ruby>状態<rt>じょうたい</rt></ruby>と<ruby>検証記録<rt>けんしょうきろく</rt></ruby>は[<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>](<\.\.\/\.\.\/implementation\-status\.json>)、<ruby>到達条件<rt>とうたつじょうけん</rt></ruby>は[タスク<ruby>定義<rt>ていぎ</rt></ruby>](<\.\.\/\.\.\/design\/tasks\.json>)を<ruby>参照<rt>さんしょう</rt></ruby>する。<ruby>現在<rt>げんざい</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>は[<ruby>第<rt>だい</rt></ruby>06<ruby>章<rt>しょう</rt></ruby>](<\.\.\/spec\/06\-math\.md>)、Sentenceの<ruby>所有<rt>しょゆう</rt></ruby>と<ruby>接続<rt>せつぞく</rt></ruby>は[<ruby>第<rt>だい</rt></ruby>23<ruby>章<rt>しょう</rt></ruby>](<\.\.\/spec\/23\-sentence\-annotation\.md>)で<ruby>確認<rt>かくにん</rt></ruby>する。

<ruby>以下<rt>いか</rt></ruby>は<ruby>構造<rt>こうぞう</rt></ruby>・<ruby>数値<rt>すうち</rt></ruby>helper<ruby>導入時<rt>どうにゅうじ</rt></ruby>の<ruby>記録<rt>きろく</rt></ruby>である。Doc Sentenceの<ruby>旧接続<rt>きゅうせつぞく</rt></ruby>や「<ruby>残<rt>のこ</rt></ruby>り」の<ruby>記述<rt>きじゅつ</rt></ruby>は、その<ruby>段階<rt>だんかい</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>す。<ruby>現在<rt>げんざい</rt></ruby>のMath<ruby>印字<rt>いんじ</rt></ruby>・MathML<ruby>等<rt>とう</rt></ruby>の<ruby>実装判断<rt>じっそうはんだん</rt></ruby>には、<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>が<ruby>指<rt>さ</rt></ruby>す<ruby>検証記録<rt>けんしょうきろく</rt></ruby>を<ruby>用<rt>もち</rt></ruby>いる。

<a name="n-686973746f7279"></a>

<a name="構造数値helper導入時の記録"></a>

## <ruby>構造<rt>こうぞう</rt></ruby>・<ruby>数値<rt>すうち</rt></ruby>helper<ruby>導入時<rt>どうにゅうじ</rt></ruby>の<ruby>記録<rt>きろく</rt></ruby>

T08は<ruby>進行中<rt>しんこうちゅう</rt></ruby>。`doc/spec/06-math.md` と `design/forms.json` を<ruby>最終契約<rt>さいしゅうけいやく</rt></ruby>とし、<ruby>以下<rt>いか</rt></ruby>の<ruby>構造<rt>こうぞう</rt></ruby>・<ruby>数値<rt>すうち</rt></ruby>helperの<ruby>実行<rt>じっこう</rt></ruby>を<ruby>式<rt>しき</rt></ruby>evaluateやT08<ruby>全体<rt>ぜんたい</rt></ruby>の<ruby>完成<rt>かんせい</rt></ruby>へ<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えない。

`nepl3-math-core` はno\_std \+ alloc。workspace production<ruby>依存<rt>いぞん</rt></ruby>は<ruby>共通<rt>きょうつう</rt></ruby>coreだけで、Doc coreやengineへ<ruby>依存<rt>いぞん</rt></ruby>しない。<ruby>任意精度<rt>にんいせいど</rt></ruby>の<ruby>型付<rt>かたつ</rt></ruby>き<ruby>有理数<rt>ゆうりすう</rt></ruby>はcoreのInteger\/Rationalを<ruby>使<rt>つか</rt></ruby>い、Math<ruby>側<rt>がわ</rt></ruby>のnum\-bigint\/num\-traitsは<ruby>既定<rt>きてい</rt></ruby>featureを<ruby>無効<rt>むこう</rt></ruby>にした<ruby>明示外部依存<rt>めいじがいぶいぞん</rt></ruby>である。Math<ruby>固有<rt>こゆう</rt></ruby>のNumber<ruby>有限十進制約<rt>ゆうげんじっしんせいやく</rt></ruby>と<ruby>予算付<rt>よさんつ</rt></ruby>き<ruby>算術<rt>さんじゅつ</rt></ruby>helperをMath<ruby>側<rt>がわ</rt></ruby>に<ruby>持<rt>も</rt></ruby>つ。

MathValueは<ruby>全<rt>ぜん</rt></ruby>constructorの<ruby>平坦<rt>へいたん</rt></ruby>なarena、MathSyntaxは<ruby>元<rt>もと</rt></ruby>source\/Origin\/token\-local View\/SourceMapとForeignClosureを<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>構造<rt>こうぞう</rt></ruby>proofは<ruby>自由記号<rt>じゆうきごう</rt></ruby>の<ruby>解決<rt>かいけつ</rt></ruby>やCheckedExpressionの<ruby>証明<rt>しょうめい</rt></ruby>ではない。Number\(1\/3\)は<ruby>拒否<rt>きょひ</rt></ruby>するが、<ruby>評価用<rt>ひょうかよう</rt></ruby>の<ruby>一般<rt>いっぱん</rt></ruby>Rational\(1\/3\)は<ruby>合法<rt>ごうほう</rt></ruby>。`construct::expression_from_rational` は<ruby>有限十進<rt>ゆうげんじっしん</rt></ruby>をNumber、それ<ruby>以外<rt>いがい</rt></ruby>を<ruby>整数<rt>せいすう</rt></ruby>Numberを<ruby>子<rt>こ</rt></ruby>とするFracへ<ruby>明示構築<rt>めいじこうちく</rt></ruby>する。lowerはこのhelperで<ruby>著者<rt>ちょしゃ</rt></ruby>のFracを<ruby>折<rt>お</rt></ruby>り<ruby>畳<rt>たた</rt></ruby>まない。

`lower::expression` は<ruby>実<rt>じつ</rt></ruby>compilerが<ruby>生成<rt>せいせい</rt></ruby>したMath<ruby>表層<rt>ひょうそう</rt></ruby>schemaとchecked SyntaxBundleを<ruby>受<rt>う</rt></ruby>け、<ruby>同一<rt>どういつ</rt></ruby>Budget\/SourceAdmissionで<ruby>再検査<rt>さいけんさ</rt></ruby>・<ruby>変換<rt>へんかん</rt></ruby>する。<ruby>正式文法<rt>せいしきぶんぽう</rt></ruby>の29 formと2 leafを、process<ruby>不要<rt>ふよう</rt></ruby>の<ruby>保存<rt>ほぞん</rt></ruby>seedから<ruby>実<rt>じつ</rt></ruby>compile→ParseSession→lower→<ruby>初回<rt>しょかい</rt></ruby>CBOR<ruby>受信<rt>じゅしん</rt></ruby>へ<ruby>通<rt>とお</rt></ruby>す。Mathに<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>むDoc Sentenceは<ruby>実<rt>じつ</rt></ruby>Doc readerで<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>作<rt>つく</rt></ruby>り、<ruby>意味不正<rt>いみふせい</rt></ruby>な<ruby>注釈<rt>ちゅうしゃく</rt></ruby>もguest syntaxのまま<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>診断位置<rt>しんだんいち</rt></ruby>の<ruby>元<rt>もと</rt></ruby>になる<ruby>名前<rt>なまえ</rt></ruby>operandとconstructor NodeRefを<ruby>保持<rt>ほじ</rt></ruby>し、Spanを<ruby>本文検索<rt>ほんぶんけんさく</rt></ruby>で<ruby>作<rt>つく</rt></ruby>らない。

<ruby>管理対象<rt>かんりたいしょう</rt></ruby>の<ruby>数値<rt>すうち</rt></ruby>helperは1\/2\+1\/3\=5\/6、0\.1\+0\.2\=3\/10、128bitを<ruby>超<rt>こ</rt></ruby>える<ruby>整数<rt>せいすう</rt></ruby>、<ruby>除算<rt>じょざん</rt></ruby>ゼロと<ruby>先行予算停止<rt>せんこうよさんていし</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。これは<ruby>式<rt>しき</rt></ruby>のbinding、<ruby>左<rt>ひだり</rt></ruby>から<ruby>右<rt>みぎ</rt></ruby>の<ruby>評価順<rt>ひょうかじゅん</rt></ruby>、Exact\/Symbolic<ruby>分類<rt>ぶんるい</rt></ruby>や<ruby>部分評価<rt>ぶぶんひょうか</rt></ruby>の<ruby>完成<rt>かんせい</rt></ruby>ではない。<ruby>構造側<rt>こうぞうがわ</rt></ruby>はmatrix\/vector\/fence\/root degree、100k<ruby>深<rt>ふか</rt></ruby>いarenaのClone\/Eq\/drop、<ruby>共有最長<rt>きょうゆうさいちょう</rt></ruby>Depth、source<ruby>閉包不足<rt>へいほうぶそく</rt></ruby>、first\-receiver<ruby>停止<rt>ていし</rt></ruby>と<ruby>元入力不変<rt>もとにゅうりょくふへん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。

<ruby>残<rt>のこ</rt></ruby>りは<ruby>名前<rt>なまえ</rt></ruby>scopeとfree\_symbols、CheckedExpression、<ruby>式<rt>しき</rt></ruby>evaluateの<ruby>全<rt>ぜん</rt></ruby>exact\/symbolic<ruby>規則<rt>きそく</rt></ruby>と<ruby>部分結果<rt>ぶぶんけっか</rt></ruby>Report、Math<ruby>印字<rt>いんじ</rt></ruby>\/MathML backend、Doc annotationのsuite<ruby>準備<rt>じゅんび</rt></ruby>、<ruby>各操作<rt>かくそうさ</rt></ruby>の<ruby>完全<rt>かんぜん</rt></ruby>な<ruby>要求<rt>ようきゅう</rt></ruby>\/<ruby>返信包絡<rt>へんしんほうらく</rt></ruby>である。MathValue\/MathSyntaxの<ruby>値<rt>あたい</rt></ruby>schemaだけをこれらのoperation<ruby>実装済<rt>じっそうず</rt></ruby>みとしない。M01\/M02\/M03の<ruby>全<rt>ぜん</rt></ruby>target<ruby>達成<rt>たっせい</rt></ruby>も<ruby>未宣言<rt>みせんげん</rt></ruby>のまま<ruby>保持<rt>ほじ</rt></ruby>する。
