<!-- Generated from doc/spec/10&#45;integration.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page integration; source SHA-256 b79c9380f71d594650fd021527e3f49c04e0996d0d2106bea087f5a92074f48e; alias input SHA-256 00e7d3fbbc79af03ae9a66bba6cc0bcd47165fb3eb087ea15a633624a51e1309; page input SHA-256 5a2c82d7261aa570d7aa4138dfb88f7cd0f7c8bb0c5f45c272f2c0f7dc28fc75. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="10-profile埋め込み実行入口"></a>

# 10\. Profile・<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>み・<ruby>実行入口<rt>じっこういりぐち</rt></ruby>

[正本（NEPL3d）](<10-integration.nepld>)

この<ruby>章<rt>しょう</rt></ruby>は、<ruby>複数<rt>ふくすう</rt></ruby>の<ruby>言語<rt>げんご</rt></ruby>と<ruby>操作<rt>そうさ</rt></ruby>を<ruby>接続<rt>せつぞく</rt></ruby>するsuite、<ruby>言語間<rt>げんごかん</rt></ruby>のbridge、CLIの<ruby>契約<rt>けいやく</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。<ruby>利用<rt>りよう</rt></ruby>する<ruby>言語<rt>げんご</rt></ruby>とproviderの<ruby>選択<rt>せんたく</rt></ruby>、<ruby>環境<rt>かんきょう</rt></ruby>と<ruby>資源<rt>しげん</rt></ruby>の<ruby>受渡<rt>うけわた</rt></ruby>し、<ruby>成果物<rt>せいかぶつ</rt></ruby>の<ruby>準備<rt>じゅんび</rt></ruby>、<ruby>実行環境<rt>じっこうかんきょう</rt></ruby>ごとの<ruby>責務<rt>せきむ</rt></ruby>を<ruby>規定<rt>きてい</rt></ruby>する。<ruby>各操作<rt>かくそうさ</rt></ruby>と<ruby>対象環境<rt>たいしょうかんきょう</rt></ruby>の<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>・<ruby>受入結果<rt>うけいれけっか</rt></ruby>は、implementation\-status\.jsonで<ruby>管理<rt>かんり</rt></ruby>する。

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## suiteの<ruby>接続責務<rt>せつぞくせきむ</rt></ruby>

suiteは、<ruby>登録済<rt>とうろくず</rt></ruby>みlanguage package、domain operation、output adapterを<ruby>接続<rt>せつぞく</rt></ruby>する。suiteは、<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>まれた<ruby>構文<rt>こうぶん</rt></ruby>に<ruby>対<rt>たい</rt></ruby>する<ruby>操作<rt>そうさ</rt></ruby>を<ruby>調整<rt>ちょうせい</rt></ruby>する。crateの<ruby>依存関係<rt>いぞんかんけい</rt></ruby>は、<ruby>各<rt>かく</rt></ruby>coreの<ruby>責務<rt>せきむ</rt></ruby>に<ruby>従<rt>したが</rt></ruby>って<ruby>定<rt>さだ</rt></ruby>める。<ruby>言語<rt>げんご</rt></ruby>を<ruby>相互<rt>そうご</rt></ruby>に<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>むために、domain core<ruby>間<rt>かん</rt></ruby>の<ruby>直接依存<rt>ちょくせついぞん</rt></ruby>を<ruby>追加<rt>ついか</rt></ruby>することは<ruby>禁止<rt>きんし</rt></ruby>する。

<a name="n-70726f66696c65"></a>

<a name="1-profile"></a>

## 1\. Profile

ParseProfileは、<ruby>解析<rt>かいせき</rt></ruby>で<ruby>利用<rt>りよう</rt></ruby>する<ruby>言語<rt>げんご</rt></ruby>と<ruby>読取設定<rt>よみとりせってい</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>する<ruby>不変値<rt>ふへんち</rt></ruby>である。contextは、<ruby>各構文位置<rt>かくこうぶんいち</rt></ruby>で<ruby>有効<rt>ゆうこう</rt></ruby>な<ruby>読取状態<rt>よみとりじょうたい</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す。<ruby>先行<rt>せんこう</rt></ruby>する<ruby>構文<rt>こうぶん</rt></ruby>は、ParseProfileを<ruby>保持<rt>ほじ</rt></ruby>したまま<ruby>後続<rt>こうぞく</rt></ruby>のcontextを<ruby>更新<rt>こうしん</rt></ruby>できる。<ruby>各<rt>かく</rt></ruby>headのshapeはその<ruby>出現<rt>しゅつげん</rt></ruby>までに<ruby>確定<rt>かくてい</rt></ruby>した<ruby>情報<rt>じょうほう</rt></ruby>から<ruby>決<rt>き</rt></ruby>め、<ruby>後続<rt>こうぞく</rt></ruby>sourceから<ruby>遡及<rt>そきゅう</rt></ruby>して<ruby>変更<rt>へんこう</rt></ruby>しない。

suiteのProfileは、language aliasからSchemaRefへの<ruby>対応<rt>たいおう</rt></ruby>、category mode、provider allowlist、operation bridge、resource snapshot、Limitsを<ruby>保持<rt>ほじ</rt></ruby>する<ruby>不変値<rt>ふへんち</rt></ruby>である。root languageは、ファイル<ruby>拡張子<rt>かくちょうし</rt></ruby>またはCLI<ruby>引数<rt>ひきすう</rt></ruby>で<ruby>選択<rt>せんたく</rt></ruby>する。<ruby>各位置<rt>かくいち</rt></ruby>のtokenは、その<ruby>位置<rt>いち</rt></ruby>で<ruby>有効<rt>ゆうこう</rt></ruby>なcontextの<ruby>読取規則<rt>よみとりきそく</rt></ruby>に<ruby>従<rt>したが</rt></ruby>って<ruby>取得<rt>しゅとく</rt></ruby>する。<ruby>全<rt>ぜん</rt></ruby>ソースを<ruby>一律<rt>いちりつ</rt></ruby>のlexerで<ruby>事前<rt>じぜん</rt></ruby>にtoken<ruby>化<rt>か</rt></ruby>することは<ruby>禁止<rt>きんし</rt></ruby>する。

`design/profile.json` は、Profileの<ruby>生成元<rt>せいせいもと</rt></ruby>となるsource manifestである。R009の<ruby>解消<rt>かいしょう</rt></ruby>では<ruby>生成結果<rt>せいせいけっか</rt></ruby>の<ruby>閉<rt>と</rt></ruby>じた<ruby>型<rt>かた</rt></ruby>、<ruby>各<rt>かく</rt></ruby>schema\/package\/provider digest、<ruby>許可<rt>きょか</rt></ruby>capability、resource identityと<ruby>整合検査<rt>せいごうけんさ</rt></ruby>を<ruby>先<rt>さき</rt></ruby>に<ruby>定<rt>さだ</rt></ruby>める。T05\/T11で<ruby>実際<rt>じっさい</rt></ruby>の<ruby>検査済<rt>けんさず</rt></ruby>みpackageから<ruby>生成<rt>せいせい</rt></ruby>・<ruby>差分検査<rt>さぶんけんさ</rt></ruby>し、UI\/Workerはこの<ruby>値<rt>あたい</rt></ruby>を<ruby>利用<rt>りよう</rt></ruby>する。R006の<ruby>操作<rt>そうさ</rt></ruby>・bundle<ruby>型<rt>がた</rt></ruby>の<ruby>未定義<rt>みていぎ</rt></ruby>を<ruby>文字列<rt>もじれつ</rt></ruby>signatureや<ruby>仮<rt>かり</rt></ruby>digestで<ruby>補<rt>おぎな</rt></ruby>わない。

<ruby>解析<rt>かいせき</rt></ruby>の<ruby>実入口<rt>じついりぐち</rt></ruby>はengineの`ParseProfile`とし、suite Profileから<ruby>渡<rt>わた</rt></ruby>す<ruby>不変<rt>ふへん</rt></ruby>なprojectionとして<ruby>扱<rt>あつか</rt></ruby>う。`interfaces/engine.json`にlanguage<ruby>登録<rt>とうろく</rt></ruby>、category mode、<ruby>選択<rt>せんたく</rt></ruby>schema、provider<ruby>要件<rt>ようけん</rt></ruby>、operation allowlist、resource identity、Limitsの<ruby>型<rt>かた</rt></ruby>を<ruby>置<rt>お</rt></ruby>く。`resolve`は<ruby>独立<rt>どくりつ</rt></ruby>したhost RuntimeCatalogとfinalize<ruby>済<rt>ず</rt></ruby>みregistryを<ruby>使<rt>つか</rt></ruby>い、<ruby>実<rt>じつ</rt></ruby>package<ruby>意味<rt>いみ</rt></ruby>digest、schema<ruby>参照閉包<rt>さんしょうへいほう</rt></ruby>、guest category\/mode、host provider<ruby>登録<rt>とうろく</rt></ruby>の<ruby>実装<rt>じっそう</rt></ruby>identity、resourceの<ruby>実<rt>じつ</rt></ruby>byte<ruby>列<rt>れつ</rt></ruby>digestを<ruby>照合<rt>しょうごう</rt></ruby>する。<ruby>外部<rt>がいぶ</rt></ruby>Profileに<ruby>記載<rt>きさい</rt></ruby>されたidentityは、hostが<ruby>独立<rt>どくりつ</rt></ruby>に<ruby>確定<rt>かくてい</rt></ruby>した<ruby>登録情報<rt>とうろくじょうほう</rt></ruby>と<ruby>照合<rt>しょうごう</rt></ruby>する。

provider identityはhostが<ruby>実<rt>じつ</rt></ruby>assetまたは<ruby>版管理<rt>ばんかんり</rt></ruby>された<ruby>実装<rt>じっそう</rt></ruby>manifestから<ruby>確定<rt>かくてい</rt></ruby>する。coreはbinaryを<ruby>読<rt>よ</rt></ruby>み<ruby>込<rt>こ</rt></ruby>まず、binary<ruby>自身<rt>じしん</rt></ruby>のhashを<ruby>同<rt>おな</rt></ruby>じbinary<ruby>内<rt>ない</rt></ruby>の<ruby>定数<rt>ていすう</rt></ruby>へ<ruby>埋<rt>う</rt></ruby>める<ruby>自己<rt>じこ</rt></ruby>hash<ruby>循環<rt>じゅんかん</rt></ruby>も<ruby>要求<rt>ようきゅう</rt></ruby>しない。operationのschema<ruby>署名<rt>しょめい</rt></ruby>が<ruby>既知<rt>きち</rt></ruby>でもcallbackが<ruby>登録<rt>とうろく</rt></ruby>されたことにはならない。parseに<ruby>使<rt>つか</rt></ruby>うreader providerは<ruby>許可済<rt>きょかず</rt></ruby>み<ruby>実登録<rt>じつとうろく</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>し、<ruby>未実行<rt>みじっこう</rt></ruby>のfacts<ruby>等<rt>とう</rt></ruby>のextensionは<ruby>署名<rt>しょめい</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>するが<ruby>自動実行<rt>じどうじっこう</rt></ruby>しない。

<ruby>解析<rt>かいせき</rt></ruby>Profileのidentityは`NEPL3-PARSE-PROFILE-1`、zero byte、canonical JSONのSHA\-256。id、language<ruby>登録<rt>とうろく</rt></ruby>、schema、category\-mode<ruby>指定<rt>してい</rt></ruby>、providerの<ruby>実装<rt>じっそう</rt></ruby>identity、allowlist、resource identity、Limitsを<ruby>含<rt>ふく</rt></ruby>める。<ruby>宣言<rt>せんげん</rt></ruby>はalias\/idまたは<ruby>完全<rt>かんぜん</rt></ruby>OperationRef\/SchemaRef<ruby>順<rt>じゅん</rt></ruby>、Limits<ruby>列<rt>れつ</rt></ruby>はsourceBytes\/work\/depth\/nodes\/allocationUnits\/outputBytes\/diagnostics\/events<ruby>順<rt>じゅん</rt></ruby>とする。<ruby>実<rt>じつ</rt></ruby>resource bytesはhash<ruby>照合<rt>しょうごう</rt></ruby>し、Profileへ<ruby>全文複製<rt>ぜんぶんふくせい</rt></ruby>しない。

HeadProviderの<ruby>登録<rt>とうろく</rt></ruby>はheadProvidersのHeadRegistration\(alias\,category\,provider\)で<ruby>選<rt>えら</rt></ruby>ぶ。<ruby>同<rt>おな</rt></ruby>じpackageを<ruby>登録<rt>とうろく</rt></ruby>した<ruby>別<rt>べつ</rt></ruby>aliasの<ruby>設定<rt>せってい</rt></ruby>を<ruby>共有<rt>きょうゆう</rt></ruby>したと<ruby>推定<rt>すいてい</rt></ruby>しない。\(alias\,category\)<ruby>重複<rt>ちょうふく</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>し、shape\/childContextの<ruby>両<rt>りょう</rt></ruby>OperationRefについて<ruby>純粋<rt>じゅんすい</rt></ruby>なHeadCall→HeadReply<ruby>署名<rt>しょめい</rt></ruby>、allowlist、<ruby>独立<rt>どくりつ</rt></ruby>host catalogの<ruby>実装<rt>じっそう</rt></ruby>identityを<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>標準操作名<rt>ひょうじゅんそうさめい</rt></ruby>はheadShape\/headChildContextだが、<ruby>登録済<rt>とうろくず</rt></ruby>みの<ruby>同署名操作<rt>どうしょめいそうさ</rt></ruby>も<ruby>選択<rt>せんたく</rt></ruby>できる。Profile identityにはheadProviders keyを<ruby>含<rt>ふく</rt></ruby>め、alias\/category<ruby>順<rt>じゅん</rt></ruby>の `[alias,category,shapeOperation,childContextOperation]` <ruby>列<rt>れつ</rt></ruby>で<ruby>記述<rt>きじゅつ</rt></ruby>する。<ruby>列挙順<rt>れっきょじゅん</rt></ruby>だけの<ruby>変更<rt>へんこう</rt></ruby>はidentityを<ruby>変<rt>か</rt></ruby>えず、<ruby>操作<rt>そうさ</rt></ruby>の<ruby>役割<rt>やくわり</rt></ruby>・alias\/categoryへの<ruby>割当変更<rt>わりあてへんこう</rt></ruby>は<ruby>変<rt>か</rt></ruby>える。

native operationの<ruby>接続<rt>せつぞく</rt></ruby>では、<ruby>解決済<rt>かいけつず</rt></ruby>みParseProfileのallowlistとprovider<ruby>実装<rt>じっそう</rt></ruby>identityを、hostが<ruby>登録<rt>とうろく</rt></ruby>したInvoke・Resumeの<ruby>組<rt>くみ</rt></ruby>と<ruby>照合<rt>しょうごう</rt></ruby>する。<ruby>登録<rt>とうろく</rt></ruby>するoperationは<ruby>重複<rt>ちょうふく</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>し、InvokeとResumeは<ruby>同<rt>おな</rt></ruby>じoperationと<ruby>実装<rt>じっそう</rt></ruby>identityを<ruby>使用<rt>しよう</rt></ruby>する。<ruby>実行<rt>じっこう</rt></ruby>contextのdigestは、ASCIIのNEPL3\.Suite\.Context\.v1、zero byte、<ruby>解決済<rt>かいけつず</rt></ruby>みProfileの32 byte digest、host contextの32 byte digestをこの<ruby>順<rt>じゅん</rt></ruby>に<ruby>連結<rt>れんけつ</rt></ruby>したSHA\-256とする。rootと<ruby>各依存要求<rt>かくいぞんようきゅう</rt></ruby>にこの<ruby>規則<rt>きそく</rt></ruby>を<ruby>適用<rt>てきよう</rt></ruby>し、continuationとResumeは<ruby>確定済<rt>かくていず</rt></ruby>みcontextを<ruby>保持<rt>ほじ</rt></ruby>・<ruby>照合<rt>しょうごう</rt></ruby>する。<ruby>操作開始時<rt>そうさかいしじ</rt></ruby>には、root<ruby>要求<rt>ようきゅう</rt></ruby>と<ruby>共有実行<rt>きょうゆうじっこう</rt></ruby>Budgetの<ruby>全資源上限<rt>ぜんしげんじょうげん</rt></ruby>をProfile<ruby>上限以下<rt>じょうげんいか</rt></ruby>に<ruby>制限<rt>せいげん</rt></ruby>し、<ruby>超過<rt>ちょうか</rt></ruby>をcallback<ruby>実行前<rt>じっこうまえ</rt></ruby>に<ruby>拒否<rt>きょひ</rt></ruby>する。hostは<ruby>各要求<rt>かくようきゅう</rt></ruby>のenvironmentとsourceの<ruby>権限<rt>けんげん</rt></ruby>を<ruby>供給<rt>きょうきゅう</rt></ruby>し、schedulerは<ruby>既存<rt>きそん</rt></ruby>の<ruby>権限検査<rt>けんげんけんさ</rt></ruby>と<ruby>累積資源管理<rt>るいせきしげんかんり</rt></ruby>を<ruby>適用<rt>てきよう</rt></ruby>する。

ここでのallowlistはoperation<ruby>呼出可否<rt>よびだしかひ</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す。providerのtransport、<ruby>隔離<rt>かくり</rt></ruby>、ネットワーク<ruby>等<rt>とう</rt></ruby>の<ruby>権限<rt>けんげん</rt></ruby>、<ruby>強制停止方法<rt>きょうせいていしほうほう</rt></ruby>、bridge、EnvironmentProjectionを<ruby>含<rt>ふく</rt></ruby>むfull suite Profileの<ruby>契約<rt>けいやく</rt></ruby>は<ruby>引<rt>ひ</rt></ruby>き<ruby>続<rt>つづ</rt></ruby>き<ruby>実装対象<rt>じっそうたいしょう</rt></ruby>である。R009の<ruby>完了<rt>かんりょう</rt></ruby>には、<ruby>解析<rt>かいせき</rt></ruby>projectionとfull suite Profileの<ruby>契約<rt>けいやく</rt></ruby>をともに<ruby>満<rt>み</rt></ruby>たす<ruby>必要<rt>ひつよう</rt></ruby>がある。hostは<ruby>実行環境<rt>じっこうかんきょう</rt></ruby>のcapabilityを<ruby>別途検査<rt>べっとけんさ</rt></ruby>し、<ruby>解析<rt>かいせき</rt></ruby>projectionはその<ruby>承認<rt>しょうにん</rt></ruby>を<ruby>代行<rt>だいこう</rt></ruby>しない。

<ruby>配布拡張子<rt>はいふかくちょうし</rt></ruby>は `.neplg`、`.nepld`、`.neplm`、`.neplc`。<ruby>汎用<rt>はんよう</rt></ruby> `.nepl` ではlanguage<ruby>指定<rt>してい</rt></ruby>を<ruby>必須<rt>ひっす</rt></ruby>にする。<ruby>既存<rt>きそん</rt></ruby>NCGやGlossのファイルを<ruby>新言語<rt>しんげんご</rt></ruby>として<ruby>黙<rt>だま</rt></ruby>って<ruby>解釈<rt>かいしゃく</rt></ruby>しない。

ParseProfile\.limitsは、<ruby>解析操作<rt>かいせきそうさ</rt></ruby>が<ruby>遵守<rt>じゅんしゅ</rt></ruby>するresource<ruby>別<rt>べつ</rt></ruby>の<ruby>上限<rt>じょうげん</rt></ruby>である。<ruby>操作開始時<rt>そうさかいしじ</rt></ruby>に<ruby>実<rt>じつ</rt></ruby>Budgetの<ruby>全<rt>ぜん</rt></ruby>LimitsがProfile<ruby>上限以下<rt>じょうげんいか</rt></ruby>であることを<ruby>照合<rt>しょうごう</rt></ruby>し、<ruby>超<rt>こ</rt></ruby>える<ruby>場合<rt>ばあい</rt></ruby>はLimitsMismatchで<ruby>拒否<rt>きょひ</rt></ruby>する。<ruby>小<rt>ちい</rt></ruby>さい<ruby>操作予算<rt>そうさよさん</rt></ruby>を<ruby>使<rt>つか</rt></ruby>うことは<ruby>許<rt>ゆる</rt></ruby>す。<ruby>既<rt>すで</rt></ruby>に<ruby>消費<rt>しょうひ</rt></ruby>したUsageはresetせず、<ruby>継続<rt>けいぞく</rt></ruby>も<ruby>同<rt>おな</rt></ruby>じ<ruby>操作<rt>そうさ</rt></ruby>Limitsと<ruby>単調<rt>たんちょう</rt></ruby>なUsageを<ruby>保持<rt>ほじ</rt></ruby>する。Profileのresolve<ruby>自体<rt>じたい</rt></ruby>を<ruby>行<rt>おこな</rt></ruby>う<ruby>開発<rt>かいはつ</rt></ruby>・host<ruby>側<rt>がわ</rt></ruby>Budgetはこの<ruby>解析操作<rt>かいせきそうさ</rt></ruby>Budgetとは<ruby>別<rt>べつ</rt></ruby>であり、<ruby>解決時<rt>かいけつじ</rt></ruby>の<ruby>消費<rt>しょうひ</rt></ruby>を<ruby>解析<rt>かいせき</rt></ruby>へ<ruby>済<rt>す</rt></ruby>んだものとして<ruby>移<rt>うつ</rt></ruby>さない。

EntryContextはaliasを<ruby>明示保存<rt>めいじほぞん</rt></ruby>する。<ruby>同<rt>おな</rt></ruby>じpackage identityを<ruby>異<rt>こと</rt></ruby>なるaliasで<ruby>登録<rt>とうろく</rt></ruby>してcategory\-mode overrideだけを<ruby>変<rt>か</rt></ruby>えることを<ruby>許<rt>ゆる</rt></ruby>し、<ruby>子<rt>こ</rt></ruby>のLocal<ruby>解決<rt>かいけつ</rt></ruby>も<ruby>親<rt>おや</rt></ruby>の<ruby>実<rt>じつ</rt></ruby>aliasを<ruby>使<rt>つか</rt></ruby>う。Profileのlanguage<ruby>列<rt>れつ</rt></ruby>の<ruby>並<rt>なら</rt></ruby>び<ruby>順<rt>じゅん</rt></ruby>は<ruby>意味<rt>いみ</rt></ruby>に<ruby>含<rt>ふく</rt></ruby>めず、aliasから<ruby>選<rt>えら</rt></ruby>ぶ<ruby>対応<rt>たいおう</rt></ruby>を<ruby>継続<rt>けいぞく</rt></ruby>へ<ruby>保持<rt>ほじ</rt></ruby>する。alias<ruby>別<rt>べつ</rt></ruby>のreader stateとenvironmentも<ruby>明示<rt>めいじ</rt></ruby>し、guest<ruby>不足<rt>ふそく</rt></ruby>をUnitやhost<ruby>環境<rt>かんきょう</rt></ruby>で<ruby>補<rt>おぎな</rt></ruby>わない。

<a name="n-62726964676573"></a>

<a name="2-標準bridge"></a>

## 2\. <ruby>標準<rt>ひょうじゅん</rt></ruby>bridge

<ruby>次<rt>つぎ</rt></ruby>の<ruby>表<rt>ひょう</rt></ruby>は、<ruby>標準構成<rt>ひょうじゅんこうせい</rt></ruby>の<ruby>埋込先<rt>うめこみさき</rt></ruby>、<ruby>言語<rt>げんご</rt></ruby>の<ruby>入口<rt>いりぐち</rt></ruby>、guestのroot、<ruby>実行<rt>じっこう</rt></ruby>する<ruby>操作<rt>そうさ</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>す。<ruby>標準構成<rt>ひょうじゅんこうせい</rt></ruby>に<ruby>加<rt>くわ</rt></ruby>えて、<ruby>独立<rt>どくりつ</rt></ruby>したlanguage packageを<ruby>登録<rt>とうろく</rt></ruby>できる。Math LabelのDoc\.Sentenceは<ruby>既存<rt>きそん</rt></ruby>bridgeの<ruby>境界<rt>きょうかい</rt></ruby>である。Sentenceの<ruby>最終所有者<rt>さいしゅうしょゆうしゃ</rt></ruby>はNEPL3sentenceであり、consumer<ruby>移行<rt>いこう</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>は[23章](<23\-sentence\-annotation\.md>)を<ruby>参照<rt>さんしょう</rt></ruby>する。

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

Doc→Math→Docの<ruby>注記<rt>ちゅうき</rt></ruby>も、<ruby>有限<rt>ゆうげん</rt></ruby>な<ruby>構文<rt>こうぶん</rt></ruby>の<ruby>入<rt>い</rt></ruby>れ<ruby>子<rt>こ</rt></ruby>として<ruby>処理<rt>しょり</rt></ruby>できる。<ruby>操作<rt>そうさ</rt></ruby>の<ruby>循環<rt>じゅんかん</rt></ruby>は、<ruby>同<rt>おな</rt></ruby>じnodeとoperationの<ruby>組<rt>くみ</rt></ruby>へ<ruby>依存要求<rt>いぞんようきゅう</rt></ruby>が<ruby>戻<rt>もど</rt></ruby>る<ruby>場合<rt>ばあい</rt></ruby>にCyclicOperationとして<ruby>報告<rt>ほうこく</rt></ruby>する。<ruby>同<rt>おな</rt></ruby>じnodeのrenderが<ruby>自身<rt>じしん</rt></ruby>のrenderを<ruby>要求<rt>ようきゅう</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>が、これに<ruby>該当<rt>がいとう</rt></ruby>する。<ruby>生成<rt>せいせい</rt></ruby>によって<ruby>構造<rt>こうぞう</rt></ruby>が<ruby>増<rt>ふ</rt></ruby>える<ruby>場合<rt>ばあい</rt></ruby>も<ruby>共通予算<rt>きょうつうよさん</rt></ruby>とoriginを<ruby>維持<rt>いじ</rt></ruby>する。

<a name="n-656e7669726f6e6d656e74"></a>

<a name="4-環境の受渡し"></a>

## 4\. <ruby>環境<rt>かんきょう</rt></ruby>の<ruby>受渡<rt>うけわた</rt></ruby>し

<ruby>各<rt>かく</rt></ruby>guestには、<ruby>既定<rt>きてい</rt></ruby>で<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>新<rt>あたら</rt></ruby>しい<ruby>名前空間<rt>なまえくうかん</rt></ruby>を<ruby>割<rt>わ</rt></ruby>り<ruby>当<rt>あ</rt></ruby>てる。DocLabel、MathSymbol、CircuitSignalを<ruby>一<rt>ひと</rt></ruby>つの<ruby>名前辞書<rt>なまえじしょ</rt></ruby>へ<ruby>入<rt>い</rt></ruby>れない。bridgeは<ruby>必要<rt>ひつよう</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じて `EnvironmentProjection` として、どのnamespace\/entityをどの<ruby>型<rt>かた</rt></ruby>の<ruby>外部値<rt>がいぶち</rt></ruby>として<ruby>渡<rt>わた</rt></ruby>すかを<ruby>明示<rt>めいじ</rt></ruby>する。

nativeのenvironment projectionは、<ruby>検証済<rt>けんしょうず</rt></ruby>みEnvironmentから<ruby>指定<rt>してい</rt></ruby>されたbindingとresourceを<ruby>選択<rt>せんたく</rt></ruby>する。bindingの<ruby>指定<rt>してい</rt></ruby>には、<ruby>入力<rt>にゅうりょく</rt></ruby>のnamespaceと<ruby>名前<rt>なまえ</rt></ruby>、<ruby>出力<rt>しゅつりょく</rt></ruby>のnamespaceと<ruby>名前<rt>なまえ</rt></ruby>、<ruby>期待<rt>きたい</rt></ruby>するschema<ruby>型<rt>がた</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>める。<ruby>選択<rt>せんたく</rt></ruby>した<ruby>値<rt>あたい</rt></ruby>は<ruby>期待<rt>きたい</rt></ruby>する<ruby>型<rt>かた</rt></ruby>で<ruby>検査<rt>けんさ</rt></ruby>し、その<ruby>内容<rt>ないよう</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>して<ruby>出力<rt>しゅつりょく</rt></ruby>へ<ruby>配置<rt>はいち</rt></ruby>する。<ruby>選択<rt>せんたく</rt></ruby>が<ruby>空<rt>から</rt></ruby>なら、bindingとresourceを<ruby>持<rt>も</rt></ruby>たない<ruby>環境<rt>かんきょう</rt></ruby>を<ruby>生成<rt>せいせい</rt></ruby>する。<ruby>入力<rt>にゅうりょく</rt></ruby>に<ruby>存在<rt>そんざい</rt></ruby>しない<ruby>要素<rt>ようそ</rt></ruby>、<ruby>型<rt>かた</rt></ruby>の<ruby>不一致<rt>ふいっち</rt></ruby>、<ruby>不正<rt>ふせい</rt></ruby>なnamespace、<ruby>出力名<rt>しゅつりょくめい</rt></ruby>とresource IDの<ruby>重複<rt>ちょうふく</rt></ruby>は<ruby>拒否<rt>きょひ</rt></ruby>する。

<ruby>検証済<rt>けんしょうず</rt></ruby>み<ruby>環境<rt>かんきょう</rt></ruby>は、<ruby>検証対象<rt>けんしょうたいしょう</rt></ruby>の<ruby>値<rt>あたい</rt></ruby>、Origin<ruby>表<rt>ひょう</rt></ruby>、SourceStore、finalize<ruby>済<rt>ず</rt></ruby>みregistryへの<ruby>不変借用<rt>ふへんしゃくよう</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>する。projectionの<ruby>出力<rt>しゅつりょく</rt></ruby>は<ruby>選択値<rt>せんたくち</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>し、Origin IDと<ruby>元<rt>もと</rt></ruby>のOrigin<ruby>表<rt>ひょう</rt></ruby>・source<ruby>環境<rt>かんきょう</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>各操作<rt>かくそうさ</rt></ruby>へ<ruby>許可<rt>きょか</rt></ruby>するsourceはhostが<ruby>明示<rt>めいじ</rt></ruby>し、<ruby>既存<rt>きそん</rt></ruby>のGrantsで<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>探索<rt>たんさく</rt></ruby>・<ruby>複製<rt>ふくせい</rt></ruby>・<ruby>検査<rt>けんさ</rt></ruby>はBudgetへ<ruby>計上<rt>けいじょう</rt></ruby>し、<ruby>停止時<rt>ていしじ</rt></ruby>には<ruby>部分的<rt>ぶぶんてき</rt></ruby>な<ruby>環境<rt>かんきょう</rt></ruby>を<ruby>返<rt>かえ</rt></ruby>さず、<ruby>入力<rt>にゅうりょく</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>交換時<rt>こうかんじ</rt></ruby>のenvironment identityは、<ruby>既存<rt>きそん</rt></ruby>のportable codecで<ruby>計算<rt>けいさん</rt></ruby>する。

<ruby>配布<rt>はいふ</rt></ruby>bridgeはMathのfree symbol<ruby>値<rt>あたい</rt></ruby>をRenderContextから<ruby>明示的<rt>めいじてき</rt></ruby>に<ruby>受<rt>う</rt></ruby>け<ruby>取<rt>と</rt></ruby>り、DocLabelとCircuitSignalは<ruby>自動<rt>じどう</rt></ruby>exportしない。Mathの<ruby>文章注釈<rt>ぶんしょうちゅうしゃく</rt></ruby>は<ruby>独立<rt>どくりつ</rt></ruby>Sentenceが<ruby>所有<rt>しょゆう</rt></ruby>する。Sentenceの<ruby>型<rt>かた</rt></ruby>・source・foreign<ruby>境界<rt>きょうかい</rt></ruby>は<ruby>選択<rt>せんたく</rt></ruby>したadapterで<ruby>検査<rt>けんさ</rt></ruby>し、<ruby>表示結果<rt>ひょうじけっか</rt></ruby>をMathへ<ruby>接続<rt>せつぞく</rt></ruby>する。<ruby>呼出<rt>よびだ</rt></ruby>し<ruby>側<rt>がわ</rt></ruby>のlabel<ruby>参照<rt>さんしょう</rt></ruby>が<ruby>必要<rt>ひつよう</rt></ruby>ならprofileのprojectionで<ruby>明示<rt>めいじ</rt></ruby>する。

<a name="n-617274696661637473"></a>

<a name="5-artifactの準備"></a>

## 5\. artifactの<ruby>準備<rt>じゅんび</rt></ruby>

suiteがforeign subtreeをtyped MarkupFragmentへ<ruby>変換<rt>へんかん</rt></ruby>し、そのslotに<ruby>適合<rt>てきごう</rt></ruby>する<ruby>内容<rt>ないよう</rt></ruby>モデルを<ruby>検査<rt>けんさ</rt></ruby>する。doc\-htmlはPreparedEmbedsの<ruby>対応表<rt>たいおうひょう</rt></ruby>を<ruby>入力<rt>にゅうりょく</rt></ruby>として<ruby>受<rt>う</rt></ruby>け<ruby>取<rt>と</rt></ruby>る。backendへ<ruby>任意<rt>にんい</rt></ruby>のraw HTML<ruby>文字列<rt>もじれつ</rt></ruby>を<ruby>渡<rt>わた</rt></ruby>すことは<ruby>禁止<rt>きんし</rt></ruby>する。doc\-htmlからmath\-mathml\/circuit\-svgへの<ruby>直接<rt>ちょくせつ</rt></ruby>importは<ruby>禁止<rt>きんし</rt></ruby>する。

asset<ruby>参照<rt>さんしょう</rt></ruby>は、<ruby>固定<rt>こてい</rt></ruby>した<ruby>内容<rt>ないよう</rt></ruby>とdigestを<ruby>保持<rt>ほじ</rt></ruby>するResourceSnapshotで<ruby>表<rt>あらわ</rt></ruby>す。coreがpathから<ruby>読<rt>よ</rt></ruby>んだりURLへ<ruby>接続<rt>せつぞく</rt></ruby>したりしない。HTML<ruby>出力<rt>しゅつりょく</rt></ruby>は<ruby>既定<rt>きてい</rt></ruby>で<ruby>外部<rt>がいぶ</rt></ruby>network<ruby>無<rt>な</rt></ruby>しで<ruby>閲覧<rt>えつらん</rt></ruby>できる。KaTeX<ruby>生成時<rt>せいせいじ</rt></ruby>は<ruby>同<rt>おな</rt></ruby>じ<ruby>固定版<rt>こていばん</rt></ruby>のCSS\/fontをartifactへ<ruby>同梱<rt>どうこん</rt></ruby>し、<ruby>相対参照<rt>そうたいさんしょう</rt></ruby>とlicenseを<ruby>保持<rt>ほじ</rt></ruby>する。host<ruby>生成<rt>せいせい</rt></ruby>・<ruby>独立<rt>どくりつ</rt></ruby>MathML fallback・<ruby>出力検査<rt>しゅつりょくけんさ</rt></ruby>・asset identityは[17<ruby>章<rt>しょう</rt></ruby>](<17\-math\-html\.md>)に<ruby>従<rt>したが</rt></ruby>う。<ruby>全資源<rt>ぜんしげん</rt></ruby>を<ruby>明示的<rt>めいじてき</rt></ruby>なartifact dependencyとして<ruby>報告<rt>ほうこく</rt></ruby>する。

<a name="n-636c69"></a>

<a name="6-cli"></a>

## 6\. CLI

CLIは、<ruby>入力<rt>にゅうりょく</rt></ruby>ファイルと<ruby>操作<rt>そうさ</rt></ruby>を<ruby>指定<rt>してい</rt></ruby>し、<ruby>構文<rt>こうぶん</rt></ruby>・<ruby>診断<rt>しんだん</rt></ruby>・<ruby>成果物<rt>せいかぶつ</rt></ruby>を<ruby>取得<rt>しゅとく</rt></ruby>するhost<ruby>側<rt>がわ</rt></ruby>の<ruby>実行入口<rt>じっこういりぐち</rt></ruby>である。<ruby>以下<rt>いか</rt></ruby>に<ruby>各<rt>かく</rt></ruby>コマンドの<ruby>要求<rt>ようきゅう</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。<ruby>各<rt>かく</rt></ruby>コマンドと<ruby>対象環境<rt>たいしょうかんきょう</rt></ruby>の<ruby>実装状態<rt>じっそうじょうたい</rt></ruby>は、implementation\-status\.jsonを<ruby>参照<rt>さんしょう</rt></ruby>する。

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

<ruby>終了<rt>しゅうりょう</rt></ruby>コードは、0を<ruby>成功<rt>せいこう</rt></ruby>、1を<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>検査<rt>けんさ</rt></ruby>・テストの<ruby>失敗<rt>しっぱい</rt></ruby>、2をCLI\/config\/protocolエラー、3をlimit\/cancel、4をproviderの<ruby>内部違反<rt>ないぶいはん</rt></ruby>とする。<ruby>操作<rt>そうさ</rt></ruby>がSymbolicを<ruby>正常結果<rt>せいじょうけっか</rt></ruby>として<ruby>許可<rt>きょか</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>成功<rt>せいこう</rt></ruby>として<ruby>終了<rt>しゅうりょう</rt></ruby>する。stdoutへの<ruby>出力<rt>しゅつりょく</rt></ruby>は<ruby>成果物<rt>せいかぶつ</rt></ruby>に<ruby>限定<rt>げんてい</rt></ruby>する。sourceのdecode<ruby>失敗<rt>しっぱい</rt></ruby>を<ruby>成功空文書<rt>せいこうからぶんしょ</rt></ruby>にしない。

<a name="n-686f737473"></a>

<a name="7-browsernativewasi"></a>

## 7\. browser\/native\/WASI

browser・native・WASIのhostは、<ruby>同<rt>おな</rt></ruby>じsuite APIを<ruby>利用<rt>りよう</rt></ruby>する。browserではWorkerで<ruby>計算<rt>けいさん</rt></ruby>し、<ruby>未応答時<rt>みおうとうじ</rt></ruby>はWorkerを<ruby>終了<rt>しゅうりょう</rt></ruby>できる。ファイル\/ネットワーク<ruby>権限<rt>けんげん</rt></ruby>はWeb shellに<ruby>限定<rt>げんてい</rt></ruby>する。wasm\-bindgenのJS undefined\/nullは<ruby>境界<rt>きょうかい</rt></ruby>でOption\/Resultへ<ruby>変換<rt>へんかん</rt></ruby>し、domainへ<ruby>流<rt>なが</rt></ruby>さない。

wasm32\-wasip2 CLIはWASI I\/O adapterを<ruby>使<rt>つか</rt></ruby>う。nativeのprocess provider<ruby>呼出<rt>よびだ</rt></ruby>しをbrowser\/WASIへ<ruby>無条件<rt>むじょうけん</rt></ruby>に<ruby>持<rt>も</rt></ruby>ち<ruby>込<rt>こ</rt></ruby>まない。<ruby>該当<rt>がいとう</rt></ruby>hostが<ruby>提供<rt>ていきょう</rt></ruby>するregistry\/runnerのcapabilityを<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>組込<rt>くみこ</rt></ruby>み4<ruby>言語<rt>げんご</rt></ruby>の<ruby>基本操作<rt>きほんそうさ</rt></ruby>は<ruby>全<rt>ぜん</rt></ruby>targetで<ruby>使<rt>つか</rt></ruby>える。
