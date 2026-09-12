<!-- Generated from doc/spec/12&#45;model&#45;invariants.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page model&#45;invariants; source SHA-256 f16329d0cc727a51491c061cc6ae2a636a75a24f343deb8f8a782369dd1d53b8; alias input SHA-256 0d428b3fec7161d2cd0f1b075771d0597505727c39f78e90c835840c638f4c4c; document digest 457cb69d9ed646ceeeaad89f5c9bda75139dc85a0831aa3b5577e562ae39118d; input PageSet digest e183ffa10a6be3c7c3f1df322f8f76f73ee8e1380319d27de66706bc790fe5c0; input context SHA-256 01a71e3b6feac889d7a4b61f3b24e66b223a4b97f9c2ed41db3cbf4bbb46541d. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="12-意味モデル中間表現の補足不変条件"></a>

# 12\. 意味\[いみ\]モデル・中間表現\[ちゅうかんひょうげん\]の補足不変条件\[ほそくふへんじょうけん\]

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

`interfaces/model.json` はsourceのform表\[ひょう\]と別\[べつ\]の、lower後\[ご\]の意味値\[いみち\]・実行\[じっこう\]IRの構造\[こうぞう\]を記述\[きじゅつ\]する。surfaceとmeaningのSchemaRefは別\[べつ\]にする。Rustで型\[かた\]を分\[わ\]け、wireで両者\[りょうしゃ\]を混同\[こんどう\]しない。

<a name="n-76616c756573"></a>

<a name="1-値と参照"></a>

## 1\. 値\[あたい\]と参照\[さんしょう\]

JSON内\[ない\]の `record` は `[fieldName, typeExpression]` の順序付\[じゅんじょつ\]きarrayであり、その順序\[じゅんじょ\]をNDF recordのfield順\[じゅん\]とする。`sum` はvariant名\[めい\]をkeyとするmapで、各\[かく\]payloadも同\[おな\]じ順序付\[じゅんじょつ\]きfield array。空\[から\]record\/payloadは空\[から\]arrayとする。field名\[めい\]は各\[かく\]array内\[ない\]で一意\[いちい\]。`union` はvariant名\[めい\]からrecord型\[がた\]へのmap。sum\/unionおよびcontractsのvariantsのkey順\[じゅん\]には意味\[いみ\]を持\[も\]たせず、NDFではVariantNameの文字列\[もじれつ\]で識別\[しきべつ\]する。数値\[すうち\]ordinalをJSON objectの列挙順\[れっきょじゅん\]から割\[わ\]り当\[あ\]てない。List\/OptionはNDFの対応\[たいおう\]tag。Naturalはnonnegative Integer。Name\/LanguageTagはTextに対\[たい\]して各\[かく\]domainの制約\[せいやく\]を追加\[ついか\]する。

Source由来\[ゆらい\]の意味値\[いみち\]はOriginを関連付\[かんれんづ\]けられる。各\[かく\]Recordが共有\[きょうゆう\]できるようにnative実装\[じっそう\]をarenaにしてもよいが、公開値\[こうかいち\]の意味\[いみ\]をpointerに依存\[いぞん\]させない。wireは参照\[さんしょう\]table付\[つ\]きbundleを使\[つか\]い、全\[ぜん\]NodeRef\/OriginRefが有効\[ゆうこう\]であることを検査\[けんさ\]する。意味上\[いみじょう\]の構造木\[こうぞうき\]と回路\[かいろ\]graphのcycle制約\[せいやく\]は異\[こと\]なる。

r4ではNodeRef\/OriginRef、Origin、SyntaxNode\/FieldValue、SyntaxBundle\/ForeignSyntax、EnvironmentRefを共通\[きょうつう\]foundation所有\[しょゆう\]のcontractsへ移\[うつ\]した。modelはexternal\_typesから参照\[さんしょう\]し、同\[おな\]じ名義型\[めいぎがた\]を二重定義\[にじゅうていぎ\]しない。SyntaxBundleはsources、nodes、origins、root、environmentsを持\[も\]つ。ForeignSyntax\.rootとguest bundle\.rootは等\[ひと\]しくなければならず、guestのnode\/origin IDはそのbundle内\[ない\]だけで解決\[かいけつ\]する。ForeignSyntax\.environmentはforeign slotを所有\[しょゆう\]するhost bundleのEnvironmentEntryのid\/digestへ一致\[いっち\]させる。guestへの無条件\[むじょうけん\]の名前空間継承\[なまえくうかんけいしょう\]は行\[おこな\]わない。

EnvironmentEntryはid\/digest\/valueを持\[も\]ち、Environmentは明示的\[めいじてき\]なnamespace\/name\/value\/originのbinding列\[れつ\]とresource列\[れつ\]を持\[も\]つ。同\[おな\]じnamespace\/nameのbindingとresource IDの重複\[ちょうふく\]を拒否\[きょひ\]する。bindingのOriginRefはそのentryの所属\[しょぞく\]bundle内\[ない\]で解決\[かいけつ\]する。entry digestはASCII `NEPL3-ENVIRONMENT-1`、zero byte、canonical NDFで符号化\[ふごうか\]したEnvironment recordのSHA\-256とする。型付\[かたつ\]きnative graph検査\[けんさ\]はentryの選択\[せんたく\]と参照\[さんしょう\]を検査\[けんさ\]し、wire adapterはcanonical digestとresource元\[もと\]byte列\[れつ\]のdigestも再検査\[さいけんさ\]する。

Doc\/Math等\[とう\]の意味値\[いみち\]が単体\[たんたい\]Foreignを保持\[ほじ\]する場合\[ばあい\]はForeignClosureを使\[つか\]う。syntaxのguest arenaとownerEnvironment\/ownerOrigins\/ownerSources\/ownerSourceMapsのarenaは独立\[どくりつ\]する。同\[おな\]じ数値\[すうち\]の環境\[かんきょう\]ID・OriginRefでも両者\[りょうしゃ\]の値\[あたい\]を置換\[ちかん\]しない。元\[もと\]ownerOrigin列\[れつ\]を保持\[ほじ\]し、環境\[かんきょう\]digestを変\[か\]えない。ForeignClosure\.captureは検査済\[けんさず\]みownerの実\[じつ\]Foreign fieldだけを取\[と\]り出\[だ\]し、ownerの構文\[こうぶん\]nodeは所有化\[しょゆうか\]しない。受信時\[じゅしんじ\]は外側\[そとがわ\]schemaとguest rootのschema、root ID、選択\[せんたく\]したowner環境\[かんきょう\]のid\/digest、owner\/guestそれぞれの閉包\[へいほう\]を検査\[けんさ\]し、portable adapterは両環境\[りょうかんきょう\]の内容\[ないよう\]digestを再計算\[さいけいさん\]する。raw閉包\[へいほう\]の成立\[せいりつ\]から言語固有\[げんごこゆう\]の意味\[いみ\]lowerやhostによる環境授権\[かんきょうじゅけん\]を推定\[すいてい\]しない。

portable境界\[きょうかい\]の停止理由\[ていしりゆう\]は型付\[かたつ\]き原因\[げんいん\]から取\[と\]り出\[だ\]す。Schema、Source、Origin、View、Syntax、Fact、Reportに包\[つつ\]まれたStoppedも元\[もと\]StopReasonを保持\[ほじ\]し、domain側\[がわ\]の通常\[つうじょう\]Invalidへ縮約\[しゅくやく\]しない。WireErrorの直接\[ちょくせつ\]Stoppedだけを認識\[にんしき\]していた停止抽出\[ていしちゅうしゅつ\]を訂正\[ていせい\]する（R047）。意味\[いみ\]エラーを、無関係\[むかんけい\]に停止\[ていし\]したBudgetの状態\[じょうたい\]で上書\[うわが\]きする規則\[きそく\]ではない。

通常\[つうじょう\]のsource nodeではheadはcover内\[ない\]、各子\[かくこ\]のcoverは親\[おや\]cover内\[ない\]かつhead\.end以降\[いこう\]で、子同士\[こどうし\]はfield順\[じゅん\]に重複\[ちょうふく\]しない。graph\/位置検査\[いちけんさ\]だけでformのarity・引数\[ひきすう\]category・各\[かく\]field型\[がた\]まで検査\[けんさ\]したことにはしない。surface LanguagePackageの検査\[けんさ\]を別\[べつ\]に行\[おこな\]う。生成\[せいせい\]nodeに架空\[かくう\]のcoverを要求\[ようきゅう\]しない。

SourceMapはsource\/target SpanとExactまたはTransformedのMapping列\[れつ\]を持\[も\]つ。Exactは元\[もと\]と先\[さき\]のbyte列\[れつ\]が等\[ひと\]しく、局所逆写像\[きょくしょぎゃくしゃぞう\]はoffset差\[さ\]で求\[もと\]める。Transformedは対応\[たいおう\]するfragment間\[かん\]の関係\[かんけい\]だけを表\[あらわ\]し、任意\[にんい\]の部分区間\[ぶぶんくかん\]を逆変換\[ぎゃくへんかん\]できるとはしない。重\[かさ\]なった複数\[ふくすう\]の候補\[こうほ\]はAmbiguous、非可逆\[ひかぎゃく\]はIrreversibleとしてrename等\[とう\]へ返\[かえ\]す。

map循環\[じゅんかん\]の頂点\[ちょうてん\]はsnapshotとbyte位置\[いち\]である。非空範囲\[ひくうはんい\]は半開区間内\[はんかいくかんない\]の各\[かく\]byte、空範囲\[くうはんい\]はそのoffsetの独立\[どくりつ\]したanchor頂点\[ちょうてん\]とし、同\[おな\]じoffsetの内容\[ないよう\]byteとanchorを混同\[こんどう\]しない。Exactはoffset差\[さ\]を保存\[ほぞん\]するedge、Transformedは元\[もと\]fragmentの全頂点\[ぜんちょうてん\]から先\[さき\]fragmentの全頂点\[ぜんちょうてん\]への関係\[かんけい\]である。この有限\[ゆうげん\]graphのcycleを拒否\[きょひ\]する。同一\[どういつ\]snapshot内\[ない\]でも一方向\[いちほうこう\]へ進\[すす\]む重複\[ちょうふく\]Exact区間\[くかん\]や、互\[たが\]いに接続\[せつぞく\]しない範囲\[はんい\]はcycleとしない。検査量\[けんさりょう\]の超過\[ちょうか\]はStoppedであり、粗\[あら\]いsnapshot依存\[いぞん\]だけを根拠\[こんきょ\]にCycleと返\[かえ\]さない。

<a name="n-636f6e73747261696e7473"></a>

<a name="11-foundation制約id"></a>

## 1\.1 foundation制約\[せいやく\]ID

constraint IDはstructural descriptorとともにdigestへ含\[ふく\]める。共通\[きょうつう\]Rust constructor・boundary adapterは次\[つぎ\]を検査\[けんさ\]する。descriptorにIDがあるだけで検査\[けんさ\]を実行\[じっこう\]したとは扱\[あつか\]わない。

| ID | 検査\[けんさ\]する不変条件\[ふへんじょうけん\] |
| --- | --- |
| source\.identity | 空\[から\]でないopaque IDと明示的\[めいじてき\]なrevision\/content digest |
| source\.content | locator profile、UTF\-8元\[もと\]byte列\[れつ\]、content digestの一致\[いっち\] |
| source\.bundle | snapshot宣言\[せんげん\]の一意性\[いちいせい\]、同\[おな\]じID\/revisionの矛盾拒否\[むじゅんきょひ\]、共有操作\[きょうゆうそうさ\]での入力予算\[にゅうりょくよさん\] |
| source\.span | 指定\[してい\]snapshotの範囲\[はんい\]・半開順序\[はんかいじゅんじょ\]・UTF\-8 scalar境界\[きょうかい\] |
| schema\.reference \/ schema\.type\-reference | 空\[から\]でないpackage\/type名\[めい\]と、選択\[せんたく\]された版\[ばん\]・digestまたは記号的参照先\[きごうてきさんしょうさき\] |
| schema\.descriptor | 重複宣言拒否\[ちょうふくせんげんきょひ\]、canonical descriptor、全参照\[ぜんさんしょう\]のfinalize |
| operation\.reporting | すべての結果\[けっか\]の累積\[るいせき\]Usage、共有\[きょうゆう\]Limits、停止理由\[ていしりゆう\]の維持\[いじ\]、partialをCheckedとしない |
| report\.trace\-overflow | 実際\[じっさい\]の未受理\[みじゅり\]event件数\[けんすう\]が正\[せい\]、単一\[たんいつ\]の報告\[ほうこく\]、EventLimitの停止\[ていし\] |
| namespace\.reference | 明示\[めいじ\]したschemaと空\[から\]でないnamespace名\[めい\] |
| environment\.bindings \/ environment\.digest | binding\/resourceの一意性\[いちいせい\]、範囲\[はんい\]と型\[かた\]、canonical entry digest |
| syntax\.graph \/ syntax\.foreign | bundle局所参照\[きょくしょさんしょう\]、有限\[ゆうげん\]graph、source geometry、host環境\[かんきょう\]とguest rootの一致\[いっち\] |
| source\.map \/ origin\.graph | 上記\[じょうき\]のmap関係\[かんけい\]とOrigin DAG、所属\[しょぞく\]snapshot・OperationRef・参照先\[さんしょうさき\] |
| source\.reservation | 空\[から\]でないhost予約\[よやく\]SourceId、revision、絶対\[ぜったい\]logical URI。生成\[せいせい\]bytesからdigestを計算\[けいさん\]し、異\[こと\]なる結果\[けっか\]への予約再利用\[よやくさいりよう\]を拒否\[きょひ\] |
| schema\.kind\-id | 選択\[せんたく\]schemaの型名\[かためい\]scalar順\[じゅん\]に割\[わ\]り当\[あ\]てたlocalKindとdescriptorの対応\[たいおう\] |
| view\.graph | Tokenごとに局所的\[きょくしょてき\]なview参照\[さんしょう\]、DAG、field名\[めい\]の一意性\[いちいせい\]、source範囲\[はんい\] |
| token\.boundary | token head・triviaのsnapshotと境界\[きょうかい\]、内部\[ないぶ\]viewの包含\[ほうがん\]、payloadの型\[かた\]はreader\/form契約\[けいやく\]で検査\[けんさ\] |
| view\.presentation | schemaが所有\[しょゆう\]する表示分類名\[ひょうじぶんるいめい\]と明示\[めいじ\]されたfallback role |

SyntaxBundleはtokensのtableを持\[も\]ち、SyntaxNode\.tokenは同\[おな\]じbundleのTokenRefを指\[さ\]す。Tokenはpayload、内部\[ないぶ\]ViewBundle、leadingTriviaを保持\[ほじ\]する。ViewRefはそのTokenのViewBundle内\[ない\]だけ、TokenRefはそのSyntaxBundle内\[ない\]だけで解決\[かいけつ\]する。ForeignSyntaxのguest bundleは自身\[じしん\]のtableを持\[も\]つため、同\[おな\]じ数値\[すうち\]IDをhostへ解決\[かいけつ\]しない。通常\[つうじょう\]のsource由来\[ゆらい\]nodeにはheadに対応\[たいおう\]するtokenをengineが要求\[ようきゅう\]し、synthetic\/recovery nodeのtoken不在\[ふざい\]はOptionで明示\[めいじ\]する。graphの検査\[けんさ\]とformのarity・payload型\[がた\]の検査\[けんさ\]を区別\[くべつ\]する。

sourceMapsはSyntaxBundleの所有列\[しょゆうれつ\]であり、全\[ぜん\]source\/targetをそのbundleのsourcesで解決\[かいけつ\]する。SourceMapの幾何\[きか\]・Exact内容一致\[ないよういっち\]・非循環\[ひじゅんかん\]を検査\[けんさ\]したproofだけをmapped view包含\[ほうがん\]に使用\[しよう\]する。standalone Token\/ViewBundleのvalidateはmapを持\[も\]たない直接包含\[ちょくせつほうがん\]の入口\[いりぐち\]とし、変換\[へんかん\]viewはvalidate\_with\_mapsまたはSyntaxBundle境界\[きょうかい\]を使\[つか\]う。包含\[ほうがん\]は02章\[しょう\]の全逆経路規則\[ぜんぎゃくけいろきそく\]に従\[したが\]い、sourceがhost storeに偶然存在\[ぐうぜんそんざい\]することを所有証明\[しょゆうしょうめい\]にしない。

ValidatedSourceMapは不変\[ふへん\]なsnapshot identity上\[じょう\]のmap関係\[かんけい\]のproofであり、任意\[にんい\]の後続\[こうぞく\]SourceStoreへの所属\[しょぞく\]proofではない。Token\/Viewのvalidate\_with\_mapsは使用時\[しようじ\]のstoreで全\[ぜん\]map端点\[たんてん\]の宣言閉包\[せんげんへいほう\]を再照合\[さいしょうごう\]する。SourceMap\.contains単体\[たんたい\]は関係上\[かんけいじょう\]の包含計算\[ほうがんけいさん\]であり、transport source tableの所属\[しょぞく\]を検査\[けんさ\]する入口\[いりぐち\]とは区別\[くべつ\]する。

内部\[ないぶ\]viewは外側\[そとがわ\]のchildrenやarityへ加算\[かさん\]しない。SentenceLiteralの構造化\[こうぞうか\]payloadとview、Codeが保持\[ほじ\]するforeign syntaxのtoken・triviaはnativeとNDFの両経路\[りょうけいろ\]で保存\[ほぞん\]し、元\[もと\]sourceの再\[さい\]parseを情報保持\[じょうほうほじ\]の代替\[だいたい\]にしない。

Token\.payloadは、その型\[かた\]が所有\[しょゆう\]する独立\[どくりつ\]bundleまたは明示\[めいじ\]source参照\[さんしょう\]を持\[も\]つ。まだ構築\[こうちく\]されていない外側\[そとがわ\]SyntaxBundleのNodeRefを暗黙\[あんもく\]に参照\[さんしょう\]しない。payload内\[ない\]に現\[あらわ\]れる数値\[すうち\]を外側\[そとがわ\]node IDと推測\[すいそく\]して再採番\[さいさいばん\]しない。この所有契約\[しょゆうけいやく\]により、outer nodeのcanonical再採番\[さいさいばん\]はopaque payloadを壊\[こわ\]さず行\[おこな\]える。

DocのSentenceLiteralはlower後\[ご\]にDoc\:Sentenceとなる。raw Textには注釈構文\[ちゅうしゃくこうぶん\]を再適用\[さいてきよう\]しない。MathのNumberは有限十進\[ゆうげんじっしん\]で表現\[ひょうげん\]できるRational（約分後\[やくぶんご\]の分母\[ぶんぼ\]の素因数\[そいんすう\]が2と5だけ）と元\[もと\]の表記範囲\[ひょうきはんい\]を持\[も\]ち、SymbolNameはMath\:Symbolへ統合\[とうごう\]する。違反\[いはん\]はNonFiniteDecimalNumber。Numeric spellingを表示\[ひょうじ\]に使\[つか\]う場合\[ばあい\]は、そのsnapshotと値\[あたい\]が一致\[いっち\]していることを検査\[けんさ\]する。生成\[せいせい\]Numberはcanonicalな整数\[せいすう\]または有限十進\[ゆうげんじっしん\]でprintする。任意有理数\[にんいゆうりすう\]からの式構築\[しきこうちく\]と著者\[ちょしゃ\]のFracの保存\[ほぞん\]はMath章\[しょう\]の規則\[きそく\]に従\[したが\]う。

ForeignSyntaxはguestのopaque bundleを持\[も\]ち、host coreはguestの意味型\[いみがた\]をimportしない。suiteで登録済\[とうろくず\]みschemaに検査\[けんさ\]してからguest操作\[そうさ\]に渡\[わた\]す。Doc\:DocGuestもForeignSyntaxを保持\[ほじ\]する。Codeの準備\[じゅんび\]はbundleの安全性\[あんぜんせい\]を検査\[けんさ\]してsourceとviewを表示\[ひょうじ\]し、guestのlower・意味\[いみ\]check・evaluateを呼\[よ\]ばない。

<a name="n-636865636b6564"></a>

<a name="2-checked値の境界"></a>

## 2\. Checked値\[ち\]の境界\[きょうかい\]

Rust内部\[ないぶ\]のChecked\/Preparedのconstructorはprivateにする。wireでCheckedEnvelopeを受信\[じゅしん\]しただけで検査済\[けんさず\]みと信用\[しんよう\]しない。外部\[がいぶ\]providerからの結果\[けっか\]にはschema検査\[けんさ\]・構造不変条件検査\[こうぞうふへんじょうけんけんさ\]を行\[おこな\]い、利用\[りよう\]する操作\[そうさ\]に必要\[ひつよう\]な意味検査\[いみけんさ\]を再実行\[さいじっこう\]する。digestは同一性\[どういつせい\]の情報\[じょうほう\]であって証明書\[しょうめいしょ\]ではない。

process\/session内\[ない\]で検査結果\[けんさけっか\]を再利用\[さいりよう\]する場合\[ばあい\]は、provider・input・schema・environmentを固定\[こてい\]したhost所有\[しょゆう\]handleを使\[つか\]える。別\[べつ\]processへ生\[なま\]のpointerやprivateなhandleを送\[おく\]らない。

<a name="n-63697263756974"></a>

<a name="3-回路ir"></a>

## 3\. 回路\[かいろ\]IR

NetNode\.idはnodesのindexに一致\[いっち\]し、orderは組合\[くみあわ\]せDAGの全\[ぜん\]nodeを一度\[いちど\]ずつ含\[ふく\]む順序\[じゅんじょ\]。Input\/State\/Constantのinputsは空\[から\]、Not\/Sliceは1、And\/Or\/Xor\/Nor\/Add\/Concatは2、Muxは3。InputとStateは有効\[ゆうこう\]なport\/slotを参照\[さんしょう\]する。Slice\/Mux\/Concatのwidth条件\[じょうけん\]はCircuit章\[しょう\]の通\[とお\]り。

outputNodesの長\[なが\]さはoutputsと等\[ひと\]しく、各\[かく\]nodeの幅\[はば\]がportと一致\[いっち\]する。StateSlot\.nextは有効\[ゆうこう\]nodeで同\[おな\]じ幅\[はば\]。初期値\[しょきち\]は幅内\[はばない\]。state次値\[じち\]へのedgeを現在\[げんざい\]state readの依存\[いぞん\]へ戻\[もど\]さない。

NorNetlistはbit順\[じゅん\]をport宣言順\[せんげんじゅん\]、その中\[なか\]をLSB→MSBとする。stateもslot順\[じゅん\]、その中\[なか\]をLSB→MSB。nextBits\/outputBitsの長\[なが\]さは対応幅\[たいおうはば\]の和\[わ\]。Nor2の参照\[さんしょう\]はtopologicalに前\[まえ\]のnodeだけ。InputBit\/StateBitは有効範囲\[ゆうこうはんい\]。originsはnodesと同\[おな\]じ長\[なが\]さ。

<a name="n-6d61726b7570"></a>

<a name="4-markup安全性"></a>

## 4\. Markup安全性\[あんぜんせい\]

Markup modelがname\/attributeをTextとして運\[はこ\]べることは、任意\[にんい\]のtagを許可\[きょか\]することを意味\[いみ\]しない。nepl3\-markupは目的\[もくてき\]categoryに対\[たい\]して検査\[けんさ\]する。

HTMLでは文書構造\[ぶんしょこうぞう\]・注釈\[ちゅうしゃく\]・コード・リンクに加\[くわ\]え、ul\/ol\/li、table\/caption\/thead\/tbody\/tr\/th\/td、imgを扱\[あつか\]う。全要素\[ぜんようそ\]の列挙\[れっきょ\]は `design/markup.json` を正本\[せいほん\]とし、内容\[ないよう\]モデルと型付\[かたつ\]き属性\[ぞくせい\]は [HTML fragment契約\[けいやく\]](<19\-html\-fragment\.md>) に従\[したが\]う。MathML\: math\, mrow\, mi\, mn\, mo\, mtext\, mfrac\, msqrt\, mroot\, msub\, msup\, msubsup\, munder\, mover\, munderover\, mtable\, mtr\, mtd\, mspace。SVG\: svg\, g\, rect\, line\, path\, polyline\, circle\, text\, title\, desc。

全要素\[ぜんようそ\]・属性\[ぞくせい\]・属性値制約\[ぞくせいちせいやく\]の正本\[せいほん\]は `design/markup.json`。属性\[ぞくせい\]を受\[う\]け入\[い\]れる集合\[しゅうごう\]はglobal、namespace、elementの各\[かく\]allowlistの和\[わ\]とし、列挙\[れっきょ\]されていない属性\[ぞくせい\]は拒否\[きょひ\]する。SVGのpath\/points等\[とう\]は自由\[じゆう\]な文字列\[もじれつ\]として受\[う\]けず、記述\[きじゅつ\]した型付\[かたつ\]き構造\[こうぞう\]からserializerが綴\[つづ\]りを生成\[せいせい\]する。userから任意\[にんい\]のon\*、style、script、foreignObject、SVG image、任意\[にんい\]のnamespace URLを受\[う\]けない。HTMLのhrefは型付\[かたつ\]きFragment\/Artifact\/BetweenArtifacts\/Externalを使\[つか\]い、URIの許可規則\[きょかきそく\]と内部\[ないぶ\]targetの存在\[そんざい\]、文書間\[ぶんしょかん\]routeの対応\[たいおう\]をそれぞれ検査\[けんさ\]する。HTML imgのsrcは検査済\[けんさず\]みartifact内\[ない\]の相対\[そうたい\]pathであり、任意\[にんい\]の外部画像\[がいぶがぞう\]URLを許可\[きょか\]しない。字句検査\[じくけんさ\]だけでassetの存在\[そんざい\]・内容\[ないよう\]・権限\[けんげん\]を検証済\[けんしょうず\]みとせず、文書準備時\[ぶんしょじゅんびじ\]の解決\[かいけつ\]を別\[べつ\]に要求\[ようきゅう\]する。CSSはbackendが所有\[しょゆう\]する固定\[こてい\]assetであり、本文文字列\[ほんぶんもじれつ\]をCSSに埋\[う\]め込\[こ\]まない。

MathMLのmspaceのwidth\/height\/depthはNonnegativeMathLengthとする。非負\[ひふ\]のcanonical有限十進\[ゆうげんじっしん\]にemを付\[つ\]け、zeroは0em、百分率\[ひゃくぶんりつ\]・指数表記\[しすうひょうき\]・他単位\[たたんい\]・負値\[ふち\]・冗長\[じょうちょう\]なzeroを拒否\[きょひ\]する。これはMathML Coreのlength\-percentageのうち本\[ほん\]profileが使用\[しよう\]する部分集合\[ぶぶんしゅうごう\]であり、SVGの座標用\[ざひょうよう\]Decimalとは区別\[くべつ\]する。違反\[いはん\]はInvalidMarkupAttribute。

MarkupのTextと属性値\[ぞくせいち\]は、UTF\-8の妥当性\[だとうせい\]に加\[くわ\]え、XML 1\.0 Charの集合\[しゅうごう\]（U\+0009、U\+000A、U\+000D、U\+0020\.\.D7FF、U\+E000\.\.FFFD、U\+10000\.\.10FFFF）を共通\[きょうつう\]の許可集合\[きょかしゅうごう\]とする。それ以外\[いがい\]はInvalidMarkupCharacterで拒否\[きょひ\]し、削除\[さくじょ\]や置換文字\[ちかんもじ\]による黙殺\[もくさつ\]をしない。Doc\/Mathの一般\[いっぱん\]Text値\[ち\]をこの出力用制約\[しゅつりょくようせいやく\]で狭\[せば\]めるのではなく、Markup構築\[こうちく\]・検査境界\[けんさきょうかい\]で適用\[てきよう\]する。

serializerはHTML5とXMLを区別\[くべつ\]し、namespace、void element、属性\[ぞくせい\]escapeを適切\[てきせつ\]に出\[だ\]す。属性順\[ぞくせいじゅん\]は固定\[こてい\]。Textと属性\[ぞくせい\]のescapeおよび改行保存\[かいぎょうほぞん\]は再現性章\[さいげんせいしょう\]に従\[したが\]う。文書\[ぶんしょ\]テキストを文字列置換\[もじれつちかん\]でHTMLへ挿入\[そうにゅう\]しない。

<a name="n-6f7065726174696f6e73"></a>

<a name="5-operationの具体型"></a>

## 5\. operationの具体型\[ぐたいけい\]

`interfaces/contracts.json` のDomainSyntax\/CheckedDomain等\[とう\]はoperationごとに本\[ほん\]ファイルの対応\[たいおう\]domain型\[がた\]へ特殊化\[とくしゅか\]するための表記\[ひょうき\]。例\[たと\]えばmath\.lowerの結果\[けっか\]はMath\/Expr、circuit\.elaborateの結果\[けっか\]はCircuit\:PreparedNetlist、doc\.prepareの結果\[けっか\]はDoc\:PreparedArticle。異\[こと\]なるdomainのTypedValueを同\[おな\]じ入力\[にゅうりょく\]として受\[う\]け付\[つ\]けない。

Grammar packageのReaderExpr\/ReadSpec\/Binding\/StyleのpayloadはGrammarのschemaで定義\[ていぎ\]したADTを使用\[しよう\]できる。参照\[さんしょう\]を解決\[かいけつ\]した索引\[さくいん\]tableを追加\[ついか\]してよいが、意味正規形\[いみせいきけい\]は参照先\[さんしょうさき\]の識別子\[しきべつし\]と契約\[けいやく\]に従\[したが\]って比較\[ひかく\]する。無限再帰\[むげんさいき\]のRust型\[がた\]やSerde表現\[ひょうげん\]をwireへ押\[お\]し付\[つ\]けない。

HTMLのdocument shell（html\/head\/meta\/style\/body）は固定\[こてい\]のshell生成処理\[せいせいしょり\]が作\[つく\]り、userが与\[あた\]えるMarkupFragmentの要素\[ようそ\]として受\[う\]け取\[と\]らない。内部\[ないぶ\]のDoc\/Math\/SVG namespace遷移\[せんい\]とphrasing\/block制約\[せいやく\]もmarkup\.jsonに従\[したが\]う。最適化\[さいてきか\]や資源追加\[しげんついか\]の都合\[つごう\]でこのallowlistを迂回\[うかい\]しない。
