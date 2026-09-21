<!-- Generated from doc/spec/04&#45;grammar.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page grammar; source SHA-256 6b4700f3927dea3a5364eb165f089a10da61486d3a97a94acbde74af16d400c5; alias input SHA-256 ffe8ce448133c7124191d2d57181f7835884968b311cd4e03108d91d5c6c6f63; document digest 14aa794c232680a31e4a9147efb79a62f84ce1cebb33e4fb59c0198ffb25c038; input PageSet digest 0f2ce49aa1cabeeb08c4e819bcdc1b59d9d5c67860fc748c8a584179d468a3da; input context SHA-256 282aaa9f1f863bca7dcccb234f007f2ea850de273d33c56548853943f9df53e0. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="04-grammar言語"></a>

# 04\. Grammar言語\[げんご\]

共通\[きょうつう\]parserの各\[かく\]arenaは、操作内\[そうさない\]で受理\[じゅり\]したsource\/map列\[れつ\]の取込位置\[とりこみいち\]をprivate状態\[じょうたい\]として保持\[ほじ\]できる。受理列\[じゅりれつ\]は成功\[せいこう\]したtoken読取\[よみと\]り間\[かん\]でprefixを保持\[ほじ\]し、arenaには未取込\[みとりこみ\]suffixだけを検査\[けんさ\]・追加\[ついか\]する。Foreign arenaは独立\[どくりつ\]した取込位置\[とりこみいち\]から開始\[かいし\]し、host復帰後\[ふっきご\]も必要\[ひつよう\]なsource\/mapを保持\[ほじ\]する。外部\[がいぶ\]ParseProgressのechoからこの位置\[いち\]を構築\[こうちく\]せず、別\[べつ\]revisionの再解析\[さいかいせき\]は新\[あたら\]しい状態\[じょうたい\]から始\[はじ\]める。

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

Grammarは、reader・prefix形状\[けいじょう\]・束縛\[そくばく\]・表示\[ひょうじ\]の定義\[ていぎ\]を同\[おな\]じpackageへまとめる。任意\[にんい\]プログラムを、小\[ちい\]さなGrammar IRへ必\[かなら\]ず還元\[かんげん\]することは要求\[ようきゅう\]しない。call\/map\/thenで登録済\[とうろくず\]みproviderへ委譲\[いじょう\]できる。

<a name="n-726f6f7473"></a>

<a name="1-文書の根と名前解決"></a>

## 1\. 文書\[ぶんしょ\]の根\[ね\]と名前解決\[なまえかいけつ\]

根\[ね\]は `language name revision root declarations` とする。declarationsはcons\/nil列\[れつ\]である。全\[ぜん\]constructorは `grammar-signatures.md` に定義\[ていぎ\]する。宣言\[せんげん\]の順番\[じゅんばん\]は、metadata解決\[かいけつ\]に影響\[えいきょう\]しない。category\/mode\/reader\/namespace\/extension aliasの各名前空間\[かくなまえくうかん\]で同名\[どうめい\]を拒否\[きょひ\]し、参照先\[さんしょうさき\]をcompile時\[じ\]に解決\[かいけつ\]する。formとleafの宣言\[せんげん\]は、それぞれcategoryとkindの組\[くみ\]で一意\[いちい\]とする。

一\[ひと\]つのkindを複数\[ふくすう\]categoryで受\[う\]け入\[い\]れることは、field schemaが完全一致\[かんぜんいっち\]する場合\[ばあい\]だけ許可\[きょか\]する。grammar自身\[じしん\]のField宣言\[せんげん\]とSelectorのfieldなど、同\[おな\]じ綴\[つづ\]りでもshapeが異\[こと\]なる場合\[ばあい\]は、異\[こと\]なるkind IDを付\[つ\]ける。modeごとのspellingとschema kindを混同\[こんどう\]しない。

共有\[きょうゆう\]kindのsurface descriptorは一\[ひと\]つとし、categoryごとのbinding\/style\/read宣言\[せんげん\]は別\[べつ\]に保持\[ほじ\]する。DeclarationOriginはform\/leafに限\[かぎ\]ってcategoryを持\[も\]ち、同\[おな\]じkind名\[めい\]の各宣言位置\[かくせんげんいち\]を区別\[くべつ\]する。その他\[ほか\]の宣言\[せんげん\]のcategoryはNoneとする。この出自\[しゅつじ\]の区別\[くべつ\]はexecution identityへ含\[ふく\]めるが、意味上\[いみじょう\]の宣言順序\[せんげんじゅんじょ\]は引\[ひ\]き続\[つづ\]き無関係\[むかんけい\]とする。

`builtin Name/Text/Nat/Lang` は、配布\[はいふ\]された基礎\[きそ\]readerを直接参照\[ちょくせつさんしょう\]する。`local C` は現在言語\[げんざいげんご\]のcategory、`foreign Alias C` はprofileで固定\[こてい\]した別\[べつ\]schemaのcategoryである。`withmode M R` は、その引数\[ひきすう\]だけのmode切替\[きりか\]えを表\[あらわ\]す。`listof R` は専用\[せんよう\]list categoryを特殊化\[とくしゅか\]し、cons\/nilの既知\[きち\]shapeへ展開\[てんかい\]する。foreign aliasをpackage sourceのURLからfetchして解決\[かいけつ\]しない。profile manifestのSchemaRefを要求\[ようきゅう\]し、未登録\[みとうろく\]はMissingLanguage、digestの不一致\[ふいっち\]はSchemaMismatchとする。

<a name="n-666f726d73"></a>

<a name="2-formとleaf"></a>

## 2\. formとleaf

`form Kind Category "spelling" fields binding styles`。fieldsの長\[なが\]さをarityとし、各\[かく\]field名\[めい\]はform内\[ない\]で一意\[いちい\]とする。head自体\[じたい\]はfieldではなく、source範囲\[はんい\]は共通\[きょうつう\]parserが記録\[きろく\]する。

`leaf Kind Category TokenKind binding styles`。明示的\[めいじてき\]なtoken kindだけをarity 0として受\[う\]け入\[い\]れる。Category内\[ない\]の既知\[きち\]formのspellingとの照合\[しょうごう\]を、先\[さき\]に行\[おこな\]う。形状\[けいじょう\]に一致\[いっち\]しないWordを参照\[さんしょう\]leafとして受\[う\]け取\[と\]るかどうかは、そのCategoryのleaf宣言\[せんげん\]に従\[したが\]う。読\[よ\]み取\[と\]っただけではdomainの評価器\[ひょうかき\]を呼\[よ\]ばず、compileはGrammarの意味操作\[いみそうさ\]として明示的\[めいじてき\]に呼\[よ\]ぶ。

<a name="n-62696e64696e67"></a>

<a name="3-束縛plan"></a>

## 3\. 束縛\[そくばく\]plan

planはsource fieldとscopeの関係\[かんけい\]を記述\[きじゅつ\]するものであり、評価順\[ひょうかじゅん\]やメモリへの代入命令\[だいにゅうめいれい\]ではない。

- noneでは、このnode自身\[じしん\]は何\[なに\]も追加\[ついか\]しない。記述\[きじゅつ\]し忘\[わす\]れを隠\[かく\]すために、子\[こ\]を自動\[じどう\]visitしない。fieldのうち語義解析\[ごぎかいせき\]が不要\[ふよう\]なliteral以外\[いがい\]には、visit\/import\/propagate\/専用\[せんよう\]plan\/customのいずれかで処理方針\[しょりほうしん\]を指定\[してい\]する必要\[ひつよう\]がある。
- visit fieldは、現在\[げんざい\]scopeでその子\[こ\]のplanを適用\[てきよう\]する。子内部\[こないぶ\]のscopeは外\[そと\]へ漏\[も\]れない。
- group plansは、現在\[げんざい\]scopeを使\[つか\]ってplanをまとめる。
- scope plansは子\[こ\]scopeを作\[つく\]ってplanを適用\[てきよう\]し、bindの位置\[いち\]より後\[あと\]のvisitだけに新\[あたら\]しい宣言\[せんげん\]を見\[み\]せる。
- bind namespace fieldは、一\[ひと\]つの名前\[なまえ\]fieldから新\[あたら\]しいEntityを導入\[どうにゅう\]する。名前\[なまえ\]を作\[つく\]れるfieldは、builtin Name\/Textか、schemaでTextを返\[かえ\]すと検査\[けんさ\]されたleafに限\[かぎ\]る。Textのescapeはdecode後\[ご\]の綴\[つづ\]りで比較\[ひかく\]し、元位置\[もといち\]はSourceMapで保持\[ほじ\]する。一般\[いっぱん\]の式\[しき\]を名前\[なまえ\]へ変換\[へんかん\]しない。
- reference namespace fieldは、名前\[なまえ\]field（leafではself）を現在\[げんざい\]scopeへの参照\[さんしょう\]として登録\[とうろく\]する。
- export namespace fieldは、名前\[なまえ\]を導入候補\[どうにゅうこうほ\]として親\[おや\]へ返\[かえ\]す。単独\[たんどく\]ではglobal scopeを変更\[へんこう\]しない。
- import fieldは、子\[こ\]がexportした候補\[こうほ\]を現在\[げんざい\]scopeへ導入\[どうにゅう\]する。
- propagate fieldは、子\[こ\]のexportをこのnodeのexportとして返\[かえ\]す。child内\[ない\]の参照\[さんしょう\]も、そのchildに割\[わ\]り当\[あ\]てたscopeで登録\[とうろく\]する。
- sequential declarations bodyは、各宣言\[かくせんげん\]を、それ以前\[いぜん\]のexportだけが見\[み\]えるscopeで処理\[しょり\]する。そのexportを追加\[ついか\]した新\[あたら\]しいscopeで次\[つぎ\]へ進\[すす\]み、最後\[さいご\]のscopeでbodyを処理\[しょり\]する。
- recursive declarations bodyは、各宣言\[かくせんげん\]のexport headerを先\[さき\]に収集\[しゅうしゅう\]して共通\[きょうつう\]scopeへ導入\[どうにゅう\]する。全宣言\[ぜんせんげん\]の本体\[ほんたい\]とbodyは、そのscopeで処理\[しょり\]する。
- custom providerは、同\[おな\]じScope\/Entity\/Occurrence\/Relation契約\[けいやく\]を返\[かえ\]す専用処理\[せんようしょり\]である。providerの出力\[しゅつりょく\]についても、範囲\[はんい\]・ID・scope edgeを検査\[けんさ\]する。

二相処理\[にそうしょり\]としてscopeと宣言\[せんげん\]を作\[つく\]り、参照\[さんしょう\]を後\[あと\]で解決\[かいけつ\]してよいが、上記\[じょうき\]の可視性\[かしせい\]を変\[か\]えてはならない。scope graphは有限\[ゆうげん\]とする。候補\[こうほ\]が複数\[ふくすう\]ならAmbiguousを返\[かえ\]し、最初\[さいしょ\]に見\[み\]つかった候補\[こうほ\]へ勝手\[かって\]に決定\[けってい\]しない。

lexical namespaceは、内側\[うちがわ\]scopeを優先\[ゆうせん\]する。openは同\[おな\]じlexical探索\[たんさく\]を行\[おこな\]い、見\[み\]つからない名前\[なまえ\]を自由入力要求\[じゆうにゅうりょくようきゅう\]として返\[かえ\]す。架空\[かくう\]の定義位置\[ていぎいち\]は作\[つく\]らない。globalは、指定\[してい\]されたroot scope内\[ない\]で同名\[どうめい\]の重複\[じゅうふく\]を拒否\[きょひ\]する。別\[べつ\]articleのDocLabelや別\[べつ\]moduleのSignalを、同\[おな\]じglobalへ入\[い\]れない。rootの割当\[わりあ\]ては、そのform\/packageのplanかfacts providerが指定\[してい\]する。

Foreignの既定\[きてい\]では、各\[かく\]foreign bundleに独立\[どくりつ\]したscope rootとnamespace instanceを割\[わ\]り当\[あ\]てる。同\[おな\]じsurface schema、alias、同言語\[どうげんご\]への再帰\[さいき\]であっても、外側\[そとがわ\]rootを暗黙\[あんもく\]に共有\[きょうゆう\]しない。同\[おな\]じschema\/nameでも、rootが異\[こと\]なれば別\[べつ\]NamespaceRefとする。外側\[そとがわ\]の名前\[なまえ\]・候補\[こうほ\]を渡\[わた\]す場合\[ばあい\]は、明示\[めいじ\]EnvironmentProjectionとroot\/namespaceのprojection・grantに従\[したが\]う。読取\[よみと\]り候補\[こうほ\]の共有\[きょうゆう\]から、外側\[そとがわ\]の既存\[きそん\]scopeへ書\[か\]き込\[こ\]む権限\[けんげん\]を推定\[すいてい\]しない。import\/propagateで渡\[わた\]すexport候補\[こうほ\]も、その明示\[めいじ\]した名前空間\[なまえくうかん\]の対応\[たいおう\]に従\[したが\]う。foreignの局所\[きょくしょ\]NodeRef\/OriginRefは、所属\[しょぞく\]bundleに対\[たい\]して解決\[かいけつ\]してから、analysisの共通\[きょうつう\]tableへ再配置\[さいはいち\]する。

<a name="n-737461676573"></a>

<a name="31-宣言順と解析結果の境界"></a>

### 3\.1 宣言順\[せんげんじゅん\]と解析結果\[かいせきけっか\]の境界\[きょうかい\]

BindingStageはScopeId、previous、introduced EntityId列\[れつ\]を保持\[ほじ\]し、StageIdはその解析\[かいせき\]のstage表\[ひょう\]へのindexとする。previousは必\[かなら\]ず以前\[いぜん\]のstageを指\[さ\]す。これは同\[おな\]じscopeの以前\[いぜん\]の可視性\[かしせい\]か、scope作成時\[さくせいじ\]に捕捉\[ほそく\]した直接親\[ちょくせつおや\]scopeの可視性\[かしせい\]を表\[あらわ\]す。同\[おな\]じlexical scopeのstageを新\[あら\]たなscopeとして扱\[あつか\]わず、そのscopeの可視候補\[かしこうほ\]を集\[あつ\]めてから外側\[そとがわ\]scopeを探索\[たんさく\]する。同\[おな\]じscopeで同名\[どうめい\]の異\[こと\]なるEntityが見\[み\]える場合\[ばあい\]は、最後\[さいご\]のstageの一件\[いっけん\]だけを選\[えら\]ばない。sequentialが要求\[ようきゅう\]する新\[あたら\]しいscopeには、別\[べつ\]ScopeIdを作\[つく\]る。

初期\[しょき\]root、namespace instance、Origin閉包\[へいほう\]の割当\[わりあ\]てはhost bundleを先頭\[せんとう\]とし、正準\[せいじゅん\]node\/fieldのDFS順\[じゅん\]でForeign bundleを訪問\[ほうもん\]して行\[おこな\]う。各\[かく\]bundleの選択\[せんたく\]も正準\[せいじゅん\]node順\[じゅん\]で扱\[あつか\]い、source表\[ひょう\]はSourceId\/revision\/digest順\[じゅん\]とする。入力\[にゅうりょく\]のcontexts列\[れつ\]やselection列\[れつ\]の格納順\[かくのうじゅん\]から、FactSetのIDを割\[わ\]り当\[あ\]てない。同\[おな\]じanalysis IDとAnalysisKeyでnative実行\[じっこう\]と正準\[せいじゅん\]treeの受信後実行\[じゅしんごじっこう\]を行\[おこな\]う場合\[ばあい\]、生成\[せいせい\]Entity\/Occurrence\/ScopeのIDが異\[こと\]なる対象\[たいしょう\]を指\[さ\]してはならない。これは既存\[きそん\]opaque facts IDを、treeのnode IDと一緒\[いっしょ\]に再採番\[さいさいばん\]する規則\[きそく\]ではない。

OccurrenceStageはDefinition\/Reference\/Import\/Exportの各\[かく\]Occurrenceと、発行時\[はっこうじ\]のstageを対応\[たいおう\]させる。完成結果\[かんせいけっか\]では全\[ぜん\]Occurrenceにちょうど一\[ひと\]つの対応\[たいおう\]を要求\[ようきゅう\]し、途中結果\[とちゅうけっか\]でもOccurrenceと対応\[たいおう\]の片方\[かたほう\]だけを公開\[こうかい\]しない。visitは受\[う\]け取\[と\]った可視性\[かしせい\]から子\[こ\]のplanを実行\[じっこう\]し、子\[こ\]の変更\[へんこう\]を親\[おや\]のstageへ暗黙\[あんもく\]に反映\[はんえい\]しない。親\[おや\]への導入\[どうにゅう\]はimport、親\[おや\]への候補返却\[こうほへんきゃく\]はexport\/propagateの実行規則\[じっこうきそく\]に従\[したが\]う。stage内\[ない\]のEntityIdが存在\[そんざい\]するだけでは、そのEntityを導入\[どうにゅう\]する意味的権限\[いみてきけんげん\]を証明\[しょうめい\]しない。

Occurrence\.scopeとOccurrenceStage\.stageは、名前\[なまえ\]が現\[あらわ\]れたlexical scopeとその履歴\[りれき\]を保持\[ほじ\]する。OccurrenceStage\.namespaceStageは検索対象\[けんさくたいしょう\]の履歴\[りれき\]を別\[べつ\]に保持\[ほじ\]する。Lexical\/Openではstageと等\[ひと\]しく、Globalではそのnamespaceの指定\[してい\]rootに属\[ぞく\]する発行時\[はっこうじ\]のstageを指\[さ\]す。GlobalのEntityは指定\[してい\]rootへ配置\[はいち\]するが、Occurrenceのlexical位置\[いち\]をrootへ置換\[ちかん\]しない。Globalへのbindと明示\[めいじ\]importはrootの可視履歴\[かしりれき\]を更新\[こうしん\]し、lexical scopeを出\[で\]た後\[あと\]の兄弟\[きょうだい\]からも導入済\[どうにゅうず\]みの宣言\[せんげん\]を検索\[けんさく\]できる。exportによる候補作成\[こうほさくせい\]だけでは更新\[こうしん\]しない。明示\[めいじ\]recursiveのheader収集\[しゅうしゅう\]を除\[のぞ\]き、後続宣言\[こうぞくせんげん\]を過去\[かこ\]の参照\[さんしょう\]へ可視\[かし\]にしない。同\[おな\]じroot内\[ない\]への同名導入\[どうめいどうにゅう\]はDuplicateGlobalと位置診断\[いちしんだん\]で拒否\[きょひ\]し、別\[べつ\]Foreign bundleのrootへ暗黙\[あんもく\]に導入\[どうにゅう\]しない。

portableの構造検査\[こうぞうけんさ\]は、namespaceStageの存在\[そんざい\]、namespace policyとの組合\[くみあわ\]せ、root所属\[しょぞく\]を検査\[けんさ\]する。そのstageが実際\[じっさい\]の参照時点\[さんしょうじてん\]の最新履歴\[さいしんりれき\]だったことや、候補\[こうほ\]を導入\[どうにゅう\]する権限\[けんげん\]は、型付\[かたつ\]き値\[あたい\]だけでは証明\[しょうめい\]しない。それらは、実\[じつ\]plan実行\[じっこう\]の意味\[いみ\]proofに属\[ぞく\]する。

engineのBindingAnalysisは、実\[じつ\]BindingPlanの完走\[かんそう\]と名前解決\[なまえかいけつ\]により得\[え\]る意味\[いみ\]proofであり、共通\[きょうつう\]CheckedFactSetの型\[かた\]・ID・参照\[さんしょう\]・source検査\[けんさ\]と区別\[くべつ\]する。入力\[にゅうりょく\]treeの各\[かく\]selectionは、同\[おな\]じ解決済\[かいけつず\]みProfileのexecution identityへ再照合\[さいしょうごう\]し、意味\[いみ\]identityが等\[ひと\]しい別\[べつ\]arena配置\[はいち\]を代入\[だいにゅう\]しない。完成結果\[かんせいけっか\]は必須\[ひっす\]FactSet、stage列\[れつ\]、OccurrenceStage列\[れつ\]、open入力\[にゅうりょく\]のOccurrenceId列\[れつ\]、export候補\[こうほ\]EntityId列\[れつ\]、Report用\[よう\]source\/mapsを保持\[ほじ\]する。公開\[こうかい\]schemaの同\[おな\]じrecordを受信\[じゅしん\]しただけで、nativeの意味\[いみ\]proofを生成\[せいせい\]しない。

BindingBundleScopeは解析入力\[かいせきにゅうりょく\]treeの正準\[せいじゅん\]bundle番号\[ばんごう\]と、その準備時\[じゅんびじ\]に実際\[じっさい\]に発行\[はっこう\]したroot ScopeIdを結\[むす\]び付\[つ\]ける。ScopeIdの数値\[すうち\]を、bundle番号\[ばんごう\]やarena indexから推定\[すいてい\]しない。BindingProgress\/BindingAnalysisのbundleScopes列\[れつ\]は、正準\[せいじゅん\]bundle順\[じゅん\]の初期化済\[しょきかず\]みprefixを保持\[ほじ\]する。各\[かく\]scopeは存在\[そんざい\]する異\[こと\]なるrootでなければならない。停止前\[ていしまえ\]に発行\[はっこう\]したscope\/stageを保持\[ほじ\]し、未公開\[みこうかい\]の対応行\[たいおうぎょう\]を捏造\[ねつぞう\]しない。完了\[かんりょう\]した意味\[いみ\]proofの対応\[たいおう\]は、全\[ぜん\]bundleを覆\[おお\]う。raw返信\[へんしん\]だけでは入力\[にゅうりょく\]treeのbundle数\[すう\]や実際\[じっさい\]の発行関係\[はっこうかんけい\]を認証\[にんしょう\]できないため、prefix・参照\[さんしょう\]・root・重複\[じゅうふく\]の構造検査\[こうぞうけんさ\]を、同\[どう\]AnalysisKeyの実\[じつ\]Binding完了\[かんりょう\]proofとの照合\[しょうごう\]の代\[か\]わりにしない。

各対応行\[かくたいおうぎょう\]のcustomSourceMapsは、同\[おな\]じBindingProgress\/BindingAnalysisのsourceMaps表\[ひょう\]でCustomが正式受理\[せいしきじゅり\]した行\[ぎょう\]のindex列\[れつ\]である。呼出対象\[よびだしたいしょう\]のbundleを所有者\[しょゆうしゃ\]とし、FactsEmitterの受理時\[じゅりじ\]と検査済\[けんさず\]みFactDeltaのsource閉包受理時\[へいほうじゅりじ\]に、mapと所有\[しょゆう\]indexを同時公開\[どうじこうかい\]する。後\[あと\]の停止\[ていし\]や意味拒否\[いみきょひ\]でも、受理済\[じゅりず\]みの両列\[りょうれつ\]を保持\[ほじ\]する。indexは所有列内\[しょゆうれつない\]で狭義昇順\[きょうぎしょうじゅん\]かつ表\[ひょう\]の範囲内\[はんいない\]とし、所有列\[しょゆうれつ\]をまたいで重複\[じゅうふく\]しない。同\[おな\]じ内容\[ないよう\]のmapを別\[べつ\]bundleが明示的\[めいじてき\]に返\[かえ\]した場合\[ばあい\]は、別\[べつ\]の受理行\[じゅりぎょう\]として保持\[ほじ\]する。内容一致\[ないよういっち\]から所有者\[しょゆうしゃ\]を推測\[すいそく\]しない。元\[もと\]treeのmapには、引\[ひ\]き続\[つづ\]き各\[かく\]SyntaxBundleの局所表\[きょくしょひょう\]を使\[つか\]う。raw列\[れつ\]の構造検査\[こうぞうけんさ\]は、Custom呼出\[よびだ\]しの真正性\[しんせいせい\]や授権\[じゅけん\]の証明\[しょうめい\]ではない。

BindingReplyは共通\[きょうつう\]Reportを持\[も\]ち、Complete、Invalid、Stoppedの全枝\[ぜんえだ\]がsource\/mapsを所有\[しょゆう\]する。途中\[とちゅう\]BindingProgressは `facts:Option<FactSet>` で初期化前\[しょきかまえ\]を区別\[くべつ\]し、その場合\[ばあい\]も受\[う\]け入\[い\]れたsource\/mapsを保持\[ほじ\]する。空\[から\]のanalysis identityを持\[も\]つ偽\[にせ\]FactSetで停止\[ていし\]を表\[あらわ\]さない。Someの部分\[ぶぶん\]FactSetは共通\[きょうつう\]の構造不変条件\[こうぞうふへんじょうけん\]を満\[み\]たす必要\[ひつよう\]があるが、plan完走\[かんそう\]や名前解決\[なまえかいけつ\]の完了\[かんりょう\]を主張\[しゅちょう\]しない。InvalidのBindingFailureは、Tree\/Profile\/Package\/Planと共通\[きょうつう\]Fact\/Source\/Schema\/Origin\/View\/Syntaxの入\[い\]れ子原因\[こげんいん\]・引数\[ひきすう\]を型付\[かたつ\]きで保持\[ほじ\]する。SourceError\.DecodeのvalidUpTo\/errorLenも失\[うしな\]わず、粗\[あら\]いTree\/Source分類\[ぶんるい\]やDebug文字列\[もじれつ\]でnativeとの違\[ちが\]いを隠\[かく\]さない。UndefinedName、AmbiguousName、DuplicateDefinitionなどの語彙診断\[ごいしんだん\]は、BindingDiagnosticArguments\(namespace\, name\)と実\[じつ\]source位置\[いち\]を持\[も\]つ通常\[つうじょう\]Diagnosticとして返\[かえ\]す。

停止\[ていし\]の分類\[ぶんるい\]は、型付\[かたつ\]き原因\[げんいん\]をたどって決\[き\]める。SyntaxのSource\/View\/Schemaなどの内部\[ないぶ\]にStoppedがある場合\[ばあい\]も、元\[もと\]StopReasonをBindingOutcome\.Stoppedへ伝\[つた\]え、Invalidへ変\[か\]えない。逆\[ぎゃく\]に、別\[べつ\]の処理\[しょり\]でBudgetが停止済\[ていしず\]みであることだけを理由\[りゆう\]に、WrongTypeやBoundsなどの意味失敗\[いみしっぱい\]を上書\[うわが\]きしない。単独\[たんどく\]のBindingFailure codecは停止原因\[ていしげんいん\]もデータとして保存\[ほぞん\]できるが、BindingReplyのInvalidにStopped原因\[げんいん\]を含\[ふく\]めることは拒否\[きょひ\]する。traceOverflowも、StoppedのReportだけが持\[も\]てる。

初回\[しょかい\]BindingReply decoderは、受信\[じゅしん\]したFactSetとReport用\[よう\]source\/mapsから閉包\[へいほう\]を構成\[こうせい\]し、元\[もと\]のRust結果\[けっか\]やambient SourceStoreを要求\[ようきゅう\]しない。各\[かく\]source表内\[ひょうない\]の重複\[じゅうふく\]を拒否\[きょひ\]し、表間\[ひょうかん\]で同\[おな\]じsnapshotを共有\[きょうゆう\]する場合\[ばあい\]はbytes\/digest\/URIの一致\[いっち\]を要求\[ようきゅう\]する。stageの後方参照\[こうほうさんしょう\]、scopeの親子\[おやこ\]、全\[ぜん\]Occurrenceとの一対一対応\[いちたいいちたいおう\]、open入力\[にゅうりょく\]とexportのIDも検査\[けんさ\]する。これは構造上\[こうぞうじょう\]の結果交換\[けっかこうかん\]であり、通信認証\[つうしんにんしょう\]、元\[もと\]tree\/Profileへの要求\[ようきゅう\]identity照合\[しょうごう\]、名前解決\[なまえかいけつ\]の再実行\[さいじっこう\]を代\[か\]わりに証明\[しょうめい\]しない。schemaのBindingAnalysis recordはnativeの公開\[こうかい\]raw BindingResultへ対応\[たいおう\]し、private BindingAnalysis proofは実\[じつ\]analyzeからだけ得\[え\]る。

<a name="n-637573746f6d"></a>

<a name="custom-の-native-実行と解決更新履歴"></a>

### Customのnative実行\[じっこう\]と解決更新履歴\[かいけつこうしんりれき\]

`analyze_with_host` は、選択済\[せんたくず\]みprovider requirementと明示\[めいじ\]した呼出先\[よびだしさき\]に対\[たい\]してhostが発行\[はっこう\]したFactAuthorityを検査\[けんさ\]する。同\[おな\]じ既存\[きそん\]facts・treeを借用\[しゃくよう\]するCheckedFactsViewをcallbackへ渡\[わた\]す。owned FactsRequestと借用\[しゃくよう\]viewは同\[おな\]じvalidatorに従\[したが\]い、通常\[つうじょう\]のnative呼出\[よびだ\]しのためだけにtreeと全\[ぜん\]factsをNDFへ複製\[ふくせい\]しない。hostは登録済\[とうろくず\]み実装\[じっそう\]のrevision・digest・operationを照合\[しょうごう\]する。未登録\[みとうろく\]はMissingProviderとし、providerの自己申告\[じこしんこく\]から変更権限\[へんこうけんげん\]を得\[え\]ない。

native callbackの診断\[しんだん\]・eventは、FactsEmitterによってsource\/map・schema・Fix・件数\[けんすう\]を検査\[けんさ\]した時点\[じてん\]で、正式\[せいしき\]collectorへ追加\[ついか\]する。戻\[もど\]り値\[あたい\]のCustomOutcomeは、Complete\(delta\)、Invalid\(partial\)、Stopped\(reason\, partial\)を区別\[くべつ\]する。deltaの参照\[さんしょう\]・予約\[よやく\]ID・namespace\/root・変更権限\[へんこうけんげん\]を検査\[けんさ\]し、必要\[ひつよう\]storageを準備\[じゅんび\]した後\[あと\]に原子的\[げんしてき\]に適用\[てきよう\]する。停止\[ていし\]した親\[おや\]Budgetへ新\[あたら\]しいraw deltaを無検査\[むけんさ\]で追加\[ついか\]せず、受理済\[じゅりず\]みの診断\[しんだん\]・event・sources\/mapsを残\[のこ\]す。外部\[がいぶ\]FactsReplyのdecodeとremote実費\[じっぴ\]の精算\[せいさん\]は、このnative emitterを呼\[よ\]んだだけで成立\[せいりつ\]したことにはならない。

Customの明示\[めいじ\]resolution updateは、Occurrenceの位置\[いち\]・scope・発行時\[はっこうじ\]stage・namespaceStageを書\[か\]き換\[か\]えない。BindingResolutionBatchは選択\[せんたく\]provider、発行\[はっこう\]authority、解析要求内\[かいせきようきゅうない\]の正準\[せいじゅん\]path\/nodeで表\[あらわ\]したtarget、OccurrenceIdごとのbefore\/after列\[れつ\]を保持\[ほじ\]する。delta適用\[てきよう\]と履歴追加\[りれきついか\]は一\[ひと\]つのcommitとする。同\[おな\]じOccurrenceへの更新\[こうしん\]は、前\[まえ\]のafterと次\[つぎ\]のbeforeが一致\[いっち\]し、最後\[さいご\]のafterがFactSetの確定\[かくてい\]resolutionと一致\[いっち\]しなければならない。疎\[そ\]なEntity\/OccurrenceなどのIDをarena indexとして解釈\[かいしゃく\]しない。Custom解決\[かいけつ\]を、通常\[つうじょう\]のlexical lookupの結果\[けっか\]と偽\[いつわ\]って扱\[あつか\]わない。

raw BindingReply codecは、履歴\[りれき\]の型\[かた\]、既存\[きそん\]ID\/namespace、authorityの参照\[さんしょう\]・scope範囲\[はんい\]、明示\[めいじ\]resolution grant、before\/afterの構造\[こうぞう\]、更新\[こうしん\]の連鎖\[れんさ\]と最終結果\[さいしゅうけっか\]を検査\[けんさ\]する。履歴\[りれき\]authorityの予約範囲\[よやくはんい\]が過去\[かこ\]の発行時\[はっこうじ\]に空\[あ\]いていた事実\[じじつ\]、最初\[さいしょ\]のbeforeの真実性\[しんじつせい\]、providerが実行\[じっこう\]された事実\[じじつ\]は、raw返信\[へんしん\]だけから認証\[にんしょう\]しない。外側要求\[そとがわようきゅう\]がない場合\[ばあい\]の正準\[せいじゅん\]targetの所有者\[しょゆうしゃ\]も、raw返信\[へんしん\]だけから認証\[にんしょう\]しない。元要求\[もとようきゅう\]・選択\[せんたく\]Profileに結\[むす\]び付\[つ\]いた実実行\[じつじっこう\]が、意味\[いみ\]proofの根拠\[こんきょ\]である。

FactsRequestのphaseは、Ordinary、Header\(group\)、Body\(header\)を区別\[くべつ\]する。recursiveは全宣言\[ぜんせんげん\]のHeaderを収集\[しゅうしゅう\]してexport候補\[こうほ\]を導入\[どうにゅう\]した後\[あと\]に、各\[かく\]Bodyを実行\[じっこう\]する。HeaderのFactDeltaは新\[あたら\]しい宣言\[せんげん\]Entityと明示\[めいじ\]Export occurrenceを返\[かえ\]し、Reference occurrenceと既存\[きそん\]resolution更新\[こうしん\]は返\[かえ\]さない。初期化式\[しょきかしき\]の参照検査\[さんしょうけんさ\]は、全\[ぜん\]headerが揃\[そろ\]ったBodyで行\[おこな\]う。Headerで省略\[しょうりゃく\]したvisit\/import先\[さき\]のCustomは、BodyでOrdinaryとして実行\[じっこう\]する。propagateで実際\[じっさい\]にheaderを訪\[おとず\]れた宣言\[せんげん\]は、対応\[たいおう\]するBodyを実行\[じっこう\]する。

FactsHeaderはgroup、選択済\[せんたくず\]みProviderRequirement、要求内\[ようきゅうない\]の正準\[せいじゅん\]path\/nodeで表\[あらわ\]したtarget、受理\[じゅり\]したentitiesとexportsのID列\[れつ\]を持\[も\]つ。exportsは、このheaderで受理\[じゅり\]したentitiesの重複\[じゅうふく\]しない部分列\[ぶぶんれつ\]とする。native機械\[きかい\]はrecursive group、実\[じつ\]target、BindingIdごとの受理順\[じゅりじゅん\]にprivate memoを保存\[ほぞん\]し、各\[かく\]headerを一\[ひと\]つのBodyへ対応\[たいおう\]させる。同\[おな\]じplanを別\[べつ\]groupで実行\[じっこう\]した場合\[ばあい\]や、同\[おな\]じtargetを繰\[く\]り返\[かえ\]し訪\[おとず\]れた場合\[ばあい\]にはmemoを共有\[きょうゆう\]しない。Bodyは既存\[きそん\]header IDを参照\[さんしょう\]し、runtimeはそのexport候補\[こうほ\]を再利用\[さいりよう\]する。既存\[きそん\]IDをdelta\.entitiesへ再追加\[さいついか\]する操作\[そうさ\]は、DuplicateIdとして拒否\[きょひ\]する。新\[あたら\]しい予約\[よやく\]IDの別宣言\[べつせんげん\]を、名前\[なまえ\]・Span・scopeの一致\[いっち\]だけでheaderの再発行\[さいはっこう\]と推測\[すいそく\]して拒否\[きょひ\]しない。同名\[どうめい\]の別\[べつ\]Entityは、通常\[つうじょう\]の可視性\[かしせい\]とAmbiguousの規則\[きそく\]に従\[したが\]う。

raw phase検査\[けんさ\]では、groupがauthority\.currentScopeの祖先\[そせん\]（自身\[じしん\]を含\[ふく\]む）であること、Bodyのprovider選択\[せんたく\]・Facts署名\[しょめい\]、targetの正準座標\[せいじゅんざひょう\]、既存\[きそん\]Entity参照\[さんしょう\]、ID列\[れつ\]の重複\[じゅうふく\]とexports包含\[ほうがん\]を検査\[けんさ\]する。別\[べつ\]Foreign rootのgroupを流用\[りゅうよう\]しない。ただしraw FactSetだけでは、scopeと解析対象\[かいせきたいしょう\]の実行履歴\[じっこうりれき\]を認証\[にんしょう\]できない。受信\[じゅしん\]したreceiptを自己申告\[じこしんこく\]の実行\[じっこう\]proofへ昇格\[しょうかく\]させず、hostの授権\[じゅけん\]とnativeの保存\[ほぞん\]memoへ束縛\[そくばく\]する。replyもHeaderの制約\[せいやく\]と発行\[はっこう\]したauthorityへ照合\[しょうごう\]してから適用\[てきよう\]する。

実装試験\[じっそうしけん\]には、通常\[つうじょう\]Custom、recursive Header\/Body、独立\[どくりつ\]Foreign root、停止時\[ていしじ\]の正式\[せいしき\]collector保持\[ほじ\]、同\[おな\]じcallbackへの実\[じつ\]NDF\/CBOR初回受信\[しょかいじゅしん\]とnativeの結果比較\[けっかひかく\]を含\[ふく\]める。同期\[どうき\]codec処理\[しょり\]は、親\[おや\]のBudgetとSourceAdmissionを共有\[きょうゆう\]する。別\[べつ\]processへのFacts quota貸出\[かしだ\]し・実費精算\[じっぴせいさん\]は引\[ひ\]き続\[つづ\]き別実装項目\[べつじっそうこうもく\]であり、この同期比較\[どうきひかく\]を外部\[がいぶ\]processの費用保証\[ひようほしょう\]と扱\[あつか\]わない。

<a name="n-6c65745f6578616d706c65"></a>

<a name="4-非再帰letの完全な規則例"></a>

## 4\. 非再帰\[ひさいき\]letの完全\[かんぜん\]な規則例\[きそくれい\]

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

lambdaはparameter\/bodyの2fieldを持\[も\]ち、scopeはbodyだけに設\[もう\]ける。letrecは、同\[おな\]じnameをinit\/body双方\[そうほう\]のscopeへ導入\[どうにゅう\]する。letrecの名前解決成功\[なまえかいけつせいこう\]は、初期化\[しょきか\]の妥当性\[だとうせい\]や停止性\[ていしせい\]を意味\[いみ\]しない。

<a name="n-7374796c6573"></a>

<a name="5-style"></a>

## 5\. style

style selector class\-nameを登録\[とうろく\]し、head\/self\/field\/captureからsource領域\[りょういき\]を選\[えら\]ぶ。captureはreaderが宣言\[せんげん\]したcapture名\[めい\]とし、未定義\[みていぎ\]field\/captureはcompile errorとする。

同\[おな\]じGrammar\/Style列\[れつ\]へ `selection selector priority` を宣言\[せんげん\]できる。selectorはstyleと同\[おな\]じhead\/self\/field\/captureとし、priorityはNatから損失\[そんしつ\]なく変換\[へんかん\]できるU64とする。省略\[しょうりゃく\]したselectorのpriorityは0とし、同\[おな\]じownerの同一\[どういつ\]selectorへのselection重複\[じゅうふく\]は拒否\[きょひ\]する。範囲\[はんい\]の大小\[だいしょう\]を第一条件\[だいいちじょうけん\]とし、同\[おな\]じ範囲長\[はんいちょう\]の候補\[こうほ\]は、大\[おお\]きいpriority、深\[ふか\]い包含位置\[ほうがんいち\]、宣言順\[せんげんじゅん\]の順\[じゅん\]で選\[えら\]ぶ。styleの既存\[きそん\]arityは変更\[へんこう\]せず、classと優先度\[ゆうせんど\]を別\[べつ\]の列\[れつ\]として保持\[ほじ\]する。

LanguagePackageのForm\/Leafと動的\[どうてき\]HeadShapeは、`SelectionRule { selector, priority }` の順序付\[じゅんじょつ\]き列\[れつ\]を持\[も\]つ。実\[じつ\]Grammar compilerがこの列\[れつ\]を生成\[せいせい\]し、package意味\[いみ\]identityと選択\[せんたく\]Profileのexecution identityへ含\[ふく\]める。未定義\[みていぎ\]field\/capture、重複\[じゅうふく\]selector、U64上限\[じょうげん\]を超\[こ\]えるNatは、元\[もと\]operandか重複宣言\[じゅうふくせんげん\]の位置\[いち\]を持\[も\]つ失敗\[しっぱい\]として返\[かえ\]す。priorityをnative arena配置\[はいち\]から推定\[すいてい\]しない。class名\[めい\]はschema所有\[しょゆう\]IDであり、共通\[きょうつう\]roleへfallbackできる。実際\[じっさい\]の色\[いろ\]をgrammarに固定\[こてい\]せず、binding metadataからdefinition\/reference修飾\[しゅうしょく\]を自動導出\[じどうどうしゅつ\]する。

<a name="n-636f6d70696c65"></a>

<a name="6-compileの出力と検査"></a>

## 6\. compileの出力\[しゅつりょく\]と検査\[けんさ\]

compileは `LanguagePackage{schema, readerPlans, categoryShapes, bindingPlans, stylePlans, extensionRequirements, provenance}` を返\[かえ\]す。意味\[いみ\]データに加\[くわ\]えて、どのgrammar宣言\[せんげん\]から作\[つく\]ったかを保持\[ほじ\]し、grammar自身\[じしん\]にもdefinition jumpを提供\[ていきょう\]する。

ここでのschemaはsurface schemaであり、Doc\:Sentenceなどの意味\[いみ\]schemaと区別\[くべつ\]する。標準\[ひょうじゅん\]profileは `nepl3.syntax.grammar`、`nepl3.syntax.doc`、`nepl3.syntax.math`、`nepl3.syntax.circuit` をsurfaceの所有\[しょゆう\]packageとする。意味\[いみ\]packageは、それぞれ `nepl3.grammar`、`nepl3.doc`、`nepl3.math`、`nepl3.circuit` に分\[わ\]ける。LanguagePackageはpayloadSchemasとして、必要\[ひつよう\]な意味\[いみ\]・provider schemaの実\[じつ\]SchemaRefも宣言\[せんげん\]する。sourceのform名\[めい\]・token名\[めい\]・view名\[めい\]を同一\[どういつ\]descriptorで衝突\[しょうとつ\]させず、compiled metadataにそれぞれのkind identityへの対応\[たいおう\]を持\[も\]つ。

SyntaxNode\.schemaとForeignSyntax\.schemaは、該当\[がいとう\]するsurface schemaを指\[さ\]す。Token\.kindはWordなどのlexical分類\[ぶんるい\]なので、LetなどのSyntaxNode\.kindと同一\[どういつ\]とは限\[かぎ\]らない。form\/leaf宣言\[せんげん\]が両者\[りょうしゃ\]の対応\[たいおう\]とpayload型\[がた\]を定\[さだ\]める。SentenceLiteralの外側\[そとがわ\]shapeはarity 0のままとし、Token\.payloadに入\[はい\]るDoc\:Sentenceは意味\[いみ\]schemaを指\[さ\]す。engineはsurface構造\[こうぞう\]とpayloadの宣言型\[せんげんがた\]を照合\[しょうごう\]するが、読\[よ\]むだけで意味操作\[いみそうさ\]を実行\[じっこう\]しない。

sourceから読\[よ\]むarity 0のleaf（builtin Name\/Text\/Natなども含\[ふく\]む）は、fields\=\[\]とする。値\[あたい\]は、node\.tokenが指\[さ\]すToken\.payloadだけに所有\[しょゆう\]する。builtin引数\[ひきすう\]を親\[おや\]formのAtomへ直接縮約\[ちょくせつしゅくやく\]せず、token\/head\/cover\/Originを持\[も\]つliteral child nodeとして保持\[ほじ\]する。leafのsurface descriptorは空\[くう\]recordとし、token descriptorのpayload fieldが実際\[じっさい\]の値型\[ちがた\]を宣言\[せんげん\]する。Binding\/Styleのfield selectorは、該当\[がいとう\]childのtokenとSourceMapを明示的\[めいじてき\]にたどる。source\-lessの意味\[いみ\]constructor\/printerはdomain意味\[いみ\]モデルで提供\[ていきょう\]し、架空\[かくう\]のToken\.head\/Spanで生成構文\[せいせいこうぶん\]を埋\[う\]めない。

surface descriptorの型名\[かためい\]は、Form\:\/Token\:\/View\:\/Builtin\:などの役割\[やくわり\]を持\[も\]つ名前空間\[なまえくうかん\]で分\[わ\]ける。同\[おな\]じsource kind名\[めい\]に、異\[こと\]なるfield shapeを割\[わ\]り当\[あ\]てない。LanguagePackageの意味\[いみ\]identityはsurface SchemaRef\.digestとは別\[べつ\]であり、reader\/mode\/binding\/style\/extensionなどの実行\[じっこう\]metadataを含\[ふく\]める。名前\[なまえ\]で参照\[さんしょう\]する宣言\[せんげん\]の順序\[じゅんじょ\]は除\[のぞ\]き、skip\/take\/choice\/field\/binding列\[れつ\]の意味順\[いみじゅん\]を保存\[ほぞん\]する。readerの直接\[ちょくせつ\]ReaderId edgeはpackage内\[ない\]ではDAGとし、再帰\[さいき\]は名前付\[なまえつ\]きRefで表\[あらわ\]す。これは完全\[かんぜん\]Grammar構文\[こうぶん\]の有限\[ゆうげん\]ReaderExprと一致\[いっち\]する。standalone ReaderPlanの直接\[ちょくせつ\]cycleとは区別\[くべつ\]し、rule名順\[めいじゅん\]の根\[ね\]からDAGを展開\[てんかい\]する正準形\[せいじゅんけい\]で、共有\[きょうゆう\]・同\[おな\]じ式\[しき\]の複製\[ふくせい\]・arena配置\[はいち\]の違\[ちが\]いを除\[のぞ\]く。Refは名前\[なまえ\]を保持\[ほじ\]するため、再帰\[さいき\]の正準形\[せいじゅんけい\]も有限\[ゆうげん\]である。

extension requirementは既知\[きち\]の型付\[かたつ\]きoperation descriptorと署名\[しょめい\]を照合\[しょうごう\]するが、実行\[じっこう\]callbackの登録\[とうろく\]とは別\[べつ\]である。name\-v1\/trivia\-v1のreader\/v1 adapterはReadRequestを受\[う\]け、対応\[たいおう\]するbuiltinを予約\[よやく\]なしで実行\[じっこう\]してReadReplyを返\[かえ\]す。予約\[よやく\]を持\[も\]つBuiltinRequestとreader\/v1を暗黙互換\[あんもくごかん\]にはせず、Text builtinは明示\[めいじ\]した予約付\[よやくつ\]き入口\[いりぐち\]を使\[つか\]う。facts\/v1などのcustom bindingもparse\/compileだけで自動実行\[じどうじっこう\]しない。hostが実行操作\[じっこうそうさ\]を選\[えら\]んだときにcallbackが未登録\[みとうろく\]なら、MissingProviderで拒否\[きょひ\]する。compiler自身\[じしん\]の宣言名\[せんげんめい\]・selector検査\[けんさ\]を、facts callbackへ丸投\[まるな\]げしない。

facts\/v1の署名\[しょめい\]は、純粋\[じゅんすい\]なnepl3\.engine\.FactsRequest→FactsReplyとする。要求\[ようきゅう\]はParseTree内\[ない\]の明示\[めいじ\]path\/node、既存\[きそん\]FactSet、hostが与\[あた\]えたFactAuthorityを保持\[ほじ\]する。CompleteはFactDelta、Invalid\/Stoppedは任意\[にんい\]のpartial deltaとReportを返\[かえ\]す。全枝\[ぜんえだ\]はReport専用\[せんよう\]のsources\/sourceMapsも所有\[しょゆう\]し、partial\=Noneでも生成\[せいせい\]source上\[じょう\]の診断\[しんだん\]を失\[うしな\]わない。この表\[ひょう\]はdeltaのfacts追加\[ついか\]と独立\[どくりつ\]しており、診断\[しんだん\]のためだけに仮\[かり\]deltaを作\[つく\]らない。Reportの参照\[さんしょう\]は、要求\[ようきゅう\]のtree内\[ない\]の各局所\[かくきょくしょ\]source表\[ひょう\]・既存\[きそん\]FactSet・返却\[へんきゃく\]delta・明示\[めいじ\]Report用\[よう\]source表\[ひょう\]を合\[あ\]わせて解決\[かいけつ\]する。hostの無関係\[むかんけい\]なglobal storeで欠落\[けつらく\]を補\[おぎな\]わない。独立表\[どくりつひょう\]に同\[おな\]じsnapshotを含\[ふく\]む場合\[ばあい\]はidentity\/URIの一致\[いっち\]を要求\[ようきゅう\]し、SourceAdmissionは同\[おな\]じ操作内\[そうさない\]で一度\[いちど\]だけ計上\[けいじょう\]する。

共通\[きょうつう\]facts値\[あたい\]の所有\[しょゆう\]・検査\[けんさ\]はnepl3\.core、解析\[かいせき\]treeを含\[ふく\]む包絡\[ほうらく\]はengineが所有\[しょゆう\]する。標準\[ひょうじゅん\]catalogは実\[じつ\]descriptorを登録\[とうろく\]して宣言\[せんげん\]を照合\[しょうごう\]する。FactsRequest\/Replyの型付\[かたつ\]きadapterは、engineからcore\-owned FoundationValueCodecを介\[かい\]してwireの実\[じつ\]FactSet\/Delta\/Report変換\[へんかん\]を使\[つか\]う。wireからengineへの依存\[いぞん\]は追加\[ついか\]しない。sourceMapsの要素\[ようそ\]はSourceMappingであり、mappings列\[れつ\]をさらに持\[も\]つSourceMapレコードではない。空列\[くうれつ\]では区別\[くべつ\]できなかった旧\[きゅう\]FactsReply schemaの `List<SourceMap>` を、生成\[せいせい\]sourceと実\[じつ\]mapを含\[ふく\]む往復試験\[おうふくしけん\]に合\[あ\]わせて訂正\[ていせい\]した。

hostは自\[みずか\]ら選\[えら\]んだ要求\[ようきゅう\]をFactsRequest\.issueで検査\[けんさ\]し、変更不能\[へんこうふのう\]な元要求\[もとようきゅう\]と解決済\[かいけつず\]みProfileを借用\[しゃくよう\]するCheckedFactsRequestを発行\[はっこう\]する。これは、外部\[がいぶ\]から受信\[じゅしん\]したauthorityの自己申告\[じこしんこく\]を認証\[にんしょう\]する入口\[いりぐち\]ではない。初回受信\[しょかいじゅしん\]のrequest\_decodeは元\[もと\]Rust要求\[ようきゅう\]を持\[も\]たずに、明示\[めいじ\]wire表\[ひょう\]から型\[かた\]・参照\[さんしょう\]・source・tree・authority参照\[さんしょう\]の不変条件\[ふへんじょうけん\]を検査\[けんさ\]し、raw FactsRequestを返\[かえ\]す。授権\[じゅけん\]proofは返\[かえ\]さず、受信\[じゅしん\]hostがtransport認証\[にんしょう\]と操作許可\[そうさきょか\]を確認\[かくにん\]した後\[あと\]だけissueを呼\[よ\]ぶ。未認証\[みにんしょう\]・未許可\[みきょか\]の受信\[じゅしん\]requestから、proofを発行\[はっこう\]してはならない。

別途\[べっと\]request\_from\_valueは送信側\[そうしんがわ\]のecho\/loopback検査\[けんさ\]であり、保存\[ほぞん\]された元要求\[もとようきゅう\]のproofを要求\[ようきゅう\]する。tree\/path\/nodeを正準番号\[せいじゅんばんごう\]へ変換\[へんかん\]した後\[あと\]、要求全体\[ようきゅうぜんたい\]の型付\[かたつ\]き値\[あたい\]を予算付\[よさんつ\]きで照合\[しょうごう\]する。analysis identity、既存\[きそん\]facts、全\[ぜん\]namespace・writable\/import scope・resolution\/relation grant・ID予約\[よやく\]も、一致対象\[いっちたいしょう\]である。初回受信\[しょかいじゅしん\]と、この完全一致検査\[かんぜんいっちけんさ\]を混同\[こんどう\]しない。独立\[どくりつ\]process間\[かん\]の通信認証\[つうしんにんしょう\]と実行授権\[じっこうじゅけん\]は、host transportの責務\[せきむ\]である。

返信\[へんしん\]deltaは保存\[ほぞん\]された元要求\[もとようきゅう\]のexisting\/authorityに対\[たい\]して検査\[けんさ\]し、返信\[へんしん\]から権限\[けんげん\]を得\[え\]ない。Invalid\/Stoppedのpartial\=Noneでも、treeの全\[ぜん\]foreign bundle・既存\[きそん\]facts・任意\[にんい\]delta・Report専用\[せんよう\]source表\[ひょう\]のidentity\/URI整合\[せいごう\]とmap閉包\[へいほう\]を検査\[けんさ\]する。Reportのprimary\/related\/fix\/eventは、この明示閉包\[めいじへいほう\]へ限定\[げんてい\]する。Report\.usageの内部件数整合\[ないぶけんすうせいごう\]と、遠隔処理\[えんかくしょり\]の実消費量\[じつしょうひりょう\]の認証\[にんしょう\]・共有\[きょうゆう\]Budgetへの吸収\[きゅうしゅう\]は別\[べつ\]であり、codecは申告\[しんこく\]Usageを自動加算\[じどうかさん\]しない。この実装\[じっそう\]は静的\[せいてき\]ParseTreeと共通\[きょうつう\]factsの構造\[こうぞう\]・権限境界\[けんげんきょうかい\]の証拠\[しょうこ\]である。facts callbackや語彙\[ごい\]scope解決\[かいけつ\]アルゴリズムの実行\[じっこう\]、Dynamic tree、全\[ぜん\]providerのprocess比較\[ひかく\]の完成\[かんせい\]を意味\[いみ\]しない。

名前\[なまえ\]のbinding selectorが読\[よ\]むleaf payloadは、Textでなければならない。seqはListを返\[かえ\]し、名前\[なまえ\]への暗黙\[あんもく\]の文字列連結\[もじれつれんけつ\]は行\[おこな\]わない。正式\[せいしき\]binding例\[れい\]のNameは、標準\[ひょうじゅん\]name\-v1操作\[そうさ\]を明示\[めいじ\]してTextを得\[え\]る。名前\[なまえ\]として使用\[しよう\]しないNumber readerの `List<Text>` 出力\[しゅつりょく\]は、そのまま保持\[ほじ\]する。payloadSchemasは実際\[じっさい\]のreader型\[がた\]、token payload、view\/style、extension操作\[そうさ\]が参照\[さんしょう\]する型定義\[かたていぎ\]の閉包\[へいほう\]から集\[あつ\]め、未使用\[みしよう\]のregistry登録\[とうろく\]を依存\[いぞん\]へ含\[ふく\]めない。

解決済\[かいけつず\]みProfileはsurface、意味\[いみ\]、reader\/providerの全\[ぜん\]descriptorと計算済\[けいさんず\]みdigestを登録\[とうろく\]する。未生成\[みせいせい\]のpackageへ架空\[かくう\]のdigestを置\[お\]かず、同\[おな\]じSchemaRefに異\[こと\]なるfield shapeを割\[わ\]り当\[あ\]てない。Grammar compileはsurface descriptorを作\[つく\]る責務\[せきむ\]を持\[も\]ち、Doc\/Math\/Circuitの意味\[いみ\]schemaを勝手\[かって\]に再生成\[さいせいせい\]・上書\[うわが\]きしない。

必須検査\[ひっすけんさ\]は、次\[つぎ\]のとおりとする。

- 未定義\[みていぎ\]category\/mode\/reader\/namespace\/provider。
- 重複\[じゅうふく\]kind\/field\/spellingと、field型\[がた\]の不一致\[ふいっち\]。
- readerの空反復\[くうはんぷく\]と、進捗\[しんちょく\]のない再帰\[さいき\]。
- 未読\[みどく\]fieldへの構文\[こうぶん\]context依存\[いぞん\]。
- bindingでの非名前\[ひなまえ\]fieldの使用\[しよう\]と、範囲外\[はんいがい\]のstyle selector。
- foreign aliasの不足\[ふそく\]と、provider署名\[しょめい\]の不一致\[ふいっち\]。

Grammarが生成\[せいせい\]したdescriptorと、同\[おな\]じ契約\[けいやく\]をRustで直接構築\[ちょくせつこうちく\]したdescriptorは、同\[おな\]じengineで動\[うご\]く。埋\[う\]め込\[こ\]まれたproviderをdescriptorの固定\[こてい\]IRへ変換\[へんかん\]できなくても、その呼出\[よびだ\]し参照\[さんしょう\]を保持\[ほじ\]する。

<a name="n-73796e7461785f656e7669726f6e6d656e74"></a>

<a name="7-構文環境を更新する宣言"></a>

## 7\. 構文環境\[こうぶんかんきょう\]を更新\[こうしん\]する宣言\[せんげん\]

通常\[つうじょう\]のvalue bindingは、名前解決環境\[なまえかいけつかんきょう\]だけを変\[か\]える。関数値\[かんすうち\]をbindしても、その名前\[なまえ\]のarityを自動変更\[じどうへんこう\]しない。

外部\[がいぶ\]HeadProviderは `shape(head, existingContext)` と `context_for_child(head, index, completedChildren)` を提供\[ていきょう\]できる。後者\[こうしゃ\]は、既\[すで\]に読了\[どくりょう\]した子\[こ\]だけを参照\[さんしょう\]する。schemaを変更\[へんこう\]する宣言\[せんげん\]の作用範囲\[さようはんい\]は、その宣言\[せんげん\]が導入\[どうにゅう\]したbodyの部分木\[ぶぶんぎ\]とし、復帰時\[ふっきじ\]には元\[もと\]へ戻\[もど\]す。

動的操作\[どうてきそうさ\]の共通要求\[きょうつうようきゅう\]はHeadCall、返信\[へんしん\]はHeadReplyとする。要求\[ようきゅう\]にはsession\/call\/operation\/Profile\/execution identity、EntryContext、環境\[かんきょう\]のopaque参照\[さんしょう\]、既読\[きどく\]head、ShapeまたはChildContextの種別\[しゅべつ\]を保持\[ほじ\]する。既知\[きち\]のstatic spellingを最優先\[さいゆうせん\]とし、その次\[つぎ\]に当該\[とうがい\]alias\/categoryのshapeを呼\[よ\]び、Shape\(None\)のときだけleaf候補\[こうほ\]へ進\[すす\]む。固定\[こてい\]shapeのfieldsがarityを定\[さだ\]め、child iへの要求\[ようきゅう\]は、それより前\[まえ\]に読了\[どくりょう\]したi個\[こ\]のrootだけを順\[じゅん\]に保持\[ほじ\]する。childContext返信\[へんしん\]は登録済\[とうろくず\]みEntryContextを選\[えら\]び、固定\[こてい\]field数\[すう\]やNodeRef\/ForeignSyntaxのslot形状\[けいじょう\]を変更\[へんこう\]しない。

投影\[とうえい\]は完全\[かんぜん\]SourceSnapshotを含\[ふく\]まず、ProjectedSpan\(source\,start\,end\)と、その範囲\[はんい\]のUTF\-8 bytesを持\[も\]つSourceWindowを使\[つか\]う。これはcore Spanと別\[べつ\]の位置型\[いちがた\]であり、部分\[ぶぶん\]bytesだけから元全文\[もとぜんぶん\]digestの正\[ただ\]しさを証明\[しょうめい\]しない。hostは元\[もと\]snapshotから窓\[まど\]を捕捉\[ほそく\]して発行\[はっこう\]callに束縛\[そくばく\]し、受信側\[じゅしんがわ\]は窓内\[まどない\]の相対\[そうたい\]UTF\-8境界\[きょうかい\]・長\[なが\]さ・参照整合\[さんしょうせいごう\]を検査\[けんさ\]する。ProjectedSyntaxは、foreignも含\[ふく\]む独立\[どくりつ\]flat node arenaを持\[も\]つ。元\[もと\]bundleのEnvironmentRefはopaque metadataであり、投影全体\[とうえいぜんたい\]の環境表\[かんきょうひょう\]をlookupする権限\[けんげん\]ではない。

既読\[きどく\]Token\.payloadは、compoundを含\[ふく\]むNdfValueを保持\[ほじ\]する。その意味値\[いみち\]にSourceContentなどの形\[かたち\]をしたrecordが含\[ふく\]まれても、自動\[じどう\]source解決\[かいけつ\]・admission・環境権限\[かんきょうけんげん\]へ昇格\[しょうかく\]しない。投影\[とうえい\]の契約\[けいやく\]は、parser自身\[じしん\]が未読\[みどく\]child、未消費\[みしょうひ\]source bytes、Originや環境\[かんきょう\]resourceの自動閉包\[じどうへいほう\]を追加提供\[ついかていきょう\]しないことである。任意\[にんい\]readerがpayloadへ何\[なに\]を書\[か\]くかまで含\[ふく\]む、完全\[かんぜん\]な情報非干渉\[じょうほうひかんしょう\]の証明\[しょうめい\]ではない。

HeadReplyは新\[しん\]sourceを生成\[せいせい\]しない。ProjectedReportの位置\[いち\]だけをProjectedSpanで表\[あらわ\]し、hostが保存\[ほぞん\]windowへの包含\[ほうがん\]を照合\[しょうごう\]して、元\[もと\]snapshotから通常\[つうじょう\]Spanへ復元\[ふくげん\]する。primary\/related\/fix\/eventは同\[おな\]じ窓\[まど\]へ限定\[げんてい\]する。型\[かた\]・Fix期待\[きたい\]digest・編集非重複\[へんしゅうひじゅうふく\]・同一\[どういつ\]SourceIdの前提\[ぜんてい\]revision・Report件数\[けんすう\]などは、共通\[きょうつう\]Report検査\[けんさ\]を使\[つか\]う。部分\[ぶぶん\]sourceから短縮\[たんしゅく\]SourceSnapshotや未検査\[みけんさ\]Spanを作\[つく\]らない。HeadContinuationはparserの進捗\[しんちょく\]と待機\[たいき\]tokenを所有\[しょゆう\]するhost側\[がわ\]の継続\[けいぞく\]であり、providerへ渡\[わた\]すのはHeadCallだけである。

native resume\_headのcaller SourceStoreは元\[もと\]の主入力\[しゅにゅうりょく\]snapshotを解決\[かいけつ\]し、保存済\[ほぞんず\]みidentity・内容\[ないよう\]・URIと一致\[いっち\]しなければならない。欠落\[けつらく\]や不一致\[ふいっち\]によって待機要求\[たいきようきゅう\]を消費\[しょうひ\]しない。補助\[ほじょ\]・生成\[せいせい\]sourceは保存済\[ほぞんず\]み閉包\[へいほう\]を利用\[りよう\]でき、caller storeへの全件再登録\[ぜんけんさいとうろく\]は要求\[ようきゅう\]しない。返信\[へんしん\]Reportは、保存\[ほぞん\]された明示窓\[めいじまど\]と宣言\[せんげん\]source閉包\[へいほう\]の規則\[きそく\]で検査\[けんさ\]する。

今回\[こんかい\]の配布\[はいふ\]4言語\[げんご\]は、ソース本体\[ほんたい\]の途中\[とちゅう\]でschemaを自己変更\[じこへんこう\]しない。Grammar sourceをcompileして別\[べつ\]の入力\[にゅうりょく\]へ適用\[てきよう\]する順序\[じゅんじょ\]を、標準経路\[ひょうじゅんけいろ\]とする。動的\[どうてき\]HeadProvider経路\[けいろ\]は契約試験用\[けいやくしけんよう\]の局所構文例\[きょくしょこうぶんれい\]で実装\[じっそう\]・検証\[けんしょう\]し、単\[たん\]にAPIだけを残\[のこ\]して未実装\[みじっそう\]にしない。

<a name="n-70657273697374656e745f73656c656374696f6e"></a>

<a name="永続する解析選択と検査範囲"></a>

### 永続\[えいぞく\]する解析選択\[かいせきせんたく\]と検査範囲\[けんさはんい\]

ParseTreeはProfile digest、SyntaxBundle、bundleごとのNodeSelectionとRecoveryEntryを保持\[ほじ\]する。foreignへのpathの各\[かく\]NodeRefはその段階\[だんかい\]の所有\[しょゆう\]bundleに属\[ぞく\]し、field名\[めい\]で次\[つぎ\]のForeignSyntaxを選\[えら\]ぶ。各到達\[かくとうたつ\]nodeには選択\[せんたく\]がちょうど一\[ひと\]つ必要\[ひつよう\]であり、static form\/leaf\/readのindexはEntryContextのaliasとpackage executionDigestで所有\[しょゆう\]を固定\[こてい\]する。既知\[きち\]formの原文\[げんぶん\]spellingをleafへ付\[つ\]け替\[か\]えてarityを変\[か\]えたり、親\[おや\]ReadSpecと異\[こと\]なるalias\/category\/modeの子\[こ\]を置\[お\]いたりしてはならない。ListOfのtailは、同\[おな\]じspineのcontextとReadSpecを保持\[ほじ\]する。

Dynamicの完成選択\[かんせいせんたく\]は固定\[こてい\]HeadShape、providerの操作参照\[そうささんしょう\]、field順\[じゅん\]のchildContextsを保存\[ほぞん\]する。完成\[かんせい\]treeのchildContextsはfieldsと同数\[どうすう\]であり、各\[かく\]実\[じつ\]childの選択\[せんたく\]と一致\[いっち\]する。callbackは固定\[こてい\]arityやNodeRef\/ForeignSyntaxのslot形状\[けいじょう\]を変更\[へんこう\]できず、解決済\[かいけつず\]みProfileの実\[じつ\]package\/aliasからcontextを選\[えら\]ぶ。進行中\[しんこうちゅう\]frameは、既読\[きどく\]か開始済\[かいしず\]みのchildまでの確定\[かくてい\]prefixを保持\[ほじ\]し、完成\[かんせい\]treeと同\[おな\]じ完全性条件\[かんぜんせいじょうけん\]を先取\[さきど\]りしない。

HeadCallの初回\[しょかい\]portable受信\[じゅしん\]は、元\[もと\]Rust要求\[ようきゅう\]や全文\[ぜんぶん\]SourceStoreを前提\[ぜんてい\]にしない。ProjectedSpanはsnapshotに対\[たい\]する絶対\[ぜったい\]byte範囲\[はんい\]であり、受信側\[じゅしんがわ\]は窓内\[まどない\]の相対位置\[そうたいいち\]を使\[つか\]ってUTF\-8境界\[きょうかい\]を検査\[けんさ\]する。同\[おな\]じSourceId\/revisionへ異\[こと\]なるdigestを宣言\[せんげん\]することはできず、同一\[どういつ\]SourceRefの重\[かさ\]なる窓\[まど\]では共通区間\[きょうつうくかん\]のbytesが一致\[いっち\]しなければならない。flat projected arenaの全参照\[ぜんさんしょう\]・root数\[すう\]・schema field形状\[けいじょう\]・到達性\[とうたつせい\]・循環\[じゅんかん\]と、全位置\[ぜんいち\]の窓閉包\[まどへいほう\]を検査\[けんさ\]する。coverの元入力\[もとにゅうりょく\]への忠実性\[ちゅうじつせい\]と全文\[ぜんぶん\]digestの認証\[にんしょう\]は、この内部整合検査\[ないぶせいごうけんさ\]からは証明\[しょうめい\]されない。既読\[きどく\]payloadをsource検索権限\[けんさくけんげん\]へ昇格\[しょうかく\]させず、窓\[まど\]を短縮\[たんしゅく\]SourceSnapshotとして登録\[とうろく\]しない。

遠隔費用\[えんかくひよう\]を扱\[あつか\]う包絡\[ほうらく\]は、HeadDelegation\(call\, limits\)とHeadDelivery\(reply\, usage\)とする。HeadCallのnative意味\[いみ\]は変更\[へんこう\]しない。hostは保存\[ほぞん\]した発行\[はっこう\]callの全窓\[ぜんまど\]を実\[じつ\]snapshotと照合\[しょうごう\]し、共有\[きょうゆう\]SourceAdmissionへ全文\[ぜんぶん\]をonce計上\[けいじょう\]する。その後\[あと\]、非公開\[ひこうかい\]の発行\[はっこう\]proofに親\[おや\]Limitsと累積\[るいせき\]Usage基準\[きじゅん\]を固定\[こてい\]し、親\[おや\]Budgetを排他的\[はいたてき\]に借用\[しゃくよう\]する。子\[こ\]のSourceBytes上限\[じょうげん\]は発行窓\[はっこうまど\]の区間\[くかん\]unionであり、親\[おや\]の未使用\[みしよう\]SourceBytesではない。その他\[ほか\]の加算資源\[かさんしげん\]は、親\[おや\]の残余\[ざんよ\]からhostが明示\[めいじ\]した送受信\[そうじゅしん\]・検査用\[けんさよう\]reserveを差\[さ\]し引\[ひ\]いて貸\[か\]し出\[だ\]す。予約容量\[よやくようりょう\]を実消費\[じつしょうひ\]Usageへ記録\[きろく\]しない。Depthには親\[おや\]の絶対上限\[ぜったいじょうげん\]とHeadCall\.depthBaseを使\[つか\]う。受信窓\[じゅしんまど\]ledgerはその子操作\[こそうさ\]だけに属\[ぞく\]し、重複\[じゅうふく\]・包含\[ほうがん\]・交差\[こうさ\]する同一\[どういつ\]snapshot区間\[くかん\]をonce計上\[けいじょう\]する。

親\[おや\]のNDF変換\[へんかん\]、実\[じつ\]wireエンコード、返信\[へんしん\]デコードは発行\[はっこう\]proofのtransport入口\[いりぐち\]で実行\[じっこう\]し、貸出分\[かしだしぶん\]を除\[のぞ\]いた一時\[いちじ\]ceiling内\[ない\]で処理前\[しょりまえ\]に計上\[けいじょう\]する。reserveを超\[こ\]えれば停止\[ていし\]する。Resultの成功\[せいこう\]・失敗\[しっぱい\]にかかわらず外側\[そとがわ\]Limitsへ復帰\[ふっき\]するが、停止理由\[ていしりゆう\]は維持\[いじ\]する。子側\[こがわ\]の初回\[しょかい\]wire\/framingも、認証済\[にんしょうず\]みgrantかhostが選択\[せんたく\]したより狭\[せま\]い上限内\[じょうげんない\]で実測\[じっそく\]し、その加算資源\[かさんしげん\]を子操作\[こそうさ\]の上限\[じょうげん\]から先\[さき\]に差\[さ\]し引\[ひ\]く。受信\[じゅしん\]したLimits自体\[じたい\]を、実行許可\[じっこうきょか\]にしない。

HeadDelivery\.usageは最終返信\[さいしゅうへんしん\]エンコードの直前\[ちょくぜん\]に採取\[さいしゅ\]し、framingとその時点\[じてん\]までの子操作\[こそうさ\]を含\[ふく\]める。エンコード自身\[じしん\]の費用\[ひよう\]を、同\[おな\]じcounterへ再帰的\[さいきてき\]に埋\[う\]め込\[こ\]まない。host transportが観測\[かんそく\]・認証\[にんしょう\]したエンコード後\[ご\]の全使用量\[ぜんしようりょう\]を発行上限\[はっこうじょうげん\]と照合\[しょうごう\]して一度\[いちど\]だけsettleし、Work・Allocationなどを親\[おや\]へ記録\[きろく\]する。親\[おや\]が既\[すで\]に停止\[ていし\]していても、観測済\[かんそくず\]みの子実費\[こじっぴ\]は記録\[きろく\]し、元\[もと\]の停止理由\[ていしりゆう\]を保持\[ほじ\]する。子\[こ\]の終了\[しゅうりょう\]と観測量\[かんそくりょう\]が確定\[かくてい\]したら、不正返信\[ふせいへんしん\]のデコードより先\[さき\]に精算\[せいさん\]する。これにより、後続\[こうぞく\]の非停止拒否\[ひていしきょひ\]でも元\[もと\]の待機要求\[たいきようきゅう\]を再試行\[さいしこう\]できるようにする。精算後\[せいさんご\]は未使用\[みしよう\]の貸出分\[かしだしぶん\]だけを親\[おや\]へ戻\[もど\]し、accept時\[じ\]の返信検査\[へんしんけんさ\]もその親\[おや\]Budgetで計上\[けいじょう\]する。

未精算\[みせいさん\]proofを破棄\[はき\]した場合\[ばあい\]は親\[おや\]をCancelledにし、まだ動作\[どうさ\]し得\[う\]る子\[こ\]への貸出\[かしだ\]しを再利用\[さいりよう\]しない。窓\[まど\]SourceBytesだけは保存発行\[ほぞんはっこう\]proofにより再加算\[さいかさん\]せず、Depthはmaxで合流\[ごうりゅう\]する。Report\.usageは、発行基準\[はっこうきじゅん\]から返信採取点\[へんしんさいしゅてん\]の累積値\[るいせきち\]へ変換\[へんかん\]する。独立\[どくりつ\]processの通信認証\[つうしんにんしょう\]・停止制御\[ていしせいぎょ\]・メータリング方式\[ほうしき\]はhost実装\[じっそう\]の責務\[せきむ\]であり、NDF型検査\[かたけんさ\]や受信自己申告\[じゅしんじこしんこく\]だけでは、その証拠\[しょうこ\]を発行\[はっこう\]しない。子\[こ\]が最終\[さいしゅう\]エンコード前\[まえ\]に停止\[ていし\]した場合\[ばあい\]も、hostは確定\[かくてい\]した実観測量\[じつかんそくりょう\]を精算\[せいさん\]できる。ただし、停止\[ていし\]Budgetから成功\[せいこう\]packetを作\[つく\]れるとはしない。

現在\[げんざい\]のParseTree\.validateは静的選択\[せいてきせんたく\]・参照所有\[さんしょうしょゆう\]・回復構文\[かいふくこうぶん\]・payload型\[がた\]に加\[くわ\]え、Dynamicのalias\/categoryへ登録\[とうろく\]された実\[じつ\]provider、操作署名\[そうさしょめい\]、固定\[こてい\]shape、各\[かく\]child contextとの整合\[せいごう\]を検査\[けんさ\]する。既知\[きち\]の静的\[せいてき\]formに一致\[いっち\]するheadをDynamicとして受理\[じゅり\]せず、未登録\[みとうろく\]providerや整合\[せいごう\]しない選択\[せんたく\]は拒否\[きょひ\]する。この検査\[けんさ\]はcallbackの実行履歴\[じっこうりれき\]を認証\[にんしょう\]せず、任意\[にんい\]のreader\/providerを再実行\[さいじっこう\]して全\[ぜん\]payloadがその出力\[しゅつりょく\]であることを証明\[しょうめい\]するものでもない。nativeの動的選択実行\[どうてきせんたくじっこう\]、portable構造\[こうぞう\]の往復\[おうふく\]、provider初回受信\[しょかいじゅしん\]と返信照合\[へんしんしょうごう\]は、それぞれ実行試験\[じっこうしけん\]で区別\[くべつ\]する。全体\[ぜんたい\]のP03完成\[かんせい\]を、単独\[たんどく\]のtree検査\[けんさ\]から推定\[すいてい\]しない。

Unparsedの先頭\[せんとう\]をtokenとして既\[すで\]に読\[よ\]んでいる場合\[ばあい\]は、そのTokenRefとheadを保持\[ほじ\]し、Unparsed coverがheadを包含\[ほうがん\]することを検査\[けんさ\]する。未知\[みち\]arityを推測\[すいそく\]して通常\[つうじょう\]leafへ変\[か\]えることと、既読\[きどく\]tokenのpayload\/view\/triviaを保存\[ほぞん\]することは別\[べつ\]である。tokenを得\[え\]られないNoMatchなどでは、token\/headをともにNoneとする。どちらの場合\[ばあい\]も、不明範囲\[ふめいはんい\]と理由\[りゆう\]はRecoveryEntryに残\[のこ\]す。

<a name="n-626f6f747374726170"></a>

<a name="8-grammar自身のbootstrap"></a>

## 8\. Grammar自身\[じしん\]のbootstrap

検査済\[けんさず\]みParseTreeは、同\[おな\]じ不変\[ふへん\]なSyntaxBundleに対\[たい\]して成立\[せいりつ\]したsource・型\[かた\]・参照\[さんしょう\]の検査\[けんさ\]proofを、借用\[しゃくよう\]で公開\[こうかい\]できる。Rustの `ValidatedParseTree::syntax` は `validate_with_sources` の結果\[けっか\]を保持\[ほじ\]して返\[かえ\]し、同\[おな\]じ操作内\[そうさない\]でのlower準備\[じゅんび\]に再検査\[さいけんさ\]・再計上\[さいけいじょう\]を要求\[ようきゅう\]しない。raw入力\[にゅうりょく\]、変更\[へんこう\]したtree、別\[べつ\]のsnapshotへ、このproofを付\[つ\]け替\[か\]えてはならない。環境内容\[かんきょうないよう\]digestの一致\[いっち\]やDSLの意味検査\[いみけんさ\]まで証明\[しょうめい\]したものとは扱\[あつか\]わず、それらの検査\[けんさ\]は引\[ひ\]き続\[つづ\]き必要\[ひつよう\]とする。proofを使\[つか\]う下流操作自身\[かりゅうそうさじしん\]の処理量\[しょりりょう\]とsource admissionは省略\[しょうりゃく\]しない。

完全\[かんぜん\]な文法表\[ぶんぽうひょう\]からseed descriptorを生成\[せいせい\]し、通常\[つうじょう\]engineで自分\[じぶん\]のlanguage定義\[ていぎ\]を読\[よ\]む。seedのarity手書\[てが\]き表\[ひょう\]とsourceを、独立\[どくりつ\]に二重管理\[にじゅうかんり\]しない。生成\[せいせい\]した全表\[ぜんひょう\]・source・seedの対応\[たいおう\]を機械検査\[きかいけんさ\]する。

Grammar packageの利用者\[りようしゃ\]が機能\[きのう\]を拡張\[かくちょう\]しても、共通\[きょうつう\]engineを書\[か\]き換\[か\]えない。編集対象\[へんしゅうたいしょう\]の文法\[ぶんぽう\]を変更\[へんこう\]したら依存\[いぞん\]package\/queryを無効化\[むこうか\]し、schema digestの異\[こと\]なる結果\[けっか\]を混用\[こんよう\]しない。

<a name="n-776974686d6f6465"></a>

<a name="withmode-の所属と復帰"></a>

### WithModeの所属\[しょぞく\]と復帰\[ふっき\]

`withmode M R` は、Rの構文\[こうぶん\]rootを所有\[しょゆう\]するpackageのmode Mを選\[えら\]ぶ。Builtin・Local・ListOfのrootは現在\[げんざい\]packageに属\[ぞく\]し、Foreignのrootはaliasが指\[さ\]すguestに属\[ぞく\]する。入\[い\]れ子\[こ\]のWithModeは内側\[うちがわ\]のrootまで所属\[しょぞく\]をたどり、同\[おな\]じrootに複数\[ふくすう\]overrideがある場合\[ばあい\]は最\[もっと\]も内側\[うちがわ\]を使\[つか\]う。無効\[むこう\]な外側\[そとがわ\]mode宣言\[せんげん\]も黙殺\[もくさつ\]せず、対応\[たいおう\]ownerのmodeとして検査\[けんさ\]する。

`withmode Code (listof (foreign Guest Sentence))` はhostのcons\/nil spineをCodeで読\[よ\]み、各\[かく\]guest要素\[ようそ\]はSentenceの既定\[きてい\]modeを使\[つか\]う。`listof (withmode GuestCode (foreign Guest Sentence))` はhost listの既定\[きてい\]modeを保\[たも\]ち、guest要素\[ようそ\]のrootだけGuestCodeを使\[つか\]う。hostの同名\[どうめい\]modeは、guest modeの代用\[だいよう\]にならない。Builtinは選択\[せんたく\]modeのskipを使\[つか\]い、値\[あたい\]のreaderは指定\[してい\]builtinを直接使\[ちょくせつつか\]う。

overrideは、そのrootへ適用\[てきよう\]する。通常\[つうじょう\]formの各\[かく\]childは、自身\[じしん\]のReadSpecで新\[あたら\]しいcontextを選\[えら\]び、親\[おや\]のoverrideを暗黙継承\[あんもくけいしょう\]しない。同\[おな\]じlistのtailはspine modeを継続\[けいぞく\]し、foreign終了時\[しゅうりょうじ\]は保存\[ほぞん\]したhost contextへ戻\[もど\]る。実\[じつ\]prefix試験\[しけん\]では、hostとguestに同名\[どうめい\]で異\[こと\]なるreaderのmodeを置\[お\]き、所属\[しょぞく\]・list tail・child・復帰\[ふっき\]を検査\[けんさ\]する。

現在\[げんざい\]の `LanguagePackage::check` は、局所\[きょくしょ\]metadataとshapeの検査\[けんさ\]である。Foreignのalias\/category\/modeは、解決済\[かいけつず\]みProfileで検査\[けんさ\]する対象\[たいしょう\]として残\[のこ\]る。package意味\[いみ\]digest、解決済\[かいけつず\]みEntryContext\/Profile、実\[じつ\]parse\/Grammar bootstrapを、この局所\[きょくしょ\]proofから推定\[すいてい\]しない。
