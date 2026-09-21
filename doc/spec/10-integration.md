<!-- Generated from doc/spec/10&#45;integration.nepld; renderer nepl3-tools.markdown-annotated-pages/2; page integration; source SHA-256 9161d6faee7ab61be38132df12ebd4e83578dc376e79f213aff172174bcfc107; alias input SHA-256 00e7d3fbbc79af03ae9a66bba6cc0bcd47165fb3eb087ea15a633624a51e1309; document digest 086c0f6d22c3b4d7207523177ec002a075fd7ef916395b4407e1ff346ce9f734; input PageSet digest 06af63f0629d28f3f3cd380a0752cc882b4d35375ddc9f7066beec70c8a60944; input context SHA-256 ed700e2f76ece9ddda9c24308c5cfe29bfa6379c4cfbb30fc5683c91654ccd7a. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="10-profile埋め込み実行入口"></a>

# 10\. Profile・<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>み・<ruby>実行入口<rt>じっこういりぐち</rt></ruby>

[正本（NEPL3d）](<10-integration.nepld>)

この<ruby>章<rt>しょう</rt></ruby>はsuite・bridge・CLIの<ruby>目標契約<rt>もくひょうけいやく</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>む。<ruby>列挙<rt>れっきょ</rt></ruby>した<ruby>全操作<rt>ぜんそうさ</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>・<ruby>全<rt>ぜん</rt></ruby>targetでの<ruby>受入完了<rt>うけいれかんりょう</rt></ruby>を<ruby>宣言<rt>せんげん</rt></ruby>するものではない。<ruby>現在<rt>げんざい</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>と<ruby>受入<rt>うけいれ</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>はimplementation\-status\.jsonで<ruby>管理<rt>かんり</rt></ruby>する。

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## <ruby>方針<rt>ほうしん</rt></ruby>

ソース<ruby>上<rt>じょう</rt></ruby>の<ruby>相互埋<rt>そうごう</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みとcrate<ruby>依存<rt>いぞん</rt></ruby>を<ruby>分離<rt>ぶんり</rt></ruby>する。suiteが<ruby>登録済<rt>とうろくず</rt></ruby>みlanguage package、domain operation、output adapterを<ruby>接続<rt>せつぞく</rt></ruby>する。

<a name="n-70726f66696c65"></a>

<a name="1-profile"></a>

## 1\. Profile

<ruby>不変<rt>ふへん</rt></ruby>のParseProfileと、<ruby>構文位置<rt>こうぶんいち</rt></ruby>ごとのcontextを<ruby>区別<rt>くべつ</rt></ruby>する。<ruby>先行<rt>せんこう</rt></ruby>する<ruby>構文<rt>こうぶん</rt></ruby>で<ruby>後続<rt>こうぞく</rt></ruby>contextを<ruby>更新<rt>こうしん</rt></ruby>できることは、Profileを<ruby>不変<rt>ふへん</rt></ruby>とする<ruby>契約<rt>けいやく</rt></ruby>と<ruby>両立<rt>りょうりつ</rt></ruby>する。<ruby>各<rt>かく</rt></ruby>headのshapeはその<ruby>出現<rt>しゅつげん</rt></ruby>までに<ruby>確定<rt>かくてい</rt></ruby>した<ruby>情報<rt>じょうほう</rt></ruby>から<ruby>決<rt>き</rt></ruby>め、<ruby>後続<rt>こうぞく</rt></ruby>sourceから<ruby>遡及<rt>そきゅう</rt></ruby>して<ruby>変更<rt>へんこう</rt></ruby>しない。

Profileはlanguage alias→SchemaRef、category mode、provider allowlist、operation bridge、resource snapshot、Limitsを<ruby>持<rt>も</rt></ruby>つ<ruby>不変値<rt>ふへんち</rt></ruby>。root languageはファイル<ruby>拡張子<rt>かくちょうし</rt></ruby>またはCLI<ruby>引数<rt>ひきすう</rt></ruby>で<ruby>選<rt>えら</rt></ruby>び、<ruby>全<rt>ぜん</rt></ruby>ソースを<ruby>一律<rt>いちりつ</rt></ruby>のlexerで<ruby>先<rt>さき</rt></ruby>にtoken<ruby>化<rt>か</rt></ruby>しない。

`design/profile.json` はsource manifestであり、<ruby>解決済<rt>かいけつず</rt></ruby>みruntime Profileではない。R009の<ruby>解消<rt>かいしょう</rt></ruby>では<ruby>生成結果<rt>せいせいけっか</rt></ruby>の<ruby>閉<rt>と</rt></ruby>じた<ruby>型<rt>かた</rt></ruby>、<ruby>各<rt>かく</rt></ruby>schema\/package\/provider digest、<ruby>許可<rt>きょか</rt></ruby>capability、resource identityと<ruby>整合検査<rt>せいごうけんさ</rt></ruby>を<ruby>先<rt>さき</rt></ruby>に<ruby>定<rt>さだ</rt></ruby>める。T05\/T11で<ruby>実際<rt>じっさい</rt></ruby>の<ruby>検査済<rt>けんさず</rt></ruby>みpackageから<ruby>生成<rt>せいせい</rt></ruby>・<ruby>差分検査<rt>さぶんけんさ</rt></ruby>し、UI\/Workerはこの<ruby>値<rt>あたい</rt></ruby>を<ruby>利用<rt>りよう</rt></ruby>する。R006の<ruby>操作<rt>そうさ</rt></ruby>・bundle<ruby>型<rt>がた</rt></ruby>の<ruby>未定義<rt>みていぎ</rt></ruby>を<ruby>文字列<rt>もじれつ</rt></ruby>signatureや<ruby>仮<rt>かり</rt></ruby>digestで<ruby>補<rt>おぎな</rt></ruby>わない。

<ruby>解析<rt>かいせき</rt></ruby>の<ruby>実入口<rt>じついりぐち</rt></ruby>はengineの`ParseProfile`とし、suite Profileから<ruby>渡<rt>わた</rt></ruby>す<ruby>不変<rt>ふへん</rt></ruby>なprojectionとして<ruby>扱<rt>あつか</rt></ruby>う。`interfaces/engine.json`にlanguage<ruby>登録<rt>とうろく</rt></ruby>、category mode、<ruby>選択<rt>せんたく</rt></ruby>schema、provider<ruby>要件<rt>ようけん</rt></ruby>、operation allowlist、resource identity、Limitsの<ruby>型<rt>かた</rt></ruby>を<ruby>置<rt>お</rt></ruby>く。`resolve`は<ruby>独立<rt>どくりつ</rt></ruby>したhost RuntimeCatalogとfinalize<ruby>済<rt>ず</rt></ruby>みregistryを<ruby>使<rt>つか</rt></ruby>い、<ruby>実<rt>じつ</rt></ruby>package<ruby>意味<rt>いみ</rt></ruby>digest、schema<ruby>参照閉包<rt>さんしょうへいほう</rt></ruby>、guest category\/mode、host provider<ruby>登録<rt>とうろく</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>identity、resourceの<ruby>実<rt>じつ</rt></ruby>byte<ruby>列<rt>れつ</rt></ruby>digestを<ruby>照合<rt>しょうごう</rt></ruby>する。<ruby>外部<rt>がいぶ</rt></ruby>Profileの<ruby>自己申告<rt>じこしんこく</rt></ruby>をそのままhost<ruby>登録<rt>とうろく</rt></ruby>へ<ruby>複写<rt>ふくしゃ</rt></ruby>して<ruby>検査済<rt>けんさず</rt></ruby>みとしない。

provider identityはhostが<ruby>実<rt>じつ</rt></ruby>assetまたは<ruby>版管理<rt>ばんかんり</rt></ruby>された<ruby>実装<rt>じっそう</rt></ruby>manifestから<ruby>確定<rt>かくてい</rt></ruby>する。coreはbinaryを<ruby>読<rt>よ</rt></ruby>み<ruby>込<rt>こ</rt></ruby>まず、binary<ruby>自身<rt>じしん</rt></ruby>のhashを<ruby>同<rt>おな</rt></ruby>じbinary<ruby>内<rt>ない</rt></ruby>の<ruby>定数<rt>ていすう</rt></ruby>へ<ruby>埋<rt>う</rt></ruby>める<ruby>自己<rt>じこ</rt></ruby>hash<ruby>循環<rt>じゅんかん</rt></ruby>も<ruby>要求<rt>ようきゅう</rt></ruby>しない。operationのschema<ruby>署名<rt>しょめい</rt></ruby>が<ruby>既知<rt>きち</rt></ruby>でもcallbackが<ruby>登録<rt>とうろく</rt></ruby>されたことにはならない。parseに<ruby>使<rt>つか</rt></ruby>うreader providerは<ruby>許可済<rt>きょかず</rt></ruby>み<ruby>実登録<rt>じつとうろく</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>し、<ruby>未実行<rt>みじっこう</rt></ruby>のfacts<ruby>等<rt>とう</rt></ruby>のextensionは<ruby>署名<rt>しょめい</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>するが<ruby>自動実行<rt>じどうじっこう</rt></ruby>しない。

<ruby>解析<rt>かいせき</rt></ruby>Profileのidentityは`NEPL3-PARSE-PROFILE-1`、zero byte、canonical JSONのSHA\-256。id、language<ruby>登録<rt>とうろく</rt></ruby>、schema、category\-mode<ruby>指定<rt>してい</rt></ruby>、providerの<ruby>実装<rt>じっそう</rt></ruby>identity、allowlist、resource identity、Limitsを<ruby>含<rt>ふく</rt></ruby>める。<ruby>宣言<rt>せんげん</rt></ruby>はalias\/idまたは<ruby>完全<rt>かんぜん</rt></ruby>OperationRef\/SchemaRef<ruby>順<rt>じゅん</rt></ruby>、Limits<ruby>列<rt>れつ</rt></ruby>はsourceBytes\/work\/depth\/nodes\/allocationUnits\/outputBytes\/diagnostics\/events<ruby>順<rt>じゅん</rt></ruby>とする。<ruby>実<rt>じつ</rt></ruby>resource bytesはhash<ruby>照合<rt>しょうごう</rt></ruby>し、Profileへ<ruby>全文複製<rt>ぜんぶんふくせい</rt></ruby>しない。

HeadProviderの<ruby>登録<rt>とうろく</rt></ruby>はheadProvidersのHeadRegistration\(alias\,category\,provider\)で<ruby>選<rt>えら</rt></ruby>ぶ。<ruby>同<rt>おな</rt></ruby>じpackageを<ruby>登録<rt>とうろく</rt></ruby>した<ruby>別<rt>べつ</rt></ruby>aliasの<ruby>設定<rt>せってい</rt></ruby>を<ruby>共有<rt>きょうゆう</rt></ruby>したと<ruby>推定<rt>すいてい</rt></ruby>しない。\(alias\,category\)<ruby>重複<rt>ちょうふく</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>し、shape\/childContextの<ruby>両<rt>りょう</rt></ruby>OperationRefについて<ruby>純粋<rt>じゅんすい</rt></ruby>なHeadCall→HeadReply<ruby>署名<rt>しょめい</rt></ruby>、allowlist、<ruby>独立<rt>どくりつ</rt></ruby>host catalogの<ruby>実装<rt>じっそう</rt></ruby>identityを<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>標準操作名<rt>ひょうじゅんそうさめい</rt></ruby>はheadShape\/headChildContextだが、<ruby>登録済<rt>とうろくず</rt></ruby>みの<ruby>同署名操作<rt>どうしょめいそうさ</rt></ruby>も<ruby>選択<rt>せんたく</rt></ruby>できる。Profile identityにはheadProviders keyを<ruby>含<rt>ふく</rt></ruby>め、alias\/category<ruby>順<rt>じゅん</rt></ruby>の `[alias,category,shapeOperation,childContextOperation]` <ruby>列<rt>れつ</rt></ruby>で<ruby>記述<rt>きじゅつ</rt></ruby>する。<ruby>列挙順<rt>れっきょじゅん</rt></ruby>だけの<ruby>変更<rt>へんこう</rt></ruby>はidentityを<ruby>変<rt>か</rt></ruby>えず、<ruby>操作<rt>そうさ</rt></ruby>の<ruby>役割<rt>やくわり</rt></ruby>・alias\/categoryへの<ruby>割当変更<rt>わりあてへんこう</rt></ruby>は<ruby>変<rt>か</rt></ruby>える。

ここでのallowlistはoperation<ruby>呼出可否<rt>よびだしかひ</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す。providerのtransport、<ruby>隔離<rt>かくり</rt></ruby>、ネットワーク<ruby>等<rt>とう</rt></ruby>の<ruby>権限<rt>けんげん</rt></ruby>、<ruby>強制停止方法<rt>きょうせいていしほうほう</rt></ruby>、bridge、EnvironmentProjectionを<ruby>含<rt>ふく</rt></ruby>むfull suite Profileの<ruby>契約<rt>けいやく</rt></ruby>は<ruby>引<rt>ひ</rt></ruby>き<ruby>続<rt>つづ</rt></ruby>き<ruby>実装対象<rt>じっそうたいしょう</rt></ruby>である。この<ruby>解析<rt>かいせき</rt></ruby>projectionだけではR009<ruby>全体<rt>ぜんたい</rt></ruby>を<ruby>完了<rt>かんりょう</rt></ruby>しない。hostは<ruby>実行環境<rt>じっこうかんきょう</rt></ruby>のcapabilityを<ruby>別途検査<rt>べっとけんさ</rt></ruby>し、<ruby>解析<rt>かいせき</rt></ruby>projectionはその<ruby>承認<rt>しょうにん</rt></ruby>を<ruby>代行<rt>だいこう</rt></ruby>しない。

<ruby>配布拡張子<rt>はいふかくちょうし</rt></ruby>は `.neplg`、`.nepld`、`.neplm`、`.neplc`。<ruby>汎用<rt>はんよう</rt></ruby> `.nepl` ではlanguage<ruby>指定<rt>してい</rt></ruby>を<ruby>必須<rt>ひっす</rt></ruby>にする。<ruby>既存<rt>きそん</rt></ruby>NCGやGlossのファイルを<ruby>新言語<rt>しんげんご</rt></ruby>として<ruby>黙<rt>だま</rt></ruby>って<ruby>解釈<rt>かいしゃく</rt></ruby>しない。

ParseProfile\.limitsは<ruby>解析操作<rt>かいせきそうさ</rt></ruby>に<ruby>対<rt>たい</rt></ruby>するresource<ruby>別上限<rt>べつじょうげん</rt></ruby>であり、<ruby>単<rt>たん</rt></ruby>なる<ruby>既定値<rt>きていち</rt></ruby>ではない。<ruby>操作開始時<rt>そうさかいしじ</rt></ruby>に<ruby>実<rt>じつ</rt></ruby>Budgetの<ruby>全<rt>ぜん</rt></ruby>LimitsがProfile<ruby>上限以下<rt>じょうげんいか</rt></ruby>であることを<ruby>照合<rt>しょうごう</rt></ruby>し、<ruby>超<rt>こ</rt></ruby>える<ruby>場合<rt>ばあい</rt></ruby>はLimitsMismatchで<ruby>拒否<rt>きょひ</rt></ruby>する。<ruby>小<rt>ちい</rt></ruby>さい<ruby>操作予算<rt>そうさよさん</rt></ruby>を<ruby>使<rt>つか</rt></ruby>うことは<ruby>許<rt>ゆる</rt></ruby>す。<ruby>既<rt>すで</rt></ruby>に<ruby>消費<rt>しょうひ</rt></ruby>したUsageはresetせず、<ruby>継続<rt>けいぞく</rt></ruby>も<ruby>同<rt>おな</rt></ruby>じ<ruby>操作<rt>そうさ</rt></ruby>Limitsと<ruby>単調<rt>たんちょう</rt></ruby>なUsageを<ruby>保持<rt>ほじ</rt></ruby>する。Profileのresolve<ruby>自体<rt>じたい</rt></ruby>を<ruby>行<rt>おこな</rt></ruby>う<ruby>開発<rt>かいはつ</rt></ruby>・host<ruby>側<rt>がわ</rt></ruby>Budgetはこの<ruby>解析操作<rt>かいせきそうさ</rt></ruby>Budgetとは<ruby>別<rt>べつ</rt></ruby>であり、<ruby>解決時<rt>かいけつじ</rt></ruby>の<ruby>消費<rt>しょうひ</rt></ruby>を<ruby>解析<rt>かいせき</rt></ruby>へ<ruby>済<rt>す</rt></ruby>んだものとして<ruby>移<rt>うつ</rt></ruby>さない。

EntryContextはaliasを<ruby>明示保存<rt>めいじほぞん</rt></ruby>する。<ruby>同<rt>おな</rt></ruby>じpackage identityを<ruby>異<rt>こと</rt></ruby>なるaliasで<ruby>登録<rt>とうろく</rt></ruby>してcategory\-mode overrideだけを<ruby>変<rt>か</rt></ruby>えることを<ruby>許<rt>ゆる</rt></ruby>し、<ruby>子<rt>こ</rt></ruby>のLocal<ruby>解決<rt>かいけつ</rt></ruby>も<ruby>親<rt>おや</rt></ruby>の<ruby>実<rt>じつ</rt></ruby>aliasを<ruby>使<rt>つか</rt></ruby>う。Profileのlanguage<ruby>列<rt>れつ</rt></ruby>の<ruby>並<rt>なら</rt></ruby>び<ruby>順<rt>じゅん</rt></ruby>は<ruby>意味<rt>いみ</rt></ruby>に<ruby>含<rt>ふく</rt></ruby>めず、aliasから<ruby>選<rt>えら</rt></ruby>ぶ<ruby>対応<rt>たいおう</rt></ruby>を<ruby>継続<rt>けいぞく</rt></ruby>へ<ruby>保持<rt>ほじ</rt></ruby>する。alias<ruby>別<rt>べつ</rt></ruby>のreader stateとenvironmentも<ruby>明示<rt>めいじ</rt></ruby>し、guest<ruby>不足<rt>ふそく</rt></ruby>をUnitやhost<ruby>環境<rt>かんきょう</rt></ruby>で<ruby>補<rt>おぎな</rt></ruby>わない。

<a name="n-62726964676573"></a>

<a name="2-標準bridge"></a>

## 2\. <ruby>標準<rt>ひょうじゅん</rt></ruby>bridge

ここに<ruby>列挙<rt>れっきょ</rt></ruby>する<ruby>言語<rt>げんご</rt></ruby>と<ruby>拡張子<rt>かくちょうし</rt></ruby>は<ruby>標準構成<rt>ひょうじゅんこうせい</rt></ruby>であり、<ruby>追加可能<rt>ついかかのう</rt></ruby>なlanguage packageの<ruby>上限<rt>じょうげん</rt></ruby>ではない。Math LabelのDoc\.Sentenceは<ruby>既存<rt>きそん</rt></ruby>bridgeの<ruby>境界<rt>きょうかい</rt></ruby>である。Sentenceの<ruby>最終所有者<rt>さいしゅうしょゆうしゃ</rt></ruby>はNEPL3sentenceであり、consumer<ruby>移行<rt>いこう</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>は[23章](<23\-sentence\-annotation\.md>)を<ruby>参照<rt>さんしょう</rt></ruby>する。

| host slot | source<ruby>入口<rt>いりぐち</rt></ruby> | guest root | <ruby>操作<rt>そうさ</rt></ruby> |
| --- | --- | --- | --- |
| Doc InlineMath | Math | Math\.Expr | lower\/check、17<ruby>章<rt>しょう</rt></ruby>の<ruby>生成<rt>せいせい</rt></ruby>policyによるrender |
| Doc DisplayMath | Math | Math\.Expr | <ruby>同上<rt>どうじょう</rt></ruby>、display style |
| Doc CircuitFigure | Circuit | Circuit\.Design | lower\/check\/elaborate\/diagram |
| Doc Code | Grammar\/Doc\/Math\/Circuit | <ruby>対応<rt>たいおう</rt></ruby>する<ruby>根<rt>ね</rt></ruby> | source\/viewの<ruby>表示<rt>ひょうじ</rt></ruby>のみ |
| Math Label | Doc | Doc\.Sentence | lower\/check\/render phrasing |

MathのDoc<ruby>注記<rt>ちゅうき</rt></ruby>は<ruby>文書全体<rt>ぶんしょぜんたい</rt></ruby>やparagraphを<ruby>受<rt>う</rt></ruby>け<ruby>入<rt>い</rt></ruby>れない。これでMathMLの<ruby>注記位置<rt>ちゅうきいち</rt></ruby>へblock<ruby>文書<rt>ぶんしょ</rt></ruby>が<ruby>混入<rt>こんにゅう</rt></ruby>しない。Code<ruby>内<rt>ない</rt></ruby>の<ruby>不正<rt>ふせい</rt></ruby>なguestはRecover<ruby>構文<rt>こうぶん</rt></ruby>として<ruby>保持<rt>ほじ</rt></ruby>できる。render<ruby>時<rt>じ</rt></ruby>にもevaluate\/compileを<ruby>開始<rt>かいし</rt></ruby>しない。

<ruby>言語<rt>げんご</rt></ruby>の<ruby>切替<rt>きりか</rt></ruby>えは<ruby>一<rt>ひと</rt></ruby>つの<ruby>引数<rt>ひきすう</rt></ruby>の<ruby>範囲<rt>はんい</rt></ruby>に<ruby>限定<rt>げんてい</rt></ruby>し、<ruby>終了時<rt>しゅうりょうじ</rt></ruby>にhostのmodeへ<ruby>戻<rt>もど</rt></ruby>る。たとえばDoc\/math\/Math\/add<ruby>内<rt>ない</rt></ruby>からDoc sentenceへ<ruby>戻<rt>もど</rt></ruby>っても、そのguestの<ruby>外側<rt>そとがわ</rt></ruby>のtriviaを<ruby>読<rt>よ</rt></ruby>み<ruby>過<rt>す</rt></ruby>ぎない。

<a name="n-726563757273696f6e"></a>

<a name="3-相互依存と再帰"></a>

## 3\. <ruby>相互依存<rt>そうごいぞん</rt></ruby>と<ruby>再帰<rt>さいき</rt></ruby>

ソース<ruby>上<rt>じょう</rt></ruby>の<ruby>有限<rt>ゆうげん</rt></ruby>な<ruby>入<rt>い</rt></ruby>れ<ruby>子<rt>こ</rt></ruby>は<ruby>許<rt>ゆる</rt></ruby>す。ForeignSyntaxはroot node、language revision、originと<ruby>環境参照<rt>かんきょうさんしょう</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>操作<rt>そうさ</rt></ruby>グラフは<ruby>対象<rt>たいしょう</rt></ruby>nodeとoperationの<ruby>組<rt>くみ</rt></ruby>を<ruby>頂点<rt>ちょうてん</rt></ruby>に<ruby>持<rt>も</rt></ruby>ち、suiteが<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>依存順<rt>いぞんじゅん</rt></ruby>に<ruby>処理<rt>しょり</rt></ruby>する。

Doc→Math→Docの<ruby>有限<rt>ゆうげん</rt></ruby>の<ruby>注記<rt>ちゅうき</rt></ruby>は<ruby>循環<rt>じゅんかん</rt></ruby>ではない。<ruby>同<rt>おな</rt></ruby>じnodeのrenderが<ruby>再<rt>ふたた</rt></ruby>び<ruby>自分<rt>じぶん</rt></ruby>のrenderを<ruby>要求<rt>ようきゅう</rt></ruby>する<ruby>等<rt>とう</rt></ruby>の<ruby>循環<rt>じゅんかん</rt></ruby>はCyclicOperation。<ruby>生成<rt>せいせい</rt></ruby>によって<ruby>構造<rt>こうぞう</rt></ruby>が<ruby>増<rt>ふ</rt></ruby>える<ruby>場合<rt>ばあい</rt></ruby>も<ruby>共通予算<rt>きょうつうよさん</rt></ruby>とoriginを<ruby>維持<rt>いじ</rt></ruby>する。

<a name="n-656e7669726f6e6d656e74"></a>

<a name="4-環境の受渡し"></a>

## 4\. <ruby>環境<rt>かんきょう</rt></ruby>の<ruby>受渡<rt>うけわた</rt></ruby>し

<ruby>各<rt>かく</rt></ruby>guestの<ruby>名前空間<rt>なまえくうかん</rt></ruby>は<ruby>既定<rt>きてい</rt></ruby>で<ruby>新<rt>あたら</rt></ruby>しく<ruby>分離<rt>ぶんり</rt></ruby>する。DocLabel、MathSymbol、CircuitSignalを<ruby>一<rt>ひと</rt></ruby>つの<ruby>名前辞書<rt>なまえじしょ</rt></ruby>へ<ruby>入<rt>い</rt></ruby>れない。bridgeは<ruby>必要<rt>ひつよう</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じて `EnvironmentProjection` として、どのnamespace\/entityをどの<ruby>型<rt>かた</rt></ruby>の<ruby>外部値<rt>がいぶち</rt></ruby>として<ruby>渡<rt>わた</rt></ruby>すかを<ruby>明示<rt>めいじ</rt></ruby>する。

<ruby>配布<rt>はいふ</rt></ruby>bridgeはMathのfree symbol<ruby>値<rt>あたい</rt></ruby>をRenderContextから<ruby>明示的<rt>めいじてき</rt></ruby>に<ruby>受<rt>う</rt></ruby>け<ruby>取<rt>と</rt></ruby>り、DocLabelとCircuitSignalは<ruby>自動<rt>じどう</rt></ruby>exportしない。Doc annotation<ruby>内<rt>ない</rt></ruby>のlabelはそのsentenceの<ruby>局所<rt>きょくしょ</rt></ruby>scopeで<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>呼出<rt>よびだ</rt></ruby>し<ruby>側<rt>がわ</rt></ruby>のlabel<ruby>参照<rt>さんしょう</rt></ruby>が<ruby>必要<rt>ひつよう</rt></ruby>ならprofileのprojectionで<ruby>明示<rt>めいじ</rt></ruby>する。

<a name="n-617274696661637473"></a>

<a name="5-artifactの準備"></a>

## 5\. artifactの<ruby>準備<rt>じゅんび</rt></ruby>

backendsに<ruby>任意<rt>にんい</rt></ruby>raw HTML stringを<ruby>渡<rt>わた</rt></ruby>さない。suiteがforeign subtreeをtyped MarkupFragmentへ<ruby>変換<rt>へんかん</rt></ruby>し、そのslotに<ruby>適合<rt>てきごう</rt></ruby>する<ruby>内容<rt>ないよう</rt></ruby>モデルを<ruby>検査<rt>けんさ</rt></ruby>する。doc\-htmlはPreparedEmbedsの<ruby>対応表<rt>たいおうひょう</rt></ruby>を<ruby>入力<rt>にゅうりょく</rt></ruby>として<ruby>受<rt>う</rt></ruby>け<ruby>取<rt>と</rt></ruby>る。math\-mathml\/circuit\-svgを<ruby>直接<rt>ちょくせつ</rt></ruby>importしない。

asset<ruby>参照<rt>さんしょう</rt></ruby>は<ruby>固定内容<rt>こていないよう</rt></ruby>とdigestを<ruby>持<rt>も</rt></ruby>つResourceSnapshot。coreがpathから<ruby>読<rt>よ</rt></ruby>んだりURLへ<ruby>接続<rt>せつぞく</rt></ruby>したりしない。HTML<ruby>出力<rt>しゅつりょく</rt></ruby>は<ruby>既定<rt>きてい</rt></ruby>で<ruby>外部<rt>がいぶ</rt></ruby>network<ruby>無<rt>な</rt></ruby>しで<ruby>閲覧<rt>えつらん</rt></ruby>できる。KaTeX<ruby>生成時<rt>せいせいじ</rt></ruby>は<ruby>同<rt>おな</rt></ruby>じ<ruby>固定版<rt>こていばん</rt></ruby>のCSS\/fontをartifactへ<ruby>同梱<rt>どうこん</rt></ruby>し、<ruby>相対参照<rt>そうたいさんしょう</rt></ruby>とlicenseを<ruby>保持<rt>ほじ</rt></ruby>する。host<ruby>生成<rt>せいせい</rt></ruby>・<ruby>独立<rt>どくりつ</rt></ruby>MathML fallback・<ruby>出力検査<rt>しゅつりょくけんさ</rt></ruby>・asset identityは[17<ruby>章<rt>しょう</rt></ruby>](<17\-math\-html\.md>)に<ruby>従<rt>したが</rt></ruby>う。<ruby>全資源<rt>ぜんしげん</rt></ruby>を<ruby>明示的<rt>めいじてき</rt></ruby>なartifact dependencyとして<ruby>報告<rt>ほうこく</rt></ruby>する。

<a name="n-636c69"></a>

<a name="6-cli"></a>

## 6\. CLI

- `nepl3 parse --language doc input.nepld --format ndf|json` はRecover treeと<ruby>診断<rt>しんだん</rt></ruby>を<ruby>出<rt>だ</rt></ruby>す。
- `nepl3 check input.nepld` は<ruby>必要<rt>ひつよう</rt></ruby>なdomain<ruby>検査<rt>けんさ</rt></ruby>と<ruby>参照検査<rt>さんしょうけんさ</rt></ruby>を<ruby>行<rt>おこな</rt></ruby>う。
- `nepl3 render input.nepld --output out.html` は<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みをprepareしHTMLを<ruby>出<rt>だ</rt></ruby>す。
- `nepl3 render input.neplm --output out.html` は<ruby>生成済<rt>せいせいず</rt></ruby>み<ruby>数式<rt>すうしき</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>むHTMLと<ruby>必要<rt>ひつよう</rt></ruby>assetを<ruby>出<rt>だ</rt></ruby>す。Doc\/Mathのrenderは `--math-renderer katex-preferred|mathml-only` を<ruby>取<rt>と</rt></ruby>り、<ruby>既定<rt>きてい</rt></ruby>はKaTeX<ruby>優先<rt>ゆうせん</rt></ruby>。<ruby>生成能力不足等<rt>せいせいのうりょくふそくとう</rt></ruby>のMathML fallbackは<ruby>診断<rt>しんだん</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>する。
- `nepl3 evaluate input.neplm --bindings bindings.ndf` はExact\/Symbolic\/Invalidを<ruby>構造化出力<rt>こうぞうかしゅつりょく</rt></ruby>する。
- `nepl3 grammar compile input.neplg --output out.ndf` はLanguagePackageを<ruby>出<rt>だ</rt></ruby>す。
- `nepl3 circuit test input.neplc` は<ruby>全<rt>ぜん</rt></ruby>testを<ruby>実行<rt>じっこう</rt></ruby>する。
- `nepl3 circuit compile input.neplc --target nor --output out.ndf` はNOR IRを<ruby>出<rt>だ</rt></ruby>す。
- `nepl3 circuit diagram input.neplc --output out.svg` はSVGを<ruby>出<rt>だ</rt></ruby>す。
- `nepl3 format input --style prefix|compact` は<ruby>明示的<rt>めいじてき</rt></ruby>なformatter。<ruby>既定<rt>きてい</rt></ruby>はstdoutで、\-\-write<ruby>時<rt>じ</rt></ruby>だけファイルを<ruby>置換<rt>ちかん</rt></ruby>する。

<ruby>終了<rt>しゅうりょう</rt></ruby>code\: 0 <ruby>成功<rt>せいこう</rt></ruby>（Symbolicは<ruby>操作<rt>そうさ</rt></ruby>が<ruby>許<rt>ゆる</rt></ruby>す<ruby>正常結果<rt>せいじょうけっか</rt></ruby>）、1 <ruby>入力<rt>にゅうりょく</rt></ruby>\/<ruby>検査<rt>けんさ</rt></ruby>\/テスト<ruby>失敗<rt>しっぱい</rt></ruby>、2 CLI\/config\/protocolエラー、3 limit\/cancel、4 provider<ruby>内部違反<rt>ないぶいはん</rt></ruby>。stdoutは<ruby>成果物<rt>せいかぶつ</rt></ruby>だけ。sourceのdecode<ruby>失敗<rt>しっぱい</rt></ruby>を<ruby>成功空文書<rt>せいこうからぶんしょ</rt></ruby>にしない。

<a name="n-686f737473"></a>

<a name="7-browsernativewasi"></a>

## 7\. browser\/native\/WASI

<ruby>同<rt>おな</rt></ruby>じsuite APIを<ruby>利用<rt>りよう</rt></ruby>する。browserではWorkerで<ruby>計算<rt>けいさん</rt></ruby>し、<ruby>未応答時<rt>みおうとうじ</rt></ruby>はWorkerを<ruby>終了<rt>しゅうりょう</rt></ruby>できる。ファイル\/ネットワーク<ruby>権限<rt>けんげん</rt></ruby>はWeb shellに<ruby>限定<rt>げんてい</rt></ruby>する。wasm\-bindgenのJS undefined\/nullは<ruby>境界<rt>きょうかい</rt></ruby>でOption\/Resultへ<ruby>変換<rt>へんかん</rt></ruby>し、domainへ<ruby>流<rt>なが</rt></ruby>さない。

wasm32\-wasip2 CLIはWASI I\/O adapterを<ruby>使<rt>つか</rt></ruby>う。nativeのprocess provider<ruby>呼出<rt>よびだ</rt></ruby>しをbrowser\/WASIへ<ruby>無条件<rt>むじょうけん</rt></ruby>に<ruby>持<rt>も</rt></ruby>ち<ruby>込<rt>こ</rt></ruby>まない。<ruby>該当<rt>がいとう</rt></ruby>hostが<ruby>提供<rt>ていきょう</rt></ruby>するregistry\/runnerのcapabilityを<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>組込<rt>くみこ</rt></ruby>み4<ruby>言語<rt>げんご</rt></ruby>の<ruby>基本操作<rt>きほんそうさ</rt></ruby>は<ruby>全<rt>ぜん</rt></ruby>targetで<ruby>使<rt>つか</rt></ruby>える。
