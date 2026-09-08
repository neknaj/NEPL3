<!-- Generated from doc/spec/08&#45;editor.nepld; renderer nepl3-tools.markdown-annotated/2; source SHA-256 d326323331b335da48a044e35a92d2bd89b64bc4fde794131af4ab17ad3f91b6; alias input SHA-256 0e85d24ed98a8e151b582442218b17d3d4f191755c9528582080443fca21746b; document digest cc476675f363da36a8425749ddefd74e2799d2ec64bbc0c456d11af84b82752a. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="08-共通editor-serviceと診断"></a>

# 08\. 共通\[きょうつう\]editor serviceと診断\[しんだん\]

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

一度宣言\[いちどせんげん\]したgrammar\/binding\/style情報\[じょうほう\]からeditor機能\[きのう\]を導出\[どうしゅつ\]する。専用\[せんよう\]parserも同\[おな\]じfactsを返\[かえ\]して支援\[しえん\]を受\[う\]ける。共通分類\[きょうつうぶんるい\]をプログラミング言語\[げんご\]の要素\[ようそ\]に限定\[げんてい\]しない。

<a name="n-736e617073686f74"></a>

<a name="1-analysissnapshot"></a>

## 1\. AnalysisSnapshotの入力\[にゅうりょく\]と同一性\[どういつせい\]

入力\[にゅうりょく\]はSourceSnapshot集合\[しゅうごう\]、profile\/package revision集合\[しゅうごう\]、明示的\[めいじてき\]なresource\/schema環境\[かんきょう\]、解析\[かいせき\]options。結果\[けっか\]はParsed tree、内部\[ないぶ\]views、Scope\/Entity\/Occurrence、typed Relations、Diagnostics、ExpectedAt、Dependencies。

全結果\[ぜんけっか\]にSnapshotIdとanalysis keyを付\[つ\]ける。keyには言語\[げんご\]\/reader\/providerのrevision、context、操作\[そうさ\]optionsを含\[ふく\]む。spanのある結果\[けっか\]を意味値\[いみち\]だけのcacheから再利用\[さいりよう\]しない。

Binding の prepared request は AnalysisKey の treeDigest、profileDigest、executionDigest、requestDigest を固定\[こてい\]する。treeDigest は局所\[きょくしょ\]nodeを正準化\[せいじゅんか\]した完全\[かんぜん\]な ParseTree の NDF\/1 CBORに基\[もと\]づき、明示\[めいじ\]sourceのidentity・URI・元\[もと\]bytes、payload、環境\[かんきょう\]、各選択\[かくせんたく\]contextを含\[ふく\]む。profileDigest は解決済\[かいけつず\]みProfileの意味\[いみ\]identityで、選択\[せんたく\]schema、provider実装\[じっそう\]、resource、既定\[きてい\]limitsを含\[ふく\]む。executionDigest はalias名順\[めいじゅん\]の実\[じつ\]package execution identity表\[ひょう\]に基\[もと\]づき、同\[おな\]じ意味\[いみ\]でも異\[こと\]なるarena配置\[はいち\]を区別\[くべつ\]する。requestDigest はanalysis ID、型付\[かたつ\]きBindingOptions、有効\[ゆうこう\]な操作\[そうさ\]Limitsを含\[ふく\]む。現在\[げんざい\]のBindingOptionsは空\[くう\]recordであり、未実装\[みじっそう\]の意味\[いみ\]optionを黙\[だま\]って受\[う\]け入\[い\]れない。

treeDigest・executionDigest・requestDigestは `SHA-256(domain || canonical NDF/1 CBOR(value))` とし、tree・execution・requestのdomainはそれぞれ `nepl3.analysis.tree/1`、`nepl3.analysis.execution/1`、`nepl3.analysis.request/1` に終端\[しゅうたん\]zero byteを付\[つ\]ける。profileDigestは既存\[きそん\]の解決済\[かいけつず\]みProfile identityアルゴリズムに従\[したが\]う。codecは正準化\[せいじゅんか\]とhashのWork・割当\[わりあて\]・出力量\[しゅつりょくりょう\]を操作\[そうさ\]Budgetへ計上\[けいじょう\]する。SourceSnapshotを内包\[ないほう\]した値\[あたい\]のhash計算\[けいさん\]だけで新\[あら\]たなsource lookup権限\[けんげん\]を付与\[ふよ\]しない。

prepareは明示的\[めいじてき\]に渡\[わた\]されたBudgetとSourceAdmissionで要求\[ようきゅう\]を検査\[けんさ\]し、keyを算出\[さんしゅつ\]する。実行\[じっこう\]LimitsはProfileの上限\[じょうげん\]を超\[こ\]えられず、executeのBudgetのLimitsは要求\[ようきゅう\]と完全一致\[かんぜんいっち\]しなければならない。prepareと実行\[じっこう\]は独立\[どくりつ\]した操作予算\[そうさよさん\]を使\[つか\]うことも、同一\[どういつ\]pipelineの累積\[るいせき\]Budget\/SourceAdmissionを共有\[きょうゆう\]することもできるが、使用済\[しようず\]み量\[りょう\]を払\[はら\]い戻\[もど\]さない。keyは有効上限\[ゆうこうじょうげん\]を保持\[ほじ\]し、開始時\[かいしじ\]の残量\[ざんりょう\]やtransportの観測\[かんそく\]Usageを認証\[にんしょう\]する値\[あたい\]ではない。途中停止結果\[とちゅうていしけっか\]から完了\[かんりょう\]proofを生成\[せいせい\]しない。

初回\[しょかい\]BindingRequest decoderは元\[もと\]のRust要求\[ようきゅう\]を持\[も\]たず、明示\[めいじ\]sourceを持\[も\]つ受信\[じゅしん\]treeと選択済\[せんたくず\]みProfileからkeyを再計算\[さいけいさん\]する。自己申告\[じこしんこく\]keyやLimitsを通信認証\[つうしんにんしょう\]・hostの実行許可\[じっこうきょか\]へ昇格\[しょうかく\]しない。受信\[じゅしん\]raw要求\[ようきゅう\]のprepareでも再計算\[さいけいさん\]・一致検査\[いっちけんさ\]を行\[おこな\]い、その間\[あいだ\]に変更\[へんこう\]されたtree\/contextや条件\[じょうけん\]を拒否\[きょひ\]する。BoundBindingReplyは全結果枝\[ぜんけっかえだ\]のkeyと通常\[つうじょう\]BindingReplyを保持\[ほじ\]する。raw返信\[へんしん\]decoderの一致検査\[いっちけんさ\]は意味\[いみ\]proofの発行\[はっこう\]ではなく、完了\[かんりょう\]proofは実\[じつ\]BindingPlanの実行\[じっこう\]からのみ得\[え\]る。

BoundBindingReply\.for\_source はhostが現在要求\[げんざいようきゅう\]しているAnalysisKeyとの完全一致\[かんぜんいっち\]とSourceRefのrevision\/digestを検査\[けんさ\]してから、完了\[かんりょう\]したBindingAnalysisを公開\[こうかい\]する。これは定義\[ていぎ\]・参照検索\[さんしょうけんさく\]の前提\[ぜんてい\]となる照合入口\[しょうごういりぐち\]であり、positionからの対象選択\[たいしょうせんたく\]、定義\[ていぎ\]ジャンプ、参照検索\[さんしょうけんさく\]、renameのアルゴリズム完成\[かんせい\]を意味\[いみ\]しない。既存\[きそん\]のunkeyed analyzeのFactSet\.analysis\_idだけを、editor用\[よう\]のstale判定\[はんてい\]として使\[つか\]わない。

<a name="n-64657269766564"></a>

<a name="2-自動で得られる機能"></a>

## 2\. 自動\[じどう\]で得\[え\]られる機能\[きのう\]

schemaのform\/leaf\/fieldとreader captureから構文\[こうぶん\]ハイライト、構造的\[こうぞうてき\]selection、expected categoryの補完\[ほかん\]を提供\[ていきょう\]する。binding\/reference\/export\/importから定義\[ていぎ\]ジャンプ、参照検索\[さんしょうけんさく\]、未定義\[みていぎ\]\/重複診断\[じゅうふくしんだん\]、scope内\[ない\]の候補補完\[こうほほかん\]を提供\[ていきょう\]する。name fieldとenclosing rangeからoutlineを作\[つく\]る。ドキュメントfieldを宣言\[せんげん\]すればhoverに出\[だ\]す。

型推論\[かたすいろん\]、回路幅\[かいろはば\]、数値計算結果等\[すうちけいさんけっかなど\]はdomain factsの追加\[ついか\]で精度\[せいど\]を上\[あ\]げる。grammarだけから任意\[にんい\]domainの意味\[いみ\]を推測\[すいそく\]したと主張\[しゅちょう\]しない。

手書\[てが\]きreaderにはview\/factsの同\[おな\]じcontractを要求\[ようきゅう\]する。内部\[ないぶ\]viewなしならtoken全体\[ぜんたい\]のfallbackだけ。すべてのcustom readerへ精密\[せいみつ\]な内部支援\[ないぶしえん\]を自動生成\[じどうせいせい\]できるとはしない。

<a name="n-726567696f6e"></a>

<a name="3-regionの選択"></a>

## 3\. regionの選択\[せんたく\]

source positionから、同一\[どういつ\]snapshot上\[じょう\]で包含\[ほうがん\]する最小\[さいしょう\]のfield\/elementを選\[えら\]ぶ。grammar priority、最内側\[さいうちがわ\]、宣言順\[せんげんじゅん\]でtie\-breakを固定\[こてい\]する。sentence内\[ない\]のreadingを問\[と\]い合\[あ\]わせたらsentence token全体\[ぜんたい\]よりreading viewを優先\[ゆうせん\]する。

現在\[げんざい\]の `analysis::region::regions` はprepared Binding入力\[にゅうりょく\]の検査済\[けんさず\]みtree\/Profileと、明示\[めいじ\]ReaderFactBatch sidecarを受\[う\]ける。`RegionKey` は既存\[きそん\]AnalysisKeyとsidecarのdigestの組\[くみ\]である。sidecarは `RegionSidecar { facts: Option<List<ReaderFactBatch>> }` を正準\[せいじゅん\]NDF\/1 CBORへ符号化\[ふごうか\]し、`SHA-256("nepl3.region.reader-facts/1\0" || bytes)` で識別\[しきべつ\]する。Foreign pathとnodeはtreeの正準\[せいじゅん\]DFS番号\[ばんごう\]へ写\[うつ\]し、batch\/fact\/triviaの順序\[じゅんじょ\]は保存\[ほぞん\]する。NoneはSyntaxOnly、Some（空列\[くうれつ\]を含\[ふく\]む）はReaderFactsとして区別\[くべつ\]し、入力\[にゅうりょく\]しなかったCaptureをView名\[めい\]から推測\[すいそく\]しない。

sidecarのentryは所有\[しょゆう\]bundle\/nodeの保存\[ほぞん\]selectionと照合\[しょうごう\]する。nodeのあるbatchのtriviaはToken\.leadingTriviaの完全\[かんぜん\]な再掲\[さいけい\]であり、別\[べつ\]tokenをSkippedと名乗\[なの\]って混入\[こんにゅう\]できない。Captureは所有\[しょゆう\]bundleの宣言\[せんげん\]SourceMapを使\[つか\]い、token headまたは正式\[せいしき\]なskip triviaへの包含\[ほうがん\]を検査\[けんさ\]する。Presentation\/Relationは通常\[つうじょう\]reader契約\[けいやく\]どおり明示\[めいじ\]source\/schemaの整合\[せいごう\]を検査\[けんさ\]し、消費範囲\[しょうひはんい\]への包含条件\[ほうがんじょうけん\]を追加\[ついか\]しない。所有\[しょゆう\]bundleのsource\/map不足\[ふそく\]をForeignの別表\[べっぴょう\]で補\[おぎな\]わない。これらは構造\[こうぞう\]と出自参照\[しゅつじさんしょう\]の検査\[けんさ\]であり、raw sidecarが実\[じつ\]providerから返\[かえ\]されたことを認証\[にんしょう\]しない。

RegionRequestはRegionKey、SourceRef、byte offsetを持\[も\]つ。key\/revision\/digestとUTF\-8 scalar境界\[きょうかい\]を照合\[しょうごう\]し、範囲\[はんい\]は半開区間\[はんかいくかん\]とする。返\[かえ\]すSourceRegionは正準\[せいじゅん\]bundle\/node、field\/element番号\[ばんごう\]またはtoken所有\[しょゆう\]Viewのroot\/field\/child path、表示\[ひょうじ\]source span、元\[もと\]のlogical span、priority、depth、宣言順\[せんげんじゅん\]、PresentationClassを保持\[ほじ\]する。nodeなしの末尾\[まつび\]\/skip\-only batchはnode\=Noneであり、架空\[かくう\]のroot所有\[しょゆう\]nodeを作\[つく\]らない。通常\[つうじょう\]node\/field\/headのdepthはその宣言\[せんげん\]ownerの深\[ふか\]さ、Viewはそのownerに内部\[ないぶ\]field\/childの深\[ふか\]さを加\[くわ\]える。子\[こ\]nodeは親\[おや\]fieldより内側\[うちがわ\]となる。

同\[おな\]じ長\[なが\]さならpriority降順\[こうじゅん\]、depth降順\[こうじゅん\]、宣言順\[せんげんじゅん\]で選\[えら\]ぶ。宣言順\[せんげんじゅん\]はrootからfield順\[じゅん\]のDFSとViewの宣言順\[せんげんじゅん\]で、arenaの保存順\[ほぞんじゅん\]とは区別\[くべつ\]する。共有\[きょうゆう\]DAGの同\[おな\]じ対象\[たいしょう\]には最初\[さいしょ\]の宣言順\[せんげんじゅん\]と実際\[じっさい\]の最深到達経路\[さいしんとうたつけいろ\]を保持\[ほじ\]する。回復\[かいふく\]nodeはRecoveryとして区別\[くべつ\]し、UnknownHeadのUnparsed範囲\[はんい\]へ推測\[すいそく\]した子\[こ\]を追加\[ついか\]しない。Missingの空\[くう\]anchorやEOFを直前\[ちょくぜん\]の名前\[なまえ\]に吸着\[きゅうちゃく\]させない。

表示用\[ひょうじよう\]SourceMap投影\[とうえい\]は領域\[りょういき\]ownerの明示表\[めいじひょう\]だけをたどる。隣接\[りんせつ\]Exact区間\[くかん\]は同\[おな\]じsourceとbyte変位\[へんい\]の連続被覆\[れんぞくひふく\]として結合\[けつごう\]し、区間分割\[くかんぶんかつ\]や表順\[ひょうじゅん\]で結果\[けっか\]を変\[か\]えない。完全\[かんぜん\]Exact、部分被覆\[ぶぶんひふく\]ExactFragment、保守的\[ほしゅてき\]なTransformedを区別\[くべつ\]し、多義的\[たぎてき\]な表示候補\[ひょうじこうほ\]を保持\[ほじ\]する。この表示情報\[ひょうじじょうほう\]はrenameの一意逆写像\[いちいぎゃくしゃぞう\]や位置単位\[いちたんい\]の名前解決\[なまえかいけつ\]を証明\[しょうめい\]しない。Foreign fieldの子位置\[こいち\]には子\[こ\]bundleの表\[ひょう\]を使\[つか\]い、guestだけにあるmapをhost nodeへ適用\[てきよう\]しない。map探索\[たんさく\]の深\[ふか\]さは構文\[こうぶん\]\/View ownerの深\[ふか\]さに合成\[ごうせい\]して計量\[けいりょう\]する。

RegionReplyは全表示\[ぜんひょうじ\]regionと選択\[せんたく\]index、またはtyped Invalid\/Stopped、Usage\-only Report、明示\[めいじ\]source閉包\[へいほう\]を返\[かえ\]す。停止\[ていし\]で候補途中列\[こうほとちゅうれつ\]を公開\[こうかい\]しない。初回\[しょかい\]tree\/sidecar\/request復元\[ふくげん\]は元\[もと\]Rust要求\[ようきゅう\]やambient sourceによる補完\[ほかん\]を必要\[ひつよう\]としない。返信\[へんしん\]のCompleteは対応\[たいおう\]する準備入力\[じゅんびにゅうりょく\]から構造選択\[こうぞうせんたく\]を予算付\[よさんつ\]きで再計算\[さいけいさん\]して照合\[しょうごう\]する。これはreader実行認証\[じっこうにんしょう\]、Binding完了\[かんりょう\]proof、別\[べつ\]process費用計測\[ひようけいそく\]の代\[か\]わりではない。一般\[いっぱん\]regionを用\[もち\]いるdefinition\/references入口\[いりぐち\]は次節\[じせつ\]のRegionQueryであり、既存\[きそん\]のOccurrenceベース入口\[いりぐち\]の動作\[どうさ\]を変更\[へんこう\]しない。

型付\[かたつ\]きregion列\[れつ\]の構築\[こうちく\]はWork・Nodes・Depth・AllocationUnitsとsource受入\[うけいれ\]を計上\[けいじょう\]し、まだ生成\[せいせい\]していないserializationのbyte数\[すう\]をOutputBytesへ推計加算\[すいけいかさん\]しない。native返信\[へんしん\]のReportはその計算終了時点\[けいさんしゅうりょうじてん\]のUsageである。実\[じつ\]NDF\/1 CBOR encodeは同\[おな\]じ操作予算\[そうさよさん\]へ実出力\[じつしゅつりょく\]のOutputBytesを生成前\[せいせいまえ\]に課金\[かきん\]する。従\[したが\]ってnativeのOutputBytes上限\[じょうげん\]0で型付\[かたつ\]き結果\[けっか\]が得\[え\]られることと、実\[じつ\]encodeがOutputLimitで停止\[ていし\]することは別\[べつ\]の境界\[きょうかい\]であり、返信中\[へんしんちゅう\]のUsageをその後\[ご\]のserializationや外部\[がいぶ\]transportまで含\[ふく\]む総費用\[そうひよう\]と扱\[あつか\]わない。

highlightは内部\[ないぶ\]regionへ分割\[ぶんかつ\]し、leafに近\[ちか\]い具体的\[ぐたいてき\]classを優先\[ゆうせん\]する。overlapを持\[も\]つ元\[もと\]データは保存\[ほぞん\]するが、LSPへは非重複\[ひじゅうふく\]・source順\[じゅん\]・単一行\[たんいつぎょう\]に正規化\[せいきか\]したspan列\[れつ\]を返\[かえ\]す。色\[いろ\]はthemeに委\[ゆだ\]ねる。

<a name="n-72656c6174696f6e73"></a>

<a name="4-definitionとrelations"></a>

## 4\. definitionとrelationsの扱\[あつか\]い

Occurrenceベースの `analysis::query::query` は、完了\[かんりょう\]したBoundBindingReplyとQueryRequest\(key\, SourceRef\, byte offset\, kind\)を受\[う\]ける。SourceRefのrevision\/digestと現在要求\[げんざいようきゅう\]するkeyを照合\[しょうごう\]し、UTF\-8 scalar境界\[きょうかい\]を検査\[けんさ\]する。範囲\[はんい\]は半開区間\[はんかいくかん\]で、終端\[しゅうたん\]byteや空白\[くうはく\]を直前\[ちょくぜん\]の名前\[なまえ\]へ吸着\[きゅうちゃく\]させない。包含\[ほうがん\]するOccurrence\.spanのうち最小\[さいしょう\]のものを選\[えら\]び、同長\[どうちょう\]なら確定\[かくてい\]factsの発行順\[はっこうじゅん\]を使\[つか\]う。

一般\[いっぱん\]regionを使\[つか\]う `analysis::region::query::query` は、同\[おな\]じ準備入力\[じゅんびにゅうりょく\]と実\[じつ\]Binding完了結果\[かんりょうけっか\]に対\[たい\]するRegionQueryRequest\(region\:RegionRequest\, kind\:QueryKind\)を受\[う\]ける。まず3節\[せつ\]のpriorityを含\[ふく\]む構造選択\[こうぞうせんたく\]を実行\[じっこう\]し、選択\[せんたく\]したSourceRegionを保持\[ほじ\]する。確定\[かくてい\]BindingBundleScopeからそのbundleのrootを解決\[かいけつ\]し、そのrootのOccurrenceだけを候補\[こうほ\]にする。Foreign fieldは宣言元\[せんげんもと\]のRegionTargetを保持\[ほじ\]したまま、内容\[ないよう\]のguest bundleに対応\[たいおう\]するrootと局所\[きょくしょ\]mapを用\[もち\]いる。単\[たん\]なるsource座標一致\[ざひょういっち\]でhost\/guestの名前\[なまえ\]を混\[ま\]ぜない。

要求位置\[ようきゅういち\]の一\[いち\]Unicode scalar範囲\[はんい\]と選択\[せんたく\]regionのlogical範囲\[はんい\]を、所有\[しょゆう\]bundleの宣言\[せんげん\]Exact\/Transformed関係\[かんけい\]でOccurrenceのsourceへ対応付\[たいおうづ\]け、両者\[りょうしゃ\]とOccurrence\.spanが重\[かさ\]なるものを残\[のこ\]す。Exactはbyte差分\[さぶん\]と被覆区間\[ひふくくかん\]、Transformedは宣言\[せんげん\]された範囲全体\[はんいぜんたい\]の対応\[たいおう\]である。query用\[よう\]の対応\[たいおう\]は双方向\[そうほうこう\]の関係\[かんけい\]であり、非一意\[ひいちい\]なExactやTransformedの候補\[こうほ\]を黙\[だま\]って一\[ひと\]つへ絞\[しぼ\]らず、rename用\[よう\]の一意\[いちい\]な逆変換\[ぎゃくへんかん\]や編集権限\[へんしゅうけんげん\]とは区別\[くべつ\]する。source不足\[ふそく\]・古\[ふる\]いrevision・不正\[ふせい\]scalar境界\[きょうかい\]・停止\[ていし\]は型付\[かたつ\]き失敗\[しっぱい\]となる。

対応\[たいおう\]に使\[つか\]うmapはそのSyntaxBundleの局所表\[きょくしょひょう\]と、実\[じつ\]BindingBundleScope\.customSourceMapsが指\[さ\]すCustom受理行\[じゅりぎょう\]だけである。Customの生成\[せいせい\]sourceもBinding結果\[けっか\]の明示\[めいじ\]source閉包\[へいほう\]で解決\[かいけつ\]する。Binding全体\[ぜんたい\]の平坦\[へいたん\]なsourceMaps表\[ひょう\]を全\[ぜん\]bundleへ適用\[てきよう\]せず、同\[おな\]じ生成\[せいせい\]sourceをhost\/guestが共有\[きょうゆう\]していても別\[べつ\]ownerのmapによる候補追加\[こうほついか\]を許\[ゆる\]さない。

RegionQueryReplyのCompleteは選択\[せんたく\]regionとOccurrenceごとのDefinition\/References結果列\[けっかれつ\]を持\[も\]つ。列\[れつ\]は確定\[かくてい\]factsの発行順\[はっこうじゅん\]を保\[たも\]ち、各結果\[かくけっか\]は保存\[ほぞん\]された最終\[さいしゅう\]ResolutionのEntity IDを使\[つか\]う。region\=None、regionはあるがOccurrenceがない、未解決\[みかいけつ\]\/Open入力\[にゅうりょく\]、複数\[ふくすう\]Occurrence、Ambiguousの複数\[ふくすう\]Entity、位置\[いち\]を持\[も\]たないEntityを区別\[くべつ\]する。名前\[なまえ\]の再探索\[さいたんさく\]や発行時\[はっこうじ\]stageの書換\[かきか\]えでCustomの更新\[こうしん\]を消\[け\]さない。途中候補\[とちゅうこうほ\]はInvalid\/Stoppedで公開\[こうかい\]せず、使用済\[しようず\]み資源\[しげん\]を払\[はら\]い戻\[もど\]さない。ReportのUsageはnative処理終了時点\[しょりしゅうりょうじてん\]であり、後続\[こうぞく\]serializationの実出力課金\[じつしゅつりょくかきん\]とは分\[わ\]ける。

初回\[しょかい\]RegionQuery受信\[じゅしん\]は、明示\[めいじ\]BindingRequest\/ReaderFactBatch sidecarを復元\[ふくげん\]して準備\[じゅんび\]し、同\[おな\]じ選択\[せんたく\]Profileで実\[じつ\]Bindingを実行\[じっこう\]した後\[あと\]に要求\[ようきゅう\]と返信\[へんしん\]を照合\[しょうごう\]する。返信\[へんしん\]Completeのcodec検査\[けんさ\]はこの入力\[にゅうりょく\]と完了結果\[かんりょうけっか\]からqueryを予算付\[よさんつ\]きで再計算\[さいけいさん\]し、候補\[こうほ\]・選択\[せんたく\]・source閉包\[へいほう\]を比較\[ひかく\]する。raw BindingResultやbundleScopes行\[ぎょう\]、自己申告\[じこしんこく\]RegionKeyから意味\[いみ\]proof・通信認証\[つうしんにんしょう\]・provider実行許可\[じっこうきょか\]を発行\[はっこう\]しない。

QuerySelectionは元\[もと\]のOccurrenceId・span・確定\[かくてい\]ReferenceResolutionとopenInputを保持\[ほじ\]する。対象\[たいしょう\]なしはNone、未解決\[みかいけつ\]とOpen namespaceのfree inputはopenInputで区別\[くべつ\]し、DeferredもUnresolvedに潰\[つぶ\]さない。定義候補\[ていぎこうほ\]は確定\[かくてい\]resolutionのEntity ID列\[れつ\]に従\[したが\]い、曖昧\[あいまい\]な全候補\[ぜんこうほ\]とその順序\[じゅんじょ\]を維持\[いじ\]する。位置\[いち\]を持\[も\]たないEntityはIDとlocation\=Noneを返\[かえ\]し、仮\[かり\]のsource位置\[いち\]を作\[つく\]らない。locationがある場合\[ばあい\]はURI、定義全体\[ていぎぜんたい\]range、任意\[にんい\]の名前\[なまえ\]selectionを分離\[ぶんり\]する。

Referencesは選択\[せんたく\]した各\[かく\]Entity IDごとに確定\[かくてい\]Occurrenceを検索\[けんさく\]する。既定\[きてい\]ではResolvedのReferenceだけを返\[かえ\]す。ReferenceOptionsはDefinition・Import・Export role、およびそのEntityをAmbiguous候補\[こうほ\]に含\[ふく\]むOccurrenceをそれぞれ明示的\[めいじてき\]に追加\[ついか\]できる。曖昧\[あいまい\]な出現\[しゅつげん\]はambiguous\=trueで区別\[くべつ\]し、同綴\[どうつづ\]りの別\[べつ\]Entityを混\[ま\]ぜない。通常\[つうじょう\]BindingのImportが子\[こ\]のexport候補\[こうほ\]を可視\[かし\]にする操作\[そうさ\]と、providerが明示的\[めいじてき\]に返\[かえ\]すImport roleのOccurrenceは別\[べつ\]であり、queryが新\[あたら\]しいOccurrenceを捏造\[ねつぞう\]しない。Customの更新後\[こうしんご\]も元\[もと\]stageから名前検索\[なまえけんさく\]をやり直\[なお\]さず、確定\[かくてい\]resolutionを用\[もち\]いる。

QueryReplyはDefinition\/References\/Invalid\/StoppedとReport、参照位置\[さんしょういち\]に必要\[ひつよう\]な宣言\[せんげん\]sourcesを持\[も\]つ。現\[げん\]queryは新\[あら\]たな診断\[しんだん\]・eventを発行\[はっこう\]せず、解析診断\[かいせきしんだん\]は元\[もと\]BindingReplyに残\[のこ\]す。query失敗\[しっぱい\]はtyped原因\[げんいん\]または元\[もと\]StopReasonを保持\[ほじ\]し、候補途中列\[こうほとちゅうれつ\]を成功結果\[せいこうけっか\]として公開\[こうかい\]しない。使用済\[しようず\]みBudgetとSourceAdmissionは払\[はら\]い戻\[もど\]さない。sourcesはsource ID・revision・digest順\[じゅん\]に正準化\[せいじゅんか\]する。

QueryRequestの初回\[しょかい\]decoderは元\[もと\]Rust要求\[ようきゅう\]を必要\[ひつよう\]としないが、自己申告\[じこしんこく\]keyから完了\[かんりょう\]したBindingAnalysisを作\[つく\]らない。実\[じつ\]queryはhostが保持\[ほじ\]する同\[どう\]keyの意味\[いみ\]proofを使\[つか\]う。返信\[へんしん\]decoderは対応\[たいおう\]する要求\[ようきゅう\]、role options、候補\[こうほ\]ID列\[れつ\]、範囲\[はんい\]・URIと明示\[めいじ\]source閉包\[へいほう\]を検査\[けんさ\]し、ambient sourceで欠損\[けっそん\]を埋\[う\]めない。これはraw結果\[けっか\]の構造検査\[こうぞうけんさ\]であり、通信認証\[つうしんにんしょう\]やリモートの解決結果\[かいけつけっか\]の意味証明\[いみしょうめい\]ではない。

Customを使\[つか\]うprepared実行\[じっこう\]にも同\[おな\]じkey\/有効\[ゆうこう\]Limitsの検査\[けんさ\]を適用\[てきよう\]する。hostのauthorizeが参照\[さんしょう\]する設定\[せってい\]・権限方針\[けんげんほうしん\]・明示\[めいじ\]resourceを変\[か\]えて結果\[けっか\]が変\[か\]わる場合\[ばあい\]、hostはその入力変更\[にゅうりょくへんこう\]をProfile resourcesやschemaで宣言\[せんげん\]したrequest options等\[など\]のidentityへ反映\[はんえい\]し、旧\[きゅう\]keyの結果\[けっか\]を再利用\[さいりよう\]してはならない。keyは実行時\[じっこうじ\]FactAuthorityそのものを認証\[にんしょう\]せず、同\[おな\]じkeyのまま任意\[にんい\]のhost設定変更\[せっていへんこう\]を安全\[あんぜん\]に許\[ゆる\]す値\[あたい\]でもない。

definition結果\[けっか\]はoriginSelection、targetUri、targetRange、targetSelectionを分離\[ぶんり\]する。entityのnameだけ選択\[せんたく\]しつつ定義全体\[ていぎぜんたい\]も示\[しめ\]せる。sourceが配布\[はいふ\]packageの場合\[ばあい\]は読\[よ\]み取\[と\]り専用\[せんよう\]virtual documentとして公開\[こうかい\]できる。

CorrespondsToには「対応文\[たいおうぶん\]へ移動\[いどう\]」、Originには「生成元\[せいせいもと\]へ移動\[いどう\]」を提供\[ていきょう\]し、通常\[つうじょう\]の定義\[ていぎ\]ジャンプとは別\[べつ\]relationとして扱\[あつか\]う。多義的\[たぎてき\]な解決\[かいけつ\]は複数候補\[ふくすうこうほ\]を返\[かえ\]し、単語一致\[たんごいっち\]で一\[ひと\]つに決\[き\]めない。

renameは同\[おな\]じEntityを指\[さ\]すOccurrenceだけを対象\[たいしょう\]にする。新名\[しんめい\]の適格性\[てきかくせい\]、予約語\[よやくご\]、shadowing、scope内衝突\[ないしょうとつ\]、外側\[そとがわ\]の参照\[さんしょう\]の捕捉\[ほそく\]まで再検査\[さいけんさ\]する。SourceMapの逆変換\[ぎゃくへんかん\]が一意\[いちい\]でない場所\[ばしょ\]はRenameNotInvertible。全\[ぜん\]TextEditはrevision付\[つ\]きでatomicに返\[かえ\]す。

現在\[げんざい\]の rename は `RenameRequest(key, source, offset, newName, writable)` に対\[たい\]する二段操作\[にだんそうさ\]である。対象選択\[たいしょうせんたく\]は上記\[じょうき\]の名前\[なまえ\]Occurrence規則\[きそく\]を使\[つか\]う。writable はhostが許可\[きょか\]した元\[もと\]snapshot集合\[しゅうごう\]であり、受信\[じゅしん\]したSourceRefやURIだけから編集権限\[へんしゅうけんげん\]を発行\[はっこう\]しない。未選択\[みせんたく\]・未解決\[みかいけつ\]・曖昧\[あいまい\]・Deferred・位置\[いち\]なしを型付\[かたつ\]きで区別\[くべつ\]して拒否\[きょひ\]する。予約\[よやく\]headは対象\[たいしょう\]EntityのFactNamespace\.schemaを所有\[しょゆう\]する選択済\[せんたくず\]みpackageのForm\.spellingと照合\[しょうごう\]する。別\[べつ\]schemaの言語\[げんご\]だけに存在\[そんざい\]するheadを予約語\[よやくご\]へ加\[くわ\]えない。最終的\[さいしゅうてき\]な名前\[なまえ\]のreader適格性\[てきかくせい\]と構文選択\[こうぶんせんたく\]は実再\[じっさい\]parseで検査\[けんさ\]する。

prepareは元\[もと\]の実\[じつ\]parse完了結果\[かんりょうけっか\]と、その同\[おな\]じimmutable treeを借用\[しゃくよう\]するprepared Binding要求\[ようきゅう\]、同\[どう\]keyの完了解析\[かんりょうかいせき\]を要求\[ようきゅう\]する。private RenameDraftには元\[もと\]revisionへの編集候補\[へんしゅうこうほ\]と候補\[こうほ\]snapshot集合\[しゅうごう\]を保持\[ほじ\]するが、成功\[せいこう\]transactionや編集列\[へんしゅうれつ\]の公開入口\[こうかいいりぐち\]を与\[あた\]えない。draftはrename操作専用\[そうさせんよう\]SourceAdmissionを所有\[しょゆう\]し、同\[おな\]じ有効\[ゆうこう\]LimitsのBudgetをprepare・再\[さい\]parse・再解析\[さいかいせき\]・acceptの間\[あいだ\]、排他的\[はいたてき\]に借用\[しゃくよう\]する。失敗\[しっぱい\]やdraft破棄\[はき\]で使用済\[しようず\]み費用\[ひよう\]を払\[はら\]い戻\[もど\]さない。別候補\[べつこうほ\]は同\[おな\]じ元\[もと\]revisionから分岐\[ぶんき\]でき、共有\[きょうゆう\]の元\[もと\]SourceStoreを候補作成時\[こうほさくせいじ\]に変更\[へんこう\]しない。

逆写像\[ぎゃくしゃぞう\]は宣言\[せんげん\]SourceMapを全探索\[ぜんたんさく\]し、同一\[どういつ\]source・revision・digest上\[じょう\]の一意\[いちい\]な連続範囲\[れんぞくはんい\]へ達\[たっ\]するExact対応\[たいおう\]だけを使\[つか\]う。隣接\[りんせつ\]するExact区間\[くかん\]は同\[おな\]じbyte変位\[へんい\]で穴\[あな\]なく全対象範囲\[ぜんたいしょうはんい\]を被覆\[ひふく\]するなら結合\[けつごう\]でき、表順\[ひょうじゅん\]や区間分割\[くかんぶんかつ\]で結果\[けっか\]を変\[か\]えない。異\[こと\]なるsourceや変位\[へんい\]への多義性\[たぎせい\]、被覆穴\[ひふくあな\]、Transformed、Composite\/Synthetic origin、cycleは拒否\[きょひ\]する。quoteやescapeから復号\[ふくごう\]された意味名\[いみめい\]と元\[もと\]bytesが異\[こと\]なる場合\[ばあい\]、明示逆\[めいじぎゃく\]encoderがないままquoteを消\[け\]す置換\[ちかん\]を行\[おこな\]わない。探索\[たんさく\]・範囲比較\[はんいひかく\]・複製\[ふくせい\]はWork\/Depth\/Nodes\/Allocationの対象\[たいしょう\]となる。

ここで探索\[たんさく\]する宣言表\[せんげんひょう\]は対象\[たいしょう\]Entityまたは各\[かく\]Occurrenceのnamespace rootに対応\[たいおう\]する実\[じつ\]bundleの局所\[きょくしょ\]mapと、そのbundleのcustomSourceMapsが指\[さ\]す受理\[じゅり\]mapに限\[かぎ\]る。平坦\[へいたん\]なFactSet\.sourceMaps全体\[ぜんたい\]を編集先\[へんしゅうさき\]の根拠\[こんきょ\]にしない。各\[かく\]Occurrenceは自分\[じぶん\]のownerで逆写像\[ぎゃくしゃぞう\]し、同\[おな\]じEntityを指\[さ\]すという理由\[りゆう\]だけで対象\[たいしょう\]Entityのmapを流用\[りゅうよう\]しない。前向\[まえむ\]き導出\[どうしゅつ\]も各元編集\[かくもとへんしゅう\]と対応\[たいおう\]ownerの組\[くみ\]ごとに独立\[どくりつ\]した閉包\[へいほう\]をたどり、最終的\[さいしゅうてき\]な編集列\[へんしゅうれつ\]だけを統合\[とうごう\]する。再解析後\[さいかいせきご\]のsource位置対応\[いちたいおう\]はold\/newそれぞれのEntity・Occurrence owner、構文位置\[こうぶんいち\]は各正準\[かくせいじゅん\]bundle ownerで検査\[けんさ\]する。guestだけが宣言\[せんげん\]したmapでhost名\[めい\]のwritable rootを置\[お\]き換\[か\]えることはできない。

元\[もと\]への編集\[へんしゅう\]からExact対応\[たいおう\]を前向\[まえむ\]きにも適用\[てきよう\]し、影響\[えいきょう\]する派生\[はせい\]snapshotの次\[つぎ\]revisionと内部編集\[ないぶへんしゅう\]を作\[つく\]る。派生側\[はせいがわ\]の未対応\[みたいおう\]byteは保存\[ほぞん\]する。再\[さい\]parseが返\[かえ\]す全宣言\[ぜんせんげん\]sourceはdraftの指定\[してい\]snapshot集合\[しゅうごう\]と、ID・revision・digest・URI・全\[ぜん\]bytesで一致\[いっち\]しなければならない。mapのtargetであることはsnapshot全体\[ぜんたい\]の照合免除\[しょうごうめんじょ\]にならない。hostがreaderの生成\[せいせい\]sourceを予約\[よやく\]する場合\[ばあい\]も、この候補\[こうほ\]identityを使\[つか\]う。公開\[こうかい\]する編集\[へんしゅう\]は許可\[きょか\]された元\[もと\]snapshotへの列\[れつ\]だけで、派生側\[はせいがわ\]の内部編集\[ないぶへんしゅう\]を追加\[ついか\]の書込権限\[かきこみけんげん\]として外部\[がいぶ\]へ渡\[わた\]さない。

acceptは候補\[こうほ\]の実\[じつ\]parse完了結果\[かんりょうけっか\]と同\[おな\]じimmutable treeに対\[たい\]するprepared要求\[ようきゅう\]・完了解析\[かんりょうかいせき\]を照合\[しょうごう\]する。Profileの意味\[いみ\]identity、provider\/package実行\[じっこう\]identity、options・analysis ID・有効\[ゆうこう\]Limitsを維持\[いじ\]し、候補全\[こうほぜん\]source、構文\[こうぶん\]kind\/field\/選択\[せんたく\]context、編集\[へんしゅう\]に対応\[たいおう\]する位置\[いち\]と意味\[いみ\]payloadを検査\[けんさ\]する。Entityの対応\[たいおう\]は宣言位置\[せんげんいち\]・scope\/namespace・意味名\[いみめい\]に基\[もと\]づき、opaque IDの数値一致\[すうちいっち\]だけで決\[き\]めない。対象外\[たいしょうがい\]の名前\[なまえ\]も含\[ふく\]め、Resolved・全\[ぜん\]Ambiguous候補\[こうほ\]・Unresolved\/free input・Deferred・Customの確定\[かくてい\]resolutionが対応\[たいおう\]していることを検査\[けんさ\]する。後続\[こうぞく\]scopeで名前検索\[なまえけんさく\]をやり直\[なお\]してCustomの判断\[はんだん\]を上書\[うわが\]きしない。衝突\[しょうとつ\]・捕捉\[ほそく\]・構文変更\[こうぶんへんこう\]・別要求\[べつようきゅう\]との組替\[くみか\]え・停止\[ていし\]では編集列\[へんしゅうれつ\]を一部\[いちぶ\]も公開\[こうかい\]しない。

nativeのCompletedParseは実\[じつ\]ParseSessionのread\/resume\/reserve\/resume\_headがCompleteを返\[かえ\]した場合\[ばあい\]だけ発行\[はっこう\]する所有\[しょゆう\]proofであり、待機\[たいき\]・回復\[かいふく\]・停止\[ていし\]は通常\[つうじょう\]のParseReplyを保持\[ほじ\]する。元\[もと\]と候補\[こうほ\]のprepared treeがこのproofの同一\[どういつ\]immutable treeを借用\[しゃくよう\]する条件\[じょうけん\]はnative内部\[ないぶ\]の保証\[ほしょう\]で、pointerやRust ABIを公開\[こうかい\]schemaへ露出\[ろしゅつ\]しない。raw treeの取出\[とりだ\]しはproofを消費\[しょうひ\]し、raw ParseTreeやNDF decodeから再発行\[さいはっこう\]できない。将来\[しょうらい\]の別言語\[べつげんご\]・別\[べつ\]process parserも、認証\[にんしょう\]された発行要求\[はっこうようきゅう\]と実\[じつ\]parse実行結果\[じっこうけっか\]を結\[むす\]ぶ同等\[どうとう\]のhost境界\[きょうかい\]を必要\[ひつよう\]とする。現在\[げんざい\]のraw rename request\/reply codecはschema、要求\[ようきゅう\]key、書込対象\[かきこみたいしょう\]、old digest、編集非重複\[へんしゅうひじゅうふく\]、元\[もと\]source閉包\[へいほう\]を検査\[けんさ\]するが、通信認証\[つうしんにんしょう\]やparse実行証明\[じっこうしょうめい\]を発行\[はっこう\]しない。

RenameReplyのCompleteは新\[しん\]AnalysisKeyと全\[ぜん\]TextEdit、Invalid\/Stoppedは編集\[へんしゅう\]なしを返\[かえ\]す。Reportは実消費\[じつしょうひ\]Usageを保持\[ほじ\]し、この操作自身\[そうさじしん\]は新規\[しんき\]diagnostic\/eventを発行\[はっこう\]しない。再\[さい\]parse\/解析\[かいせき\]の正式\[せいしき\]Reportはそれぞれの返\[かえ\]り値\[ち\]に保持\[ほじ\]する。受信\[じゅしん\]したUsageの内部整合\[ないぶせいごう\]と外部処理費用\[がいぶしょりひよう\]の認証\[にんしょう\]は別\[べつ\]の責務\[せきむ\]である。最終適用\[さいしゅうてきよう\]は呼出側\[よびだしがわ\]の元\[もと\]SourceStoreに対\[たい\]し、base revision・old digest付\[つ\]きのatomic applyで行\[おこな\]う。一般\[いっぱん\]region selector、任意\[にんい\]encoderによる意味名\[いみめい\]の逆変換\[ぎゃくへんかん\]、増分再解析\[ぞうぶんさいかいせき\]、外部\[がいぶ\]parser実行\[じっこう\]の認証\[にんしょう\]transportはこの段階\[だんかい\]の完成範囲\[かんせいはんい\]に含\[ふく\]めない。

<a name="n-7265636f76657279"></a>

<a name="5-不完全入力"></a>

## 5\. 不完全入力\[ふかんぜんにゅうりょく\]

構文\[こうぶん\]エラーで文書全体\[ぶんしょぜんたい\]を失\[うしな\]わない。treeにMissing\(expected\, anchor\)、Unexpected\(span\)、Unparsed\(range\,reason\)を持\[も\]てる。通常\[つうじょう\]の成功\[せいこう\]nodeと区別\[くべつ\]し、checked値\[ち\]へ混入\[こんにゅう\]させない。

EOFで既知\[きち\]arityの子\[こ\]が不足\[ふそく\]するとMissingを作\[つく\]る。現在\[げんざい\]のcategoryに不適合\[ふてきごう\]だがancestorの明示的同期位置\[めいじてきどうきいち\]に適合\[てきごう\]するtokenは消費\[しょうひ\]せずMissingを挿入\[そうにゅう\]できる。同期根拠\[どうきこんきょ\]がなければUnexpectedとして消費\[しょうひ\]するか、その範囲\[はんい\]をUnparsedとして残\[のこ\]す。どちらを選\[えら\]ぶかはpackageのRecoveryPlanに固定\[こてい\]する。

arity不明\[ふめい\]のheadの子\[こ\]の数\[かず\]を推測\[すいそく\]しない。該当\[がいとう\]rangeをUnparsedにし、既\[すで\]に確定\[かくてい\]した周辺\[しゅうへん\]の情報\[じょうほう\]だけを返\[かえ\]す。prefix構文\[こうぶん\]では誤\[あやま\]り後\[ご\]の正\[ただ\]しい境界\[きょうかい\]が入力\[にゅうりょく\]だけから復元\[ふくげん\]できない場合\[ばあい\]がある。完全\[かんぜん\]に復元\[ふくげん\]できると広告\[こうこく\]しない。

SentenceLiteralでは改行\[かいぎょう\]\/EOF\/終了引用符\[しゅうりょういんようふ\]が回復境界\[かいふくきょうかい\]。注釈\[ちゅうしゃく\]の不足括弧\[ふそくかっこ\]についてexpected tokenと開始位置\[かいしいち\]を返\[かえ\]す。期待\[きたい\]する閉\[と\]じ括弧\[かっこ\]を自動挿入\[じどうそうにゅう\]するfixは提案\[ていあん\]であり、ユーザー操作\[そうさ\]なしにソースを書\[か\]き換\[か\]えない。

<a name="n-696e6372656d656e74616c"></a>

<a name="6-増分解析と取消し"></a>

## 6\. 増分解析\[ぞうぶんかいせき\]と取消\[とりけ\]し

編集\[へんしゅう\]はbaseRevision付\[つ\]きTextEdit列\[れつ\]。編集後\[へんしゅうご\]に新\[しん\]snapshotを作\[つく\]り、古\[ふる\]いsnapshotは不変\[ふへん\]。編集\[へんしゅう\]に交差\[こうさ\]しないrangeは編集写像\[へんしゅうしゃぞう\]で新\[しん\]snapshotへ明示的\[めいじてき\]に移\[うつ\]せる。交差\[こうさ\]するtoken、子\[こ\]contextを変更\[へんこう\]する前方宣言\[ぜんぽうせんげん\]、その依存下流\[いぞんかりゅう\]を無効化\[むこうか\]する。

再利用可能\[さいりようかのう\]なnodeは、source内容\[ないよう\]、entry category、reader\/context\/provider digestが一致\[いっち\]する場合\[ばあい\]に限\[かぎ\]る。独立\[どくりつ\]な埋\[う\]め込\[こ\]みや意味値\[いみち\]は再利用\[さいりよう\]できる。全再解析\[ぜんさいかいせき\]との出力比較\[しゅつりょくひかく\]をconformanceに含\[ふく\]める。最悪時\[さいあくじ\]の全体再解析\[ぜんたいさいかいせき\]は正\[ただ\]しい経路\[けいろ\]として残\[のこ\]し、常\[つね\]に編集差分\[へんしゅうさぶん\]だけの計算量\[けいさんりょう\]を保証\[ほしょう\]しない。

LSP側\[がわ\]はdebounceとcancelを担当\[たんとう\]する。coreは明示的\[めいじてき\]budget\/pollを使\[つか\]う。古\[ふる\]いrevisionの結果\[けっか\]は公開\[こうかい\]しない。cacheと統計\[とうけい\]は明示的\[めいじてき\]session stateであり、隠\[かく\]れたglobalにはしない。

<a name="n-6c7370"></a>

<a name="7-lsp-adapter"></a>

## 7\. LSP adapterの責務\[せきむ\]

3\.17で定義\[ていぎ\]された位置\[いち\]encoding\/diagnostic\/semantic token\/definition等\[など\]の契約\[けいやく\]を利用\[りよう\]し、機能\[きのう\]はcapabilityで交渉\[こうしょう\]する。UTF\-8\/UTF\-16\/UTF\-32の位置変換\[いちへんかん\]をLineIndexで行\[おこな\]う。UTF\-16をfallbackとして必\[かなら\]ず扱\[あつか\]う。日本語\[にほんご\]や補助平面文字\[ほじょへいめんもじ\]のbyte数\[すう\]、UTF\-16 code unit数\[すう\]、表示幅\[ひょうじはば\]を混同\[こんどう\]しない。

一般的\[いっぱんてき\]なDSL class名\[めい\]はsemantic token legendの独自\[どくじ\]typeとして出\[だ\]せる。クライアントが対応\[たいおう\]しない場合\[ばあい\]は表示\[ひょうじ\]fallbackを使\[つか\]うが、NEPL3内部\[ないぶ\]kindを書\[か\]き換\[か\]えない。position\/legendの変換\[へんかん\]はdomain crateに置\[お\]かない。

nepl3\-lspは一\[ひと\]つの汎用\[はんよう\]server。workspace設定\[せってい\]から拡張子\[かくちょうし\]とLanguagePackageを選択\[せんたく\]し、各\[かく\]DSLごとのserverの再実装\[さいじっそう\]は不要\[ふよう\]。VS Code\/Neovimには接続\[せつぞく\]と設定\[せってい\]だけの薄\[うす\]いadapterを置\[お\]く。packageを変更\[へんこう\]したらanalysisを無効化\[むこうか\]し、必要\[ひつよう\]ならlegend登録\[とうろく\]も更新\[こうしん\]する。

<a name="n-646961676e6f7374696373"></a>

<a name="8-診断ログの表示"></a>

## 8\. 診断\[しんだん\]・ログの表示\[ひょうじ\]

同一\[どういつ\]DiagnosticをCLI、LSP、browserへrenderする。CLI stderrは表示\[ひょうじ\]adapter、stdoutは要求\[ようきゅう\]された成果物\[せいかぶつ\]だけ。JSON\/NDF診断出力\[しんだんしゅつりょく\]と人間用表示\[にんげんようひょうじ\]を分離\[ぶんり\]する。詳細\[しょうさい\]traceは明示的\[めいじてき\]に選択\[せんたく\]し、ソース全文\[ぜんぶん\]や環境\[かんきょう\]の機密値\[きみつち\]を既定\[きてい\]では記録\[きろく\]しない。

情報\[じょうほう\]が不足\[ふそく\]する場合\[ばあい\]はstage\/requirementsとして表示\[ひょうじ\]し、存在\[そんざい\]しないsource位置\[いち\]を作\[つく\]らない。providerの内部\[ないぶ\]エラーとユーザーの構文\[こうぶん\]ミスを区別\[くべつ\]する。

<a name="n-7472757374"></a>

<a name="9-workspace-trust"></a>

## 9\. workspace trustの境界\[きょうかい\]

通常\[つうじょう\]の解析\[かいせき\]と診断\[しんだん\]で対象\[たいしょう\]プログラムをevaluateしない。Grammar reader providerはhostのallowlist・署名\[しょめい\]・schemaで制限\[せいげん\]する。workspaceからnative pluginを自動\[じどう\]build\/loadしない。untrusted providerには隔離\[かくり\]runnerが必要\[ひつよう\]で、ない環境\[かんきょう\]ではTrustRequiredを返\[かえ\]す。巨大入力\[きょだいにゅうりょく\]\/再帰\[さいき\]\/イベント\/出力\[しゅつりょく\]にも上限\[じょうげん\]を設定\[せってい\]する。
