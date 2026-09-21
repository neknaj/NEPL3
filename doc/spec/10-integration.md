<!-- Generated from doc/spec/10&#45;integration.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page integration; source SHA-256 9161d6faee7ab61be38132df12ebd4e83578dc376e79f213aff172174bcfc107; alias input SHA-256 00e7d3fbbc79af03ae9a66bba6cc0bcd47165fb3eb087ea15a633624a51e1309; document digest 086c0f6d22c3b4d7207523177ec002a075fd7ef916395b4407e1ff346ce9f734; input PageSet digest 943ad743b12614300239fe63cb49f04dc425ce932763acae94dab8be258e8a89; input context SHA-256 3aee2f9d1af4170ff68e2561f995e52d2e115755069880d4eb57776b9385cbef. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="10-profile埋め込み実行入口"></a>

# 10\. Profile・埋\[う\]め込\[こ\]み・実行入口\[じっこういりぐち\]

この章\[しょう\]はsuite・bridge・CLIの目標契約\[もくひょうけいやく\]を含\[ふく\]む。列挙\[れっきょ\]した全操作\[ぜんそうさ\]の実装\[じっそう\]・全\[ぜん\]targetでの受入完了\[うけいれかんりょう\]を宣言\[せんげん\]するものではない。現在\[げんざい\]の実装\[じっそう\]と受入\[うけいれ\]の状態\[じょうたい\]はimplementation\-status\.jsonで管理\[かんり\]する。

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

ソース上\[じょう\]の相互埋\[そうごう\]め込\[こ\]みとcrate依存\[いぞん\]を分離\[ぶんり\]する。suiteが登録済\[とうろくず\]みlanguage package、domain operation、output adapterを接続\[せつぞく\]する。

<a name="n-70726f66696c65"></a>

<a name="1-profile"></a>

## 1\. Profile

不変\[ふへん\]のParseProfileと、構文位置\[こうぶんいち\]ごとのcontextを区別\[くべつ\]する。先行\[せんこう\]する構文\[こうぶん\]で後続\[こうぞく\]contextを更新\[こうしん\]できることは、Profileを不変\[ふへん\]とする契約\[けいやく\]と両立\[りょうりつ\]する。各\[かく\]headのshapeはその出現\[しゅつげん\]までに確定\[かくてい\]した情報\[じょうほう\]から決\[き\]め、後続\[こうぞく\]sourceから遡及\[そきゅう\]して変更\[へんこう\]しない。

Profileはlanguage alias→SchemaRef、category mode、provider allowlist、operation bridge、resource snapshot、Limitsを持\[も\]つ不変値\[ふへんち\]。root languageはファイル拡張子\[かくちょうし\]またはCLI引数\[ひきすう\]で選\[えら\]び、全\[ぜん\]ソースを一律\[いちりつ\]のlexerで先\[さき\]にtoken化\[か\]しない。

`design/profile.json` はsource manifestであり、解決済\[かいけつず\]みruntime Profileではない。R009の解消\[かいしょう\]では生成結果\[せいせいけっか\]の閉\[と\]じた型\[かた\]、各\[かく\]schema\/package\/provider digest、許可\[きょか\]capability、resource identityと整合検査\[せいごうけんさ\]を先\[さき\]に定\[さだ\]める。T05\/T11で実際\[じっさい\]の検査済\[けんさず\]みpackageから生成\[せいせい\]・差分検査\[さぶんけんさ\]し、UI\/Workerはこの値\[あたい\]を利用\[りよう\]する。R006の操作\[そうさ\]・bundle型\[がた\]の未定義\[みていぎ\]を文字列\[もじれつ\]signatureや仮\[かり\]digestで補\[おぎな\]わない。

解析\[かいせき\]の実入口\[じついりぐち\]はengineの`ParseProfile`とし、suite Profileから渡\[わた\]す不変\[ふへん\]なprojectionとして扱\[あつか\]う。`interfaces/engine.json`にlanguage登録\[とうろく\]、category mode、選択\[せんたく\]schema、provider要件\[ようけん\]、operation allowlist、resource identity、Limitsの型\[かた\]を置\[お\]く。`resolve`は独立\[どくりつ\]したhost RuntimeCatalogとfinalize済\[ず\]みregistryを使\[つか\]い、実\[じつ\]package意味\[いみ\]digest、schema参照閉包\[さんしょうへいほう\]、guest category\/mode、host provider登録\[とうろく\]の実装\[じっそう\]identity、resourceの実\[じつ\]byte列\[れつ\]digestを照合\[しょうごう\]する。外部\[がいぶ\]Profileの自己申告\[じこしんこく\]をそのままhost登録\[とうろく\]へ複写\[ふくしゃ\]して検査済\[けんさず\]みとしない。

provider identityはhostが実\[じつ\]assetまたは版管理\[ばんかんり\]された実装\[じっそう\]manifestから確定\[かくてい\]する。coreはbinaryを読\[よ\]み込\[こ\]まず、binary自身\[じしん\]のhashを同\[おな\]じbinary内\[ない\]の定数\[ていすう\]へ埋\[う\]める自己\[じこ\]hash循環\[じゅんかん\]も要求\[ようきゅう\]しない。operationのschema署名\[しょめい\]が既知\[きち\]でもcallbackが登録\[とうろく\]されたことにはならない。parseに使\[つか\]うreader providerは許可済\[きょかず\]み実登録\[じつとうろく\]を要求\[ようきゅう\]し、未実行\[みじっこう\]のfacts等\[とう\]のextensionは署名\[しょめい\]を検査\[けんさ\]するが自動実行\[じどうじっこう\]しない。

解析\[かいせき\]Profileのidentityは`NEPL3-PARSE-PROFILE-1`、zero byte、canonical JSONのSHA\-256。id、language登録\[とうろく\]、schema、category\-mode指定\[してい\]、providerの実装\[じっそう\]identity、allowlist、resource identity、Limitsを含\[ふく\]める。宣言\[せんげん\]はalias\/idまたは完全\[かんぜん\]OperationRef\/SchemaRef順\[じゅん\]、Limits列\[れつ\]はsourceBytes\/work\/depth\/nodes\/allocationUnits\/outputBytes\/diagnostics\/events順\[じゅん\]とする。実\[じつ\]resource bytesはhash照合\[しょうごう\]し、Profileへ全文複製\[ぜんぶんふくせい\]しない。

HeadProviderの登録\[とうろく\]はheadProvidersのHeadRegistration\(alias\,category\,provider\)で選\[えら\]ぶ。同\[おな\]じpackageを登録\[とうろく\]した別\[べつ\]aliasの設定\[せってい\]を共有\[きょうゆう\]したと推定\[すいてい\]しない。\(alias\,category\)重複\[ちょうふく\]を拒否\[きょひ\]し、shape\/childContextの両\[りょう\]OperationRefについて純粋\[じゅんすい\]なHeadCall→HeadReply署名\[しょめい\]、allowlist、独立\[どくりつ\]host catalogの実装\[じっそう\]identityを検査\[けんさ\]する。標準操作名\[ひょうじゅんそうさめい\]はheadShape\/headChildContextだが、登録済\[とうろくず\]みの同署名操作\[どうしょめいそうさ\]も選択\[せんたく\]できる。Profile identityにはheadProviders keyを含\[ふく\]め、alias\/category順\[じゅん\]の `[alias,category,shapeOperation,childContextOperation]` 列\[れつ\]で記述\[きじゅつ\]する。列挙順\[れっきょじゅん\]だけの変更\[へんこう\]はidentityを変\[か\]えず、操作\[そうさ\]の役割\[やくわり\]・alias\/categoryへの割当変更\[わりあてへんこう\]は変\[か\]える。

ここでのallowlistはoperation呼出可否\[よびだしかひ\]を表\[あらわ\]す。providerのtransport、隔離\[かくり\]、ネットワーク等\[とう\]の権限\[けんげん\]、強制停止方法\[きょうせいていしほうほう\]、bridge、EnvironmentProjectionを含\[ふく\]むfull suite Profileの契約\[けいやく\]は引\[ひ\]き続\[つづ\]き実装対象\[じっそうたいしょう\]である。この解析\[かいせき\]projectionだけではR009全体\[ぜんたい\]を完了\[かんりょう\]しない。hostは実行環境\[じっこうかんきょう\]のcapabilityを別途検査\[べっとけんさ\]し、解析\[かいせき\]projectionはその承認\[しょうにん\]を代行\[だいこう\]しない。

配布拡張子\[はいふかくちょうし\]は `.neplg`、`.nepld`、`.neplm`、`.neplc`。汎用\[はんよう\] `.nepl` ではlanguage指定\[してい\]を必須\[ひっす\]にする。既存\[きそん\]NCGやGlossのファイルを新言語\[しんげんご\]として黙\[だま\]って解釈\[かいしゃく\]しない。

ParseProfile\.limitsは解析操作\[かいせきそうさ\]に対\[たい\]するresource別上限\[べつじょうげん\]であり、単\[たん\]なる既定値\[きていち\]ではない。操作開始時\[そうさかいしじ\]に実\[じつ\]Budgetの全\[ぜん\]LimitsがProfile上限以下\[じょうげんいか\]であることを照合\[しょうごう\]し、超\[こ\]える場合\[ばあい\]はLimitsMismatchで拒否\[きょひ\]する。小\[ちい\]さい操作予算\[そうさよさん\]を使\[つか\]うことは許\[ゆる\]す。既\[すで\]に消費\[しょうひ\]したUsageはresetせず、継続\[けいぞく\]も同\[おな\]じ操作\[そうさ\]Limitsと単調\[たんちょう\]なUsageを保持\[ほじ\]する。Profileのresolve自体\[じたい\]を行\[おこな\]う開発\[かいはつ\]・host側\[がわ\]Budgetはこの解析操作\[かいせきそうさ\]Budgetとは別\[べつ\]であり、解決時\[かいけつじ\]の消費\[しょうひ\]を解析\[かいせき\]へ済\[す\]んだものとして移\[うつ\]さない。

EntryContextはaliasを明示保存\[めいじほぞん\]する。同\[おな\]じpackage identityを異\[こと\]なるaliasで登録\[とうろく\]してcategory\-mode overrideだけを変\[か\]えることを許\[ゆる\]し、子\[こ\]のLocal解決\[かいけつ\]も親\[おや\]の実\[じつ\]aliasを使\[つか\]う。Profileのlanguage列\[れつ\]の並\[なら\]び順\[じゅん\]は意味\[いみ\]に含\[ふく\]めず、aliasから選\[えら\]ぶ対応\[たいおう\]を継続\[けいぞく\]へ保持\[ほじ\]する。alias別\[べつ\]のreader stateとenvironmentも明示\[めいじ\]し、guest不足\[ふそく\]をUnitやhost環境\[かんきょう\]で補\[おぎな\]わない。

<a name="n-62726964676573"></a>

<a name="2-標準bridge"></a>

## 2\. 標準\[ひょうじゅん\]bridge

ここに列挙\[れっきょ\]する言語\[げんご\]と拡張子\[かくちょうし\]は標準構成\[ひょうじゅんこうせい\]であり、追加可能\[ついかかのう\]なlanguage packageの上限\[じょうげん\]ではない。Math LabelのDoc\.Sentenceは既存\[きそん\]bridgeの境界\[きょうかい\]である。Sentenceの最終所有者\[さいしゅうしょゆうしゃ\]はNEPL3sentenceであり、consumer移行\[いこう\]の状態\[じょうたい\]は[23章](<23\-sentence\-annotation\.md>)を参照\[さんしょう\]する。

| host slot | source入口\[いりぐち\] | guest root | 操作\[そうさ\] |
| --- | --- | --- | --- |
| Doc InlineMath | Math | Math\.Expr | lower\/check、17章\[しょう\]の生成\[せいせい\]policyによるrender |
| Doc DisplayMath | Math | Math\.Expr | 同上\[どうじょう\]、display style |
| Doc CircuitFigure | Circuit | Circuit\.Design | lower\/check\/elaborate\/diagram |
| Doc Code | Grammar\/Doc\/Math\/Circuit | 対応\[たいおう\]する根\[ね\] | source\/viewの表示\[ひょうじ\]のみ |
| Math Label | Doc | Doc\.Sentence | lower\/check\/render phrasing |

MathのDoc注記\[ちゅうき\]は文書全体\[ぶんしょぜんたい\]やparagraphを受\[う\]け入\[い\]れない。これでMathMLの注記位置\[ちゅうきいち\]へblock文書\[ぶんしょ\]が混入\[こんにゅう\]しない。Code内\[ない\]の不正\[ふせい\]なguestはRecover構文\[こうぶん\]として保持\[ほじ\]できる。render時\[じ\]にもevaluate\/compileを開始\[かいし\]しない。

言語\[げんご\]の切替\[きりか\]えは一\[ひと\]つの引数\[ひきすう\]の範囲\[はんい\]に限定\[げんてい\]し、終了時\[しゅうりょうじ\]にhostのmodeへ戻\[もど\]る。たとえばDoc\/math\/Math\/add内\[ない\]からDoc sentenceへ戻\[もど\]っても、そのguestの外側\[そとがわ\]のtriviaを読\[よ\]み過\[す\]ぎない。

<a name="n-726563757273696f6e"></a>

<a name="3-相互依存と再帰"></a>

## 3\. 相互依存\[そうごいぞん\]と再帰\[さいき\]

ソース上\[じょう\]の有限\[ゆうげん\]な入\[い\]れ子\[こ\]は許\[ゆる\]す。ForeignSyntaxはroot node、language revision、originと環境参照\[かんきょうさんしょう\]を保持\[ほじ\]する。操作\[そうさ\]グラフは対象\[たいしょう\]nodeとoperationの組\[くみ\]を頂点\[ちょうてん\]に持\[も\]ち、suiteが必要\[ひつよう\]な依存順\[いぞんじゅん\]に処理\[しょり\]する。

Doc→Math→Docの有限\[ゆうげん\]の注記\[ちゅうき\]は循環\[じゅんかん\]ではない。同\[おな\]じnodeのrenderが再\[ふたた\]び自分\[じぶん\]のrenderを要求\[ようきゅう\]する等\[とう\]の循環\[じゅんかん\]はCyclicOperation。生成\[せいせい\]によって構造\[こうぞう\]が増\[ふ\]える場合\[ばあい\]も共通予算\[きょうつうよさん\]とoriginを維持\[いじ\]する。

<a name="n-656e7669726f6e6d656e74"></a>

<a name="4-環境の受渡し"></a>

## 4\. 環境\[かんきょう\]の受渡\[うけわた\]し

各\[かく\]guestの名前空間\[なまえくうかん\]は既定\[きてい\]で新\[あたら\]しく分離\[ぶんり\]する。DocLabel、MathSymbol、CircuitSignalを一\[ひと\]つの名前辞書\[なまえじしょ\]へ入\[い\]れない。bridgeは必要\[ひつよう\]に応\[おう\]じて `EnvironmentProjection` として、どのnamespace\/entityをどの型\[かた\]の外部値\[がいぶち\]として渡\[わた\]すかを明示\[めいじ\]する。

配布\[はいふ\]bridgeはMathのfree symbol値\[あたい\]をRenderContextから明示的\[めいじてき\]に受\[う\]け取\[と\]り、DocLabelとCircuitSignalは自動\[じどう\]exportしない。Doc annotation内\[ない\]のlabelはそのsentenceの局所\[きょくしょ\]scopeで検査\[けんさ\]する。呼出\[よびだ\]し側\[がわ\]のlabel参照\[さんしょう\]が必要\[ひつよう\]ならprofileのprojectionで明示\[めいじ\]する。

<a name="n-617274696661637473"></a>

<a name="5-artifactの準備"></a>

## 5\. artifactの準備\[じゅんび\]

backendsに任意\[にんい\]raw HTML stringを渡\[わた\]さない。suiteがforeign subtreeをtyped MarkupFragmentへ変換\[へんかん\]し、そのslotに適合\[てきごう\]する内容\[ないよう\]モデルを検査\[けんさ\]する。doc\-htmlはPreparedEmbedsの対応表\[たいおうひょう\]を入力\[にゅうりょく\]として受\[う\]け取\[と\]る。math\-mathml\/circuit\-svgを直接\[ちょくせつ\]importしない。

asset参照\[さんしょう\]は固定内容\[こていないよう\]とdigestを持\[も\]つResourceSnapshot。coreがpathから読\[よ\]んだりURLへ接続\[せつぞく\]したりしない。HTML出力\[しゅつりょく\]は既定\[きてい\]で外部\[がいぶ\]network無\[な\]しで閲覧\[えつらん\]できる。KaTeX生成時\[せいせいじ\]は同\[おな\]じ固定版\[こていばん\]のCSS\/fontをartifactへ同梱\[どうこん\]し、相対参照\[そうたいさんしょう\]とlicenseを保持\[ほじ\]する。host生成\[せいせい\]・独立\[どくりつ\]MathML fallback・出力検査\[しゅつりょくけんさ\]・asset identityは[17章\[しょう\]](<17\-math\-html\.md>)に従\[したが\]う。全資源\[ぜんしげん\]を明示的\[めいじてき\]なartifact dependencyとして報告\[ほうこく\]する。

<a name="n-636c69"></a>

<a name="6-cli"></a>

## 6\. CLI

- `nepl3 parse --language doc input.nepld --format ndf|json` はRecover treeと診断\[しんだん\]を出\[だ\]す。
- `nepl3 check input.nepld` は必要\[ひつよう\]なdomain検査\[けんさ\]と参照検査\[さんしょうけんさ\]を行\[おこな\]う。
- `nepl3 render input.nepld --output out.html` は埋\[う\]め込\[こ\]みをprepareしHTMLを出\[だ\]す。
- `nepl3 render input.neplm --output out.html` は生成済\[せいせいず\]み数式\[すうしき\]を含\[ふく\]むHTMLと必要\[ひつよう\]assetを出\[だ\]す。Doc\/Mathのrenderは `--math-renderer katex-preferred|mathml-only` を取\[と\]り、既定\[きてい\]はKaTeX優先\[ゆうせん\]。生成能力不足等\[せいせいのうりょくふそくとう\]のMathML fallbackは診断\[しんだん\]を保持\[ほじ\]する。
- `nepl3 evaluate input.neplm --bindings bindings.ndf` はExact\/Symbolic\/Invalidを構造化出力\[こうぞうかしゅつりょく\]する。
- `nepl3 grammar compile input.neplg --output out.ndf` はLanguagePackageを出\[だ\]す。
- `nepl3 circuit test input.neplc` は全\[ぜん\]testを実行\[じっこう\]する。
- `nepl3 circuit compile input.neplc --target nor --output out.ndf` はNOR IRを出\[だ\]す。
- `nepl3 circuit diagram input.neplc --output out.svg` はSVGを出\[だ\]す。
- `nepl3 format input --style prefix|compact` は明示的\[めいじてき\]なformatter。既定\[きてい\]はstdoutで、\-\-write時\[じ\]だけファイルを置換\[ちかん\]する。

終了\[しゅうりょう\]code\: 0 成功\[せいこう\]（Symbolicは操作\[そうさ\]が許\[ゆる\]す正常結果\[せいじょうけっか\]）、1 入力\[にゅうりょく\]\/検査\[けんさ\]\/テスト失敗\[しっぱい\]、2 CLI\/config\/protocolエラー、3 limit\/cancel、4 provider内部違反\[ないぶいはん\]。stdoutは成果物\[せいかぶつ\]だけ。sourceのdecode失敗\[しっぱい\]を成功空文書\[せいこうからぶんしょ\]にしない。

<a name="n-686f737473"></a>

<a name="7-browsernativewasi"></a>

## 7\. browser\/native\/WASI

同\[おな\]じsuite APIを利用\[りよう\]する。browserではWorkerで計算\[けいさん\]し、未応答時\[みおうとうじ\]はWorkerを終了\[しゅうりょう\]できる。ファイル\/ネットワーク権限\[けんげん\]はWeb shellに限定\[げんてい\]する。wasm\-bindgenのJS undefined\/nullは境界\[きょうかい\]でOption\/Resultへ変換\[へんかん\]し、domainへ流\[なが\]さない。

wasm32\-wasip2 CLIはWASI I\/O adapterを使\[つか\]う。nativeのprocess provider呼出\[よびだ\]しをbrowser\/WASIへ無条件\[むじょうけん\]に持\[も\]ち込\[こ\]まない。該当\[がいとう\]hostが提供\[ていきょう\]するregistry\/runnerのcapabilityを検査\[けんさ\]する。組込\[くみこ\]み4言語\[げんご\]の基本操作\[きほんそうさ\]は全\[ぜん\]targetで使\[つか\]える。
