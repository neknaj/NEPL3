<!-- Generated from doc/spec/14&#45;web&#45;ui.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page web&#45;ui; source SHA-256 9ede4845c82e3fb0dc0711f943c5714df3426abca964e2570c87eae4555a64ae; alias input SHA-256 76af7a32409a225e5a16830c4db4c4f9d5ffb1fa1c7c8ae94ab60a443ef1f843; document digest c02585e06a44c67984e2134133c2f052efff540cdca5f2f35df6d0bd51886e9a; input PageSet digest 1b5e3849c4d1f347ca6b59e97d9d1ae4c0244cc6657f3d39aa12480838b9c40d; input context SHA-256 9a8c521bcbe028c7be3bf71358a9de1352f11057e9787ac4bf04245bcd480b31. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="14-web-uiteaplayground"></a>

# 14\. Web UI・TEA・Playground

<a name="n-726573706f6e736962696c6974696573"></a>

<a name="1-責務と実装範囲"></a>

## 1\. 責務\[せきむ\]と実装範囲\[じっそうはんい\]

NEPL3の共通構文基盤\[きょうつうこうぶんきばん\]と言語\[げんご\]compositionをブラウザWasmから利用\[りよう\]するPlaygroundを、最終成果物\[さいしゅうせいかぶつ\]に含\[ふく\]める。既存\[きそん\]4言語\[げんご\]はreference profileとして扱\[あつか\]い、特定\[とくてい\]guest languageの専用\[せんよう\]IDEや閉\[と\]じた言語一覧\[げんごいちらん\]を共通\[きょうつう\]UIのモデルにしない。専用\[せんよう\]の言語処理\[げんごしょり\]server、localhost LSP、WebSocketを利用条件\[りようじょうけん\]にしない。T15はWorker\/Wasm実行境界\[じっこうきょうかい\]、T17は純粋\[じゅんすい\]UI core、T18はWeb editorと操作画面\[そうさがめん\]を所有\[しょゆう\]する。現在\[げんざい\]は計画\[けいかく\]であり、動\[うご\]く画面\[がめん\]・Wasm・完成\[かんせい\]したUI schemaを提供\[ていきょう\]した状態\[じょうたい\]ではない。

`nepl3-ui-core` は `no_std + alloc` とし、Model、Msg、Cmd、SubscriptionSet、ViewModelと純粋\[じゅんすい\]な状態遷移\[じょうたいせんい\]を所有\[しょゆう\]する。依存\[いぞん\]は、共通\[きょうつう\]coreの値\[あたい\]・source・操作\[そうさ\]データ契約\[けいやく\]に限定\[げんてい\]する。suite、各言語実装\[かくげんごじっそう\]、DOM、Web API、LSP transportを呼\[よ\]ばない。共通操作型\[きょうつうそうさがた\]を閉\[と\]じるR006と、解決済\[かいけつず\]みProfileを型\[かた\]として定\[さだ\]めるR009を先\[さき\]に解消\[かいしょう\]し、UI型\[がた\]の穴\[あな\]を万能辞書\[ばんのうじしょ\]やRust pointerで埋\[う\]めない。

概念上\[がいねんじょう\]の操作\[そうさ\]は `init(configuration) -> (Model, Commands)`、`update(Model, Msg) -> (Model, Commands)`、`view(Model) -> ViewModel`、`subscriptions(Model) -> SubscriptionSet` である。これらは責務\[せきむ\]のsignatureであり、未確定\[みかくてい\]のpayloadを実行可能\[じっこうかのう\]schemaとして広告\[こうこく\]しない。`interfaces/ui/` に言語中立\[げんごちゅうりつ\]のfield・variant・不変条件\[ふへんじょうけん\]・失敗\[しっぱい\]を定義\[ていぎ\]し、Rust型\[がた\]とwire経路\[けいろ\]、別実装\[べつじっそう\]replayを同時\[どうじ\]に実装\[じっそう\]する。

update\/viewはclock、乱数\[らんすう\]、DOM、I\/O、Worker、GPUを利用\[りよう\]しない。解析\[かいせき\]・評価\[ひょうか\]・回路\[かいろ\]stepも、Cmdでhostへ委譲\[いじょう\]する。hostは実行結果\[じっこうけっか\]をMsgへ変換\[へんかん\]する。保存要求\[ほぞんようきゅう\]の発行\[はっこう\]だけで保存済\[ほぞんず\]みにはせず、SaveFinished\(Result\)の対象\[たいしょう\]snapshotを照合\[しょうごう\]して反映\[はんえい\]する。timerなどのsubscriptionも論理的\[ろんりてき\]な要求\[ようきゅう\]であり、登録\[とうろく\]・解除\[かいじょ\]・実時刻\[じつじこく\]の取得\[しゅとく\]はhostが行\[おこな\]う。同一\[どういつ\]Modelと同一\[どういつ\]Msg列\[れつ\]から、Model\/Cmd\/View\/Subscriptionの意味正規形\[いみせいきけい\]が一致\[いっち\]することを検証\[けんしょう\]する。

Cmdに任意\[にんい\]closureを格納\[かくのう\]して、作用\[さよう\]を隠\[かく\]さない。局所\[きょくしょ\]mutationは、外部状態\[がいぶじょうたい\]を変\[か\]えなければ許容\[きょよう\]する。子\[こ\]Model\/Msgへ分割\[ぶんかつ\]でき、巨大\[きょだい\]な一枚\[いちまい\]のupdateや汎用\[はんよう\]widget frameworkの自作\[じさく\]を要求\[ようきゅう\]しない。subscriptionはIDで差分適用\[さぶんてきよう\]し、listener\/Promise\/timerからModelを直接書\[ちょくせつか\]き換\[か\]えない。入力\[にゅうりょく\]・選択\[せんたく\]の反映\[はんえい\]まで、解析\[かいせき\]debounceに巻\[ま\]き込\[こ\]まない。

<a name="n-6c61796f7574"></a>

<a name="2-配置とhost"></a>

## 2\. 配置\[はいち\]とhost

`crates/ui/core/src/` はmodel\/message\/update\/command\/subscription\/viewへ責務\[せきむ\]を分\[わ\]ける。`apps/web/` はRust\/Wasm facade、`web/src/` はshell\/editor\/worker\/preview\/storage\/bindingsの薄\[うす\]いTypeScript adapterを置\[お\]く。画面\[がめん\]の状態遷移\[じょうたいせんい\]、parser、束縛\[そくばく\]・名前解決\[なまえかいけつ\]をTypeScriptへ再実装\[さいじっそう\]しない。将来\[しょうらい\]のnative UIも同\[おな\]じcoreを使\[つか\]えるが、専用\[せんよう\]native GUI製品\[せいひん\]を新\[あたら\]しい必須成果物\[ひっすせいかぶつ\]にはしない。

TypeScript境界\[きょうかい\]はnull\/undefined・未知\[みち\]payloadを検査\[けんさ\]し、Option\/Resultへ変換\[へんかん\]する。未検査\[みけんさ\]の型\[かた\]assertionで、公開\[こうかい\]schemaを通過\[つうか\]させない。editor\/bundlerなどの依存\[いぞん\]は通常\[つうじょう\]のpackage manifestとlockfileで固定\[こてい\]し、生成\[せいせい\]bindingsとschemaの一致\[いっち\]を検査\[けんさ\]する。

<a name="n-6173796e6368726f6e6f7573"></a>

<a name="3-非同期の同一性と停止"></a>

## 3\. 非同期\[ひどうき\]の同一性\[どういつせい\]と停止\[ていし\]

各要求\[かくようきゅう\]・応答\[おうとう\]は `sessionEpoch`、`workerEpoch`、`requestId`、`sourceSetIdentity`、`profileIdentity`、`operationIdentity`、`optionsAndResourcesIdentity` を照合\[しょうごう\]する。source集合\[しゅうごう\]には、document identityとsnapshot revision\/digestを含\[ふく\]める。profileには、実際\[じっさい\]のschema\/package\/provider revision・digestを含\[ふく\]める。各結果\[かくけっか\]slotは、現在待\[げんざいま\]っている要求\[ようきゅう\]との完全一致\[かんぜんいっち\]だけを受理\[じゅり\]する。

応答逆転\[おうとうぎゃくてん\]、cancel後\[ご\]の成功応答\[せいこうおうとう\]、close\/reopen、Worker再作成\[さいさくせい\]、profile\/provider\/options\/resources変更\[へんこう\]で、旧結果\[きゅうけっか\]を採用\[さいよう\]しない。単一\[たんいつ\]のsource revisionだけで比較\[ひかく\]しない。履歴\[りれき\]として表示\[ひょうじ\]する場合\[ばあい\]は、対象\[たいしょう\]snapshotと旧結果\[きゅうけっか\]であることを明示\[めいじ\]し、現在\[げんざい\]のrename・diagnostic・previewへ適用\[てきよう\]しない。request IDを再利用\[さいりよう\]するときも、epochで隔離\[かくり\]する。

協調\[きょうちょう\]cancelはbudget\/pollで処理\[しょり\]するが、長\[なが\]い同期\[どうき\]Wasm実行中\[じっこうちゅう\]に通常\[つうじょう\]のpostMessageが直\[ただ\]ちに処理\[しょり\]されるとは仮定\[かてい\]しない。hostは定義済\[ていぎず\]みの停止期限\[ていしきげん\]でWorkerをterminateし、epochを進\[すす\]めて再作成\[さいさくせい\]する。終了\[しゅうりょう\]したWorker内\[ない\]の状態\[じょうたい\]・continuationを再利用\[さいりよう\]せず、必要\[ひつよう\]なsnapshot\/profileを新\[あたら\]しいWorkerへ送\[おく\]り直\[なお\]す。破棄\[はき\]と再起動\[さいきどう\]の途中\[とちゅう\]も、UIの処理状態\[しょりじょうたい\]を失敗\[しっぱい\]・停止\[ていし\]・再準備\[さいじゅんび\]として表\[あらわ\]し、成功\[せいこう\]へ置\[お\]き換\[か\]えない。

Workerだけで完全\[かんぜん\]なsecurity sandboxが成立\[せいりつ\]するとは扱\[あつか\]わず、allowlist・入力予算\[にゅうりょくよさん\]・resource権限\[けんげん\]を維持\[いじ\]する。標準利用\[ひょうじゅんりよう\]にSharedArrayBufferや追加\[ついか\]server headerを必須\[ひっす\]としない。終了\[しゅうりょう\]により部分結果\[ぶぶんけっか\]も失\[うしな\]った場合\[ばあい\]は、その事実\[じじつ\]を表示\[ひょうじ\]する。

<a name="n-656469746f72"></a>

<a name="4-editorのtransaction"></a>

## 4\. Editorのtransaction

editor widgetは、caret、IME composition、undo\/redo、layout cacheという局所状態\[きょくしょじょうたい\]を持\[も\]ってよい。文書内容\[ぶんしょないよう\]の正本\[せいほん\]は、snapshot契約\[けいやく\]で一\[ひと\]つにする。adapterはtransaction ID、base snapshot、変更\[へんこう\]byte range、置換\[ちかん\]Text、新\[しん\]snapshotを照合\[しょうごう\]し、programmatic editの再通知\[さいつうち\]を同\[おな\]じtransactionとして処理\[しょり\]する。

IME中\[ちゅう\]にpreview更新\[こうしん\]でeditorを再生成\[さいせいせい\]したり、無条件\[むじょうけん\]の全文置換\[ぜんぶんちかん\]をしたりしない。composition中\[ちゅう\]の一時入力\[いちじにゅうりょく\]と確定\[かくてい\]transactionを区別\[くべつ\]し、確定\[かくてい\]まで安全\[あんぜん\]に保留\[ほりゅう\]する操作\[そうさ\]を定義\[ていぎ\]する。renameは古\[ふる\]いsnapshotを拒否\[きょひ\]し、複数編集\[ふくすうへんしゅう\]を一\[ひと\]つのundo transactionとして適用\[てきよう\]する。選択範囲\[せんたくはんい\]のUTF\-16と内部\[ないぶ\]UTF\-8 byte offsetの変換\[へんかん\]はadapterで行\[おこな\]い、日本語\[にほんご\]・補助平面文字\[ほじょへいめんもじ\]・CRLF・undo\/redo・外部読込\[がいぶよみこ\]みと競合\[きょうごう\]する編集\[へんしゅう\]を検証\[けんしょう\]する。

毎\[まい\]eventで巨大\[きょだい\]sourceやASTを複製\[ふくせい\]しない。不変\[ふへん\]snapshot handle、差分\[さぶん\]、共有\[きょうゆう\]データを使用\[しよう\]し、公開\[こうかい\]wireではpointerを運\[はこ\]ばない。WidgetとModelに、独立\[どくりつ\]したsource正本\[せいほん\]を持\[も\]たせない。

<a name="n-6f7065726174696f6e73"></a>

<a name="5-言語compositionとreference-profileの操作"></a>

<a name="5-四言語の操作"></a>

## 5\. 言語\[げんご\]compositionとreference profileの操作\[そうさ\]

Playgroundでは最小\[さいしょう\]の独立\[どくりつ\]LanguagePackageを作成\[さくせい\]し、共通基盤\[きょうつうきばん\]やUIの言語名分岐\[げんごめいぶんき\]を変更\[へんこう\]せず登録\[とうろく\]して、そのsourceをparse\/check\/printできることを検証\[けんしょう\]する。head\/arity、先行構文\[せんこうこうぶん\]による後続\[こうぞく\]contextの更新\[こうしん\]、外国語構文\[がいこくごこうぶん\]の境界\[きょうかい\]、Source\/Originと診断\[しんだん\]を観察\[かんさつ\]できるようにする。

さらに別\[べつ\]LanguagePackageをimport\/compositionし、単一\[たんいつ\]source内\[ない\]の多階層埋\[たかいそうう\]め込\[こ\]みを共通操作経路\[きょうつうそうさけいろ\]で扱\[あつか\]う。sentenceやannotationも独立\[どくりつ\]した基礎言語\[きそげんご\]として利用\[りよう\]し、foundationやeditorの特別構文\[とくべつこうぶん\]へ内蔵\[ないぞう\]しない。annotationでは対象\[たいしょう\]syntaxとの関係\[かんけい\]と、対象\[たいしょう\]のbinding・domain意味\[いみ\]の保持\[ほじ\]を確認\[かくにん\]する。未実装\[みじっそう\]package・provider・操作\[そうさ\]は能力不足\[のうりょくぶそく\]を示\[しめ\]し、受入済\[うけいれず\]みとしない。

以下\[いか\]の4言語\[げんご\]の既存操作要件\[きそんそうさようけん\]はreference profileの受入\[うけいれ\]として維持\[いじ\]する。追加言語\[ついかげんご\]の登録\[とうろく\]とcompositionの受入\[うけいれ\]を、これらの固定例\[こていれい\]だけで代替\[だいたい\]しない。Playground\/Tutorialの共通原理\[きょうつうげんり\]と各言語固有\[かくげんごこゆう\]referenceの所有\[しょゆう\]は、[Pages情報設計\[じょうほうせっけい\]](<\.\.\/decisions\/pages\-information\-architecture\.md>)に従\[したが\]う。

| 言語\[げんご\] | 必須操作\[ひっすそうさ\]と表示\[ひょうじ\] |
| :--- | :--- |
| Grammar | 文法\[ぶんぽう\]の編集\[へんしゅう\]・検査\[けんさ\]・package生成\[せいせい\]、対象\[たいしょう\]DSL sourceの別\[べつ\]editor、生成\[せいせい\]した文法\[ぶんぽう\]でparse、highlight、定義\[ていぎ\]ジャンプ |
| Doc | sentence literal\/prefix、Ruby\/Anno、sentence単位\[たんい\]parallel、HTML preview、artifact取得\[しゅとく\] |
| Math | 元\[もと\]の式\[しき\]、bindings入力\[にゅうりょく\]、明示評価\[めいじひょうか\]、Exact\/Symbolic\/Invalid\/Stopped、HTML\/MathML artifact、KaTeX優先\[ゆうせん\]\/MathMLのみの設定\[せってい\] |
| Circuit | 接続\[せつぞく\]・幅検査\[はばけんさ\]、入力変更\[にゅうりょくへんこう\]、initial\/step\/reset、現在\[げんざい\]state\/output、test、SVG、NOR IR |
| 共通\[きょうつう\] | 例選択\[れいせんたく\]、source入出力\[にゅうしゅつりょく\]、診断一覧\[しんだんいちらん\]と位置移動\[いちいどう\]、処理状態\[しょりじょうたい\]\/cancel、成果物入出力\[せいかぶつにゅうしゅつりょく\] |

共通\[きょうつう\]editorは、highlight、definition、references、rename、completion、対応文\[たいおうぶん\]・生成元\[せいせいもと\]への移動\[いどう\]も提供\[ていきょう\]する。全文\[ぜんぶん\]sourceをdebug logへ既定出力\[きていしゅつりょく\]しない。ブラウザに存在\[そんざい\]しないproviderは能力不足\[のうりょくぶそく\]として示\[しめ\]し、workspaceからnative pluginや任意\[にんい\]JSを自動生成\[じどうせいせい\]・実行\[じっこう\]しない。

追加\[ついか\]DSLのeditor支援\[しえん\]は、Grammarの構文\[こうぶん\]・束縛\[そくばく\]・表示定義\[ひょうじていぎ\]と共通\[きょうつう\]engineから得\[え\]る。追加\[ついか\]ごとのTypeScript keyword表\[ひょう\]や名前解決器\[なまえかいけつき\]を要求\[ようきゅう\]しない。言語固有\[げんごこゆう\]の操作\[そうさ\]panelは許\[ゆる\]すが、未知\[みち\]DSLのsimulatorやrendererを自動生成\[じどうせいせい\]した扱\[あつか\]いにはしない。機能\[きのう\]のない操作\[そうさ\]は、選択肢\[せんたくし\]として成功\[せいこう\]stubを表示\[ひょうじ\]しない。

<a name="n-70726576696577"></a>

<a name="6-previewとsourceの保護"></a>

## 6\. Previewとsourceの保護\[ほご\]

source表示\[ひょうじ\]からguest評価\[ひょうか\]を開始\[かいし\]しない。Doc\/Math\/Circuitの表示\[ひょうじ\]は仕様\[しよう\]で選\[えら\]んだ操作\[そうさ\]だけを実行\[じっこう\]し、Math評価\[ひょうか\]・Circuit step・Grammar compileは明示\[めいじ\]した操作経路\[そうさけいろ\]に置\[お\]く。例\[れい\]の読\[よ\]み込\[こ\]みやURLの変更\[へんこう\]だけで、危険\[きけん\]なproviderを許可\[きょか\]しない。

自動更新\[じどうこうしん\]は、parse\/analyzeと明示的\[めいじてき\]に許可\[きょか\]されたbounded previewに限\[かぎ\]る。高負荷評価\[こうふかひょうか\]、回路\[かいろ\]step\/test、外部作用\[がいぶさよう\]は明示\[めいじ\]Msgから開始\[かいし\]し、例\[れい\]の表示\[ひょうじ\]を自動評価\[じどうひょうか\]の同意\[どうい\]として扱\[あつか\]わない。

previewは検査済\[けんさず\]みMarkup artifactだけを、scriptを許\[ゆる\]さない隔離\[かくり\]されたiframeへ渡\[わた\]す。top navigation、form送信\[そうしん\]、任意\[にんい\]network、同\[どう\]origin権限\[けんげん\]の付与\[ふよ\]を避\[さ\]け、hostとの連携\[れんけい\]は検証\[けんしょう\]したmessageと明示\[めいじ\]したIDだけを受\[う\]ける。例外的\[れいがいてき\]な機能\[きのう\]を必要\[ひつよう\]とするなら、trust契約\[けいやく\]を先\[さき\]に変更\[へんこう\]する。外部\[がいぶ\]providerの許可\[きょか\]とpreviewの隔離\[かくり\]は、別\[べつ\]に検査\[けんさ\]する。

sourceへの移動\[いどう\]はhostの診断\[しんだん\]・構造\[こうぞう\]paneから提供\[ていきょう\]でき、iframe内\[ない\]のscriptやpostMessage発行\[はっこう\]を必要\[ひつよう\]としない。hostが受信\[じゅしん\]するその他\[ほか\]の連携\[れんけい\]messageにも、origin・payload・要求\[ようきゅう\]identity検査\[けんさ\]を適用\[てきよう\]する。

数式\[すうしき\]は[17章\[しょう\]](<17\-math\-html\.md>)に従\[したが\]いWorker内\[ない\]のKaTeX adapterで生成\[せいせい\]し、同\[おな\]じ完成\[かんせい\]artifactをpreviewと書出\[かきだ\]しへ渡\[わた\]す。iframe内\[ない\]でKaTeXを再実行\[さいじっこう\]せず、CSS\/fontもその文書\[ぶんしょ\]に適用\[てきよう\]する。完全要求\[かんぜんようきゅう\]identity・停止\[ていし\]・設定\[せってい\]・asset診断\[しんだん\]を、TEAのMsg\/Cmdへ統合\[とうごう\]する。JavaScript無効時\[むこうじ\]の対話生成不可\[たいわせいせいふか\]と、書\[か\]き出\[だ\]し済\[ず\]み文書\[ぶんしょ\]のJavaScript不要\[ふよう\]な閲覧\[えつらん\]を区別\[くべつ\]する。

編集中\[へんしゅうちゅう\]sourceを、無断\[むだん\]でnetwork送信\[そうしん\]しない。読込\[よみこ\]み・保存\[ほぞん\]・download・共有\[きょうゆう\]はhostの明示操作\[めいじそうさ\]とし、失敗\[しっぱい\]、保存容量上限\[ほぞんようりょうじょうげん\]、権限拒否\[けんげんきょひ\]を状態\[じょうたい\]に反映\[はんえい\]する。未保存編集\[みほぞんへんしゅう\]を例選択\[れいせんたく\]で上書\[うわが\]きする場合\[ばあい\]の確認\[かくにん\]は、実装契約\[じっそうけいやく\]として設\[もう\]ける。

標準\[ひょうじゅん\]assetは同\[おな\]じ公開\[こうかい\]artifactから取得\[しゅとく\]し、runtimeのCDN取得\[しゅとく\]を前提\[ぜんてい\]にしない。local storageは、project\/document\/schema版\[ばん\]でnamespaceを分\[わ\]ける。例共有\[れいきょうゆう\]は例\[れい\]IDを基本\[きほん\]にし、ユーザーsourceをURL queryへ自動追加\[じどうついか\]しない。明示共有\[めいじきょうゆう\]のfragment\/importも、容量\[ようりょう\]とdecode予算\[よさん\]を検査\[けんさ\]する。

<a name="n-7265666572656e636573"></a>

<a name="7-根拠"></a>

## 7\. 根拠\[こんきょ\]

Model\/View\/Updateの分離\[ぶんり\]は [Elm公式\[こうしき\]ガイド](<https\:\/\/guide\.elm\-lang\.org\/architecture\/>) に基\[もと\]づく。Workerの終了\[しゅうりょう\]は [HTML Standard](<https\:\/\/html\.spec\.whatwg\.org\/multipage\/workers\.html\#dom\-worker\-terminate>) と [MDN](<https\:\/\/developer\.mozilla\.org\/en\-US\/docs\/Web\/API\/Worker\/terminate>) を参照\[さんしょう\]する。採用\[さいよう\]する型境界\[かたきょうかい\]とcross\-platform検証\[けんしょう\]は [ユーザーの設計指針\[せっけいししん\]](<https\:\/\/zenn\.dev\/bem130\/articles\/1b352797de94e7>) に合\[あ\]わせる。Web専用\[せんよう\]の副作用\[ふくさよう\]を純粋\[じゅんすい\]coreへ混\[ま\]ぜず、未検証\[みけんしょう\]の対応環境\[たいおうかんきょう\]を完成扱\[かんせいあつか\]いしない。
