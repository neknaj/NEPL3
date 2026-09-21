<!-- Generated from doc/spec/14&#45;web&#45;ui.nepld; renderer nepl3-tools.markdown-annotated-pages/2; page web&#45;ui; source SHA-256 9ede4845c82e3fb0dc0711f943c5714df3426abca964e2570c87eae4555a64ae; alias input SHA-256 76af7a32409a225e5a16830c4db4c4f9d5ffb1fa1c7c8ae94ab60a443ef1f843; document digest c02585e06a44c67984e2134133c2f052efff540cdca5f2f35df6d0bd51886e9a; input PageSet digest 06af63f0629d28f3f3cd380a0752cc882b4d35375ddc9f7066beec70c8a60944; input context SHA-256 ed700e2f76ece9ddda9c24308c5cfe29bfa6379c4cfbb30fc5683c91654ccd7a. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="14-web-uiteaplayground"></a>

# 14\. Web UI・TEA・Playground

[正本（NEPL3d）](<14-web-ui.nepld>)

<a name="n-726573706f6e736962696c6974696573"></a>

<a name="1-責務と実装範囲"></a>

## 1\. <ruby>責務<rt>せきむ</rt></ruby>と<ruby>実装範囲<rt>じっそうはんい</rt></ruby>

NEPL3の<ruby>共通構文基盤<rt>きょうつうこうぶんきばん</rt></ruby>と<ruby>言語<rt>げんご</rt></ruby>compositionをブラウザWasmから<ruby>利用<rt>りよう</rt></ruby>するPlaygroundを、<ruby>最終成果物<rt>さいしゅうせいかぶつ</rt></ruby>に<ruby>含<rt>ふく</rt></ruby>める。<ruby>既存<rt>きそん</rt></ruby>4<ruby>言語<rt>げんご</rt></ruby>はreference profileとして<ruby>扱<rt>あつか</rt></ruby>い、<ruby>特定<rt>とくてい</rt></ruby>guest languageの<ruby>専用<rt>せんよう</rt></ruby>IDEや<ruby>閉<rt>と</rt></ruby>じた<ruby>言語一覧<rt>げんごいちらん</rt></ruby>を<ruby>共通<rt>きょうつう</rt></ruby>UIのモデルにしない。<ruby>専用<rt>せんよう</rt></ruby>の<ruby>言語処理<rt>げんごしょり</rt></ruby>server、localhost LSP、WebSocketを<ruby>利用条件<rt>りようじょうけん</rt></ruby>にしない。T15はWorker\/Wasm<ruby>実行境界<rt>じっこうきょうかい</rt></ruby>、T17は<ruby>純粋<rt>じゅんすい</rt></ruby>UI core、T18はWeb editorと<ruby>操作画面<rt>そうさがめん</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>する。<ruby>現在<rt>げんざい</rt></ruby>は<ruby>計画<rt>けいかく</rt></ruby>であり、<ruby>動<rt>うご</rt></ruby>く<ruby>画面<rt>がめん</rt></ruby>・Wasm・<ruby>完成<rt>かんせい</rt></ruby>したUI schemaを<ruby>提供<rt>ていきょう</rt></ruby>した<ruby>状態<rt>じょうたい</rt></ruby>ではない。

`nepl3-ui-core` は `no_std + alloc` とし、Model、Msg、Cmd、SubscriptionSet、ViewModelと<ruby>純粋<rt>じゅんすい</rt></ruby>な<ruby>状態遷移<rt>じょうたいせんい</rt></ruby>を<ruby>所有<rt>しょゆう</rt></ruby>する。<ruby>依存<rt>いぞん</rt></ruby>は、<ruby>共通<rt>きょうつう</rt></ruby>coreの<ruby>値<rt>あたい</rt></ruby>・source・<ruby>操作<rt>そうさ</rt></ruby>データ<ruby>契約<rt>けいやく</rt></ruby>に<ruby>限定<rt>げんてい</rt></ruby>する。suite、<ruby>各言語実装<rt>かくげんごじっそう</rt></ruby>、DOM、Web API、LSP transportを<ruby>呼<rt>よ</rt></ruby>ばない。<ruby>共通操作型<rt>きょうつうそうさがた</rt></ruby>を<ruby>閉<rt>と</rt></ruby>じるR006と、<ruby>解決済<rt>かいけつず</rt></ruby>みProfileを<ruby>型<rt>かた</rt></ruby>として<ruby>定<rt>さだ</rt></ruby>めるR009を<ruby>先<rt>さき</rt></ruby>に<ruby>解消<rt>かいしょう</rt></ruby>し、UI<ruby>型<rt>がた</rt></ruby>の<ruby>穴<rt>あな</rt></ruby>を<ruby>万能辞書<rt>ばんのうじしょ</rt></ruby>やRust pointerで<ruby>埋<rt>う</rt></ruby>めない。

<ruby>概念上<rt>がいねんじょう</rt></ruby>の<ruby>操作<rt>そうさ</rt></ruby>は `init(configuration) -> (Model, Commands)`、`update(Model, Msg) -> (Model, Commands)`、`view(Model) -> ViewModel`、`subscriptions(Model) -> SubscriptionSet` である。これらは<ruby>責務<rt>せきむ</rt></ruby>のsignatureであり、<ruby>未確定<rt>みかくてい</rt></ruby>のpayloadを<ruby>実行可能<rt>じっこうかのう</rt></ruby>schemaとして<ruby>広告<rt>こうこく</rt></ruby>しない。`interfaces/ui/` に<ruby>言語中立<rt>げんごちゅうりつ</rt></ruby>のfield・variant・<ruby>不変条件<rt>ふへんじょうけん</rt></ruby>・<ruby>失敗<rt>しっぱい</rt></ruby>を<ruby>定義<rt>ていぎ</rt></ruby>し、Rust<ruby>型<rt>がた</rt></ruby>とwire<ruby>経路<rt>けいろ</rt></ruby>、<ruby>別実装<rt>べつじっそう</rt></ruby>replayを<ruby>同時<rt>どうじ</rt></ruby>に<ruby>実装<rt>じっそう</rt></ruby>する。

update\/viewはclock、<ruby>乱数<rt>らんすう</rt></ruby>、DOM、I\/O、Worker、GPUを<ruby>利用<rt>りよう</rt></ruby>しない。<ruby>解析<rt>かいせき</rt></ruby>・<ruby>評価<rt>ひょうか</rt></ruby>・<ruby>回路<rt>かいろ</rt></ruby>stepも、Cmdでhostへ<ruby>委譲<rt>いじょう</rt></ruby>する。hostは<ruby>実行結果<rt>じっこうけっか</rt></ruby>をMsgへ<ruby>変換<rt>へんかん</rt></ruby>する。<ruby>保存要求<rt>ほぞんようきゅう</rt></ruby>の<ruby>発行<rt>はっこう</rt></ruby>だけで<ruby>保存済<rt>ほぞんず</rt></ruby>みにはせず、SaveFinished\(Result\)の<ruby>対象<rt>たいしょう</rt></ruby>snapshotを<ruby>照合<rt>しょうごう</rt></ruby>して<ruby>反映<rt>はんえい</rt></ruby>する。timerなどのsubscriptionも<ruby>論理的<rt>ろんりてき</rt></ruby>な<ruby>要求<rt>ようきゅう</rt></ruby>であり、<ruby>登録<rt>とうろく</rt></ruby>・<ruby>解除<rt>かいじょ</rt></ruby>・<ruby>実時刻<rt>じつじこく</rt></ruby>の<ruby>取得<rt>しゅとく</rt></ruby>はhostが<ruby>行<rt>おこな</rt></ruby>う。<ruby>同一<rt>どういつ</rt></ruby>Modelと<ruby>同一<rt>どういつ</rt></ruby>Msg<ruby>列<rt>れつ</rt></ruby>から、Model\/Cmd\/View\/Subscriptionの<ruby>意味正規形<rt>いみせいきけい</rt></ruby>が<ruby>一致<rt>いっち</rt></ruby>することを<ruby>検証<rt>けんしょう</rt></ruby>する。

Cmdに<ruby>任意<rt>にんい</rt></ruby>closureを<ruby>格納<rt>かくのう</rt></ruby>して、<ruby>作用<rt>さよう</rt></ruby>を<ruby>隠<rt>かく</rt></ruby>さない。<ruby>局所<rt>きょくしょ</rt></ruby>mutationは、<ruby>外部状態<rt>がいぶじょうたい</rt></ruby>を<ruby>変<rt>か</rt></ruby>えなければ<ruby>許容<rt>きょよう</rt></ruby>する。<ruby>子<rt>こ</rt></ruby>Model\/Msgへ<ruby>分割<rt>ぶんかつ</rt></ruby>でき、<ruby>巨大<rt>きょだい</rt></ruby>な<ruby>一枚<rt>いちまい</rt></ruby>のupdateや<ruby>汎用<rt>はんよう</rt></ruby>widget frameworkの<ruby>自作<rt>じさく</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>しない。subscriptionはIDで<ruby>差分適用<rt>さぶんてきよう</rt></ruby>し、listener\/Promise\/timerからModelを<ruby>直接書<rt>ちょくせつか</rt></ruby>き<ruby>換<rt>か</rt></ruby>えない。<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>選択<rt>せんたく</rt></ruby>の<ruby>反映<rt>はんえい</rt></ruby>まで、<ruby>解析<rt>かいせき</rt></ruby>debounceに<ruby>巻<rt>ま</rt></ruby>き<ruby>込<rt>こ</rt></ruby>まない。

<a name="n-6c61796f7574"></a>

<a name="2-配置とhost"></a>

## 2\. <ruby>配置<rt>はいち</rt></ruby>とhost

`crates/ui/core/src/` はmodel\/message\/update\/command\/subscription\/viewへ<ruby>責務<rt>せきむ</rt></ruby>を<ruby>分<rt>わ</rt></ruby>ける。`apps/web/` はRust\/Wasm facade、`web/src/` はshell\/editor\/worker\/preview\/storage\/bindingsの<ruby>薄<rt>うす</rt></ruby>いTypeScript adapterを<ruby>置<rt>お</rt></ruby>く。<ruby>画面<rt>がめん</rt></ruby>の<ruby>状態遷移<rt>じょうたいせんい</rt></ruby>、parser、<ruby>束縛<rt>そくばく</rt></ruby>・<ruby>名前解決<rt>なまえかいけつ</rt></ruby>をTypeScriptへ<ruby>再実装<rt>さいじっそう</rt></ruby>しない。<ruby>将来<rt>しょうらい</rt></ruby>のnative UIも<ruby>同<rt>おな</rt></ruby>じcoreを<ruby>使<rt>つか</rt></ruby>えるが、<ruby>専用<rt>せんよう</rt></ruby>native GUI<ruby>製品<rt>せいひん</rt></ruby>を<ruby>新<rt>あたら</rt></ruby>しい<ruby>必須成果物<rt>ひっすせいかぶつ</rt></ruby>にはしない。

TypeScript<ruby>境界<rt>きょうかい</rt></ruby>はnull\/undefined・<ruby>未知<rt>みち</rt></ruby>payloadを<ruby>検査<rt>けんさ</rt></ruby>し、Option\/Resultへ<ruby>変換<rt>へんかん</rt></ruby>する。<ruby>未検査<rt>みけんさ</rt></ruby>の<ruby>型<rt>かた</rt></ruby>assertionで、<ruby>公開<rt>こうかい</rt></ruby>schemaを<ruby>通過<rt>つうか</rt></ruby>させない。editor\/bundlerなどの<ruby>依存<rt>いぞん</rt></ruby>は<ruby>通常<rt>つうじょう</rt></ruby>のpackage manifestとlockfileで<ruby>固定<rt>こてい</rt></ruby>し、<ruby>生成<rt>せいせい</rt></ruby>bindingsとschemaの<ruby>一致<rt>いっち</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。

<a name="n-6173796e6368726f6e6f7573"></a>

<a name="3-非同期の同一性と停止"></a>

## 3\. <ruby>非同期<rt>ひどうき</rt></ruby>の<ruby>同一性<rt>どういつせい</rt></ruby>と<ruby>停止<rt>ていし</rt></ruby>

<ruby>各要求<rt>かくようきゅう</rt></ruby>・<ruby>応答<rt>おうとう</rt></ruby>は `sessionEpoch`、`workerEpoch`、`requestId`、`sourceSetIdentity`、`profileIdentity`、`operationIdentity`、`optionsAndResourcesIdentity` を<ruby>照合<rt>しょうごう</rt></ruby>する。source<ruby>集合<rt>しゅうごう</rt></ruby>には、document identityとsnapshot revision\/digestを<ruby>含<rt>ふく</rt></ruby>める。profileには、<ruby>実際<rt>じっさい</rt></ruby>のschema\/package\/provider revision・digestを<ruby>含<rt>ふく</rt></ruby>める。<ruby>各結果<rt>かくけっか</rt></ruby>slotは、<ruby>現在待<rt>げんざいま</rt></ruby>っている<ruby>要求<rt>ようきゅう</rt></ruby>との<ruby>完全一致<rt>かんぜんいっち</rt></ruby>だけを<ruby>受理<rt>じゅり</rt></ruby>する。

<ruby>応答逆転<rt>おうとうぎゃくてん</rt></ruby>、cancel<ruby>後<rt>ご</rt></ruby>の<ruby>成功応答<rt>せいこうおうとう</rt></ruby>、close\/reopen、Worker<ruby>再作成<rt>さいさくせい</rt></ruby>、profile\/provider\/options\/resources<ruby>変更<rt>へんこう</rt></ruby>で、<ruby>旧結果<rt>きゅうけっか</rt></ruby>を<ruby>採用<rt>さいよう</rt></ruby>しない。<ruby>単一<rt>たんいつ</rt></ruby>のsource revisionだけで<ruby>比較<rt>ひかく</rt></ruby>しない。<ruby>履歴<rt>りれき</rt></ruby>として<ruby>表示<rt>ひょうじ</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>対象<rt>たいしょう</rt></ruby>snapshotと<ruby>旧結果<rt>きゅうけっか</rt></ruby>であることを<ruby>明示<rt>めいじ</rt></ruby>し、<ruby>現在<rt>げんざい</rt></ruby>のrename・diagnostic・previewへ<ruby>適用<rt>てきよう</rt></ruby>しない。request IDを<ruby>再利用<rt>さいりよう</rt></ruby>するときも、epochで<ruby>隔離<rt>かくり</rt></ruby>する。

<ruby>協調<rt>きょうちょう</rt></ruby>cancelはbudget\/pollで<ruby>処理<rt>しょり</rt></ruby>するが、<ruby>長<rt>なが</rt></ruby>い<ruby>同期<rt>どうき</rt></ruby>Wasm<ruby>実行中<rt>じっこうちゅう</rt></ruby>に<ruby>通常<rt>つうじょう</rt></ruby>のpostMessageが<ruby>直<rt>ただ</rt></ruby>ちに<ruby>処理<rt>しょり</rt></ruby>されるとは<ruby>仮定<rt>かてい</rt></ruby>しない。hostは<ruby>定義済<rt>ていぎず</rt></ruby>みの<ruby>停止期限<rt>ていしきげん</rt></ruby>でWorkerをterminateし、epochを<ruby>進<rt>すす</rt></ruby>めて<ruby>再作成<rt>さいさくせい</rt></ruby>する。<ruby>終了<rt>しゅうりょう</rt></ruby>したWorker<ruby>内<rt>ない</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>・continuationを<ruby>再利用<rt>さいりよう</rt></ruby>せず、<ruby>必要<rt>ひつよう</rt></ruby>なsnapshot\/profileを<ruby>新<rt>あたら</rt></ruby>しいWorkerへ<ruby>送<rt>おく</rt></ruby>り<ruby>直<rt>なお</rt></ruby>す。<ruby>破棄<rt>はき</rt></ruby>と<ruby>再起動<rt>さいきどう</rt></ruby>の<ruby>途中<rt>とちゅう</rt></ruby>も、UIの<ruby>処理状態<rt>しょりじょうたい</rt></ruby>を<ruby>失敗<rt>しっぱい</rt></ruby>・<ruby>停止<rt>ていし</rt></ruby>・<ruby>再準備<rt>さいじゅんび</rt></ruby>として<ruby>表<rt>あらわ</rt></ruby>し、<ruby>成功<rt>せいこう</rt></ruby>へ<ruby>置<rt>お</rt></ruby>き<ruby>換<rt>か</rt></ruby>えない。

Workerだけで<ruby>完全<rt>かんぜん</rt></ruby>なsecurity sandboxが<ruby>成立<rt>せいりつ</rt></ruby>するとは<ruby>扱<rt>あつか</rt></ruby>わず、allowlist・<ruby>入力予算<rt>にゅうりょくよさん</rt></ruby>・resource<ruby>権限<rt>けんげん</rt></ruby>を<ruby>維持<rt>いじ</rt></ruby>する。<ruby>標準利用<rt>ひょうじゅんりよう</rt></ruby>にSharedArrayBufferや<ruby>追加<rt>ついか</rt></ruby>server headerを<ruby>必須<rt>ひっす</rt></ruby>としない。<ruby>終了<rt>しゅうりょう</rt></ruby>により<ruby>部分結果<rt>ぶぶんけっか</rt></ruby>も<ruby>失<rt>うしな</rt></ruby>った<ruby>場合<rt>ばあい</rt></ruby>は、その<ruby>事実<rt>じじつ</rt></ruby>を<ruby>表示<rt>ひょうじ</rt></ruby>する。

<a name="n-656469746f72"></a>

<a name="4-editorのtransaction"></a>

## 4\. Editorのtransaction

editor widgetは、caret、IME composition、undo\/redo、layout cacheという<ruby>局所状態<rt>きょくしょじょうたい</rt></ruby>を<ruby>持<rt>も</rt></ruby>ってよい。<ruby>文書内容<rt>ぶんしょないよう</rt></ruby>の<ruby>正本<rt>せいほん</rt></ruby>は、snapshot<ruby>契約<rt>けいやく</rt></ruby>で<ruby>一<rt>ひと</rt></ruby>つにする。adapterはtransaction ID、base snapshot、<ruby>変更<rt>へんこう</rt></ruby>byte range、<ruby>置換<rt>ちかん</rt></ruby>Text、<ruby>新<rt>しん</rt></ruby>snapshotを<ruby>照合<rt>しょうごう</rt></ruby>し、programmatic editの<ruby>再通知<rt>さいつうち</rt></ruby>を<ruby>同<rt>おな</rt></ruby>じtransactionとして<ruby>処理<rt>しょり</rt></ruby>する。

IME<ruby>中<rt>ちゅう</rt></ruby>にpreview<ruby>更新<rt>こうしん</rt></ruby>でeditorを<ruby>再生成<rt>さいせいせい</rt></ruby>したり、<ruby>無条件<rt>むじょうけん</rt></ruby>の<ruby>全文置換<rt>ぜんぶんちかん</rt></ruby>をしたりしない。composition<ruby>中<rt>ちゅう</rt></ruby>の<ruby>一時入力<rt>いちじにゅうりょく</rt></ruby>と<ruby>確定<rt>かくてい</rt></ruby>transactionを<ruby>区別<rt>くべつ</rt></ruby>し、<ruby>確定<rt>かくてい</rt></ruby>まで<ruby>安全<rt>あんぜん</rt></ruby>に<ruby>保留<rt>ほりゅう</rt></ruby>する<ruby>操作<rt>そうさ</rt></ruby>を<ruby>定義<rt>ていぎ</rt></ruby>する。renameは<ruby>古<rt>ふる</rt></ruby>いsnapshotを<ruby>拒否<rt>きょひ</rt></ruby>し、<ruby>複数編集<rt>ふくすうへんしゅう</rt></ruby>を<ruby>一<rt>ひと</rt></ruby>つのundo transactionとして<ruby>適用<rt>てきよう</rt></ruby>する。<ruby>選択範囲<rt>せんたくはんい</rt></ruby>のUTF\-16と<ruby>内部<rt>ないぶ</rt></ruby>UTF\-8 byte offsetの<ruby>変換<rt>へんかん</rt></ruby>はadapterで<ruby>行<rt>おこな</rt></ruby>い、<ruby>日本語<rt>にほんご</rt></ruby>・<ruby>補助平面文字<rt>ほじょへいめんもじ</rt></ruby>・CRLF・undo\/redo・<ruby>外部読込<rt>がいぶよみこ</rt></ruby>みと<ruby>競合<rt>きょうごう</rt></ruby>する<ruby>編集<rt>へんしゅう</rt></ruby>を<ruby>検証<rt>けんしょう</rt></ruby>する。

<ruby>毎<rt>まい</rt></ruby>eventで<ruby>巨大<rt>きょだい</rt></ruby>sourceやASTを<ruby>複製<rt>ふくせい</rt></ruby>しない。<ruby>不変<rt>ふへん</rt></ruby>snapshot handle、<ruby>差分<rt>さぶん</rt></ruby>、<ruby>共有<rt>きょうゆう</rt></ruby>データを<ruby>使用<rt>しよう</rt></ruby>し、<ruby>公開<rt>こうかい</rt></ruby>wireではpointerを<ruby>運<rt>はこ</rt></ruby>ばない。WidgetとModelに、<ruby>独立<rt>どくりつ</rt></ruby>したsource<ruby>正本<rt>せいほん</rt></ruby>を<ruby>持<rt>も</rt></ruby>たせない。

<a name="n-6f7065726174696f6e73"></a>

<a name="5-言語compositionとreference-profileの操作"></a>

<a name="5-四言語の操作"></a>

## 5\. <ruby>言語<rt>げんご</rt></ruby>compositionとreference profileの<ruby>操作<rt>そうさ</rt></ruby>

Playgroundでは<ruby>最小<rt>さいしょう</rt></ruby>の<ruby>独立<rt>どくりつ</rt></ruby>LanguagePackageを<ruby>作成<rt>さくせい</rt></ruby>し、<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>やUIの<ruby>言語名分岐<rt>げんごめいぶんき</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>せず<ruby>登録<rt>とうろく</rt></ruby>して、そのsourceをparse\/check\/printできることを<ruby>検証<rt>けんしょう</rt></ruby>する。head\/arity、<ruby>先行構文<rt>せんこうこうぶん</rt></ruby>による<ruby>後続<rt>こうぞく</rt></ruby>contextの<ruby>更新<rt>こうしん</rt></ruby>、<ruby>外国語構文<rt>がいこくごこうぶん</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>、Source\/Originと<ruby>診断<rt>しんだん</rt></ruby>を<ruby>観察<rt>かんさつ</rt></ruby>できるようにする。

さらに<ruby>別<rt>べつ</rt></ruby>LanguagePackageをimport\/compositionし、<ruby>単一<rt>たんいつ</rt></ruby>source<ruby>内<rt>ない</rt></ruby>の<ruby>多階層埋<rt>たかいそうう</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みを<ruby>共通操作経路<rt>きょうつうそうさけいろ</rt></ruby>で<ruby>扱<rt>あつか</rt></ruby>う。sentenceやannotationも<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>基礎言語<rt>きそげんご</rt></ruby>として<ruby>利用<rt>りよう</rt></ruby>し、foundationやeditorの<ruby>特別構文<rt>とくべつこうぶん</rt></ruby>へ<ruby>内蔵<rt>ないぞう</rt></ruby>しない。annotationでは<ruby>対象<rt>たいしょう</rt></ruby>syntaxとの<ruby>関係<rt>かんけい</rt></ruby>と、<ruby>対象<rt>たいしょう</rt></ruby>のbinding・domain<ruby>意味<rt>いみ</rt></ruby>の<ruby>保持<rt>ほじ</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>未実装<rt>みじっそう</rt></ruby>package・provider・<ruby>操作<rt>そうさ</rt></ruby>は<ruby>能力不足<rt>のうりょくぶそく</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>し、<ruby>受入済<rt>うけいれず</rt></ruby>みとしない。

<ruby>以下<rt>いか</rt></ruby>の4<ruby>言語<rt>げんご</rt></ruby>の<ruby>既存操作要件<rt>きそんそうさようけん</rt></ruby>はreference profileの<ruby>受入<rt>うけいれ</rt></ruby>として<ruby>維持<rt>いじ</rt></ruby>する。<ruby>追加言語<rt>ついかげんご</rt></ruby>の<ruby>登録<rt>とうろく</rt></ruby>とcompositionの<ruby>受入<rt>うけいれ</rt></ruby>を、これらの<ruby>固定例<rt>こていれい</rt></ruby>だけで<ruby>代替<rt>だいたい</rt></ruby>しない。Playground\/Tutorialの<ruby>共通原理<rt>きょうつうげんり</rt></ruby>と<ruby>各言語固有<rt>かくげんごこゆう</rt></ruby>referenceの<ruby>所有<rt>しょゆう</rt></ruby>は、[Pages<ruby>情報設計<rt>じょうほうせっけい</rt></ruby>](<\.\.\/decisions\/pages\-information\-architecture\.md>)に<ruby>従<rt>したが</rt></ruby>う。

| <ruby>言語<rt>げんご</rt></ruby> | <ruby>必須操作<rt>ひっすそうさ</rt></ruby>と<ruby>表示<rt>ひょうじ</rt></ruby> |
| :--- | :--- |
| Grammar | <ruby>文法<rt>ぶんぽう</rt></ruby>の<ruby>編集<rt>へんしゅう</rt></ruby>・<ruby>検査<rt>けんさ</rt></ruby>・package<ruby>生成<rt>せいせい</rt></ruby>、<ruby>対象<rt>たいしょう</rt></ruby>DSL sourceの<ruby>別<rt>べつ</rt></ruby>editor、<ruby>生成<rt>せいせい</rt></ruby>した<ruby>文法<rt>ぶんぽう</rt></ruby>でparse、highlight、<ruby>定義<rt>ていぎ</rt></ruby>ジャンプ |
| Doc | sentence literal\/prefix、Ruby\/Anno、sentence<ruby>単位<rt>たんい</rt></ruby>parallel、HTML preview、artifact<ruby>取得<rt>しゅとく</rt></ruby> |
| Math | <ruby>元<rt>もと</rt></ruby>の<ruby>式<rt>しき</rt></ruby>、bindings<ruby>入力<rt>にゅうりょく</rt></ruby>、<ruby>明示評価<rt>めいじひょうか</rt></ruby>、Exact\/Symbolic\/Invalid\/Stopped、HTML\/MathML artifact、KaTeX<ruby>優先<rt>ゆうせん</rt></ruby>\/MathMLのみの<ruby>設定<rt>せってい</rt></ruby> |
| Circuit | <ruby>接続<rt>せつぞく</rt></ruby>・<ruby>幅検査<rt>はばけんさ</rt></ruby>、<ruby>入力変更<rt>にゅうりょくへんこう</rt></ruby>、initial\/step\/reset、<ruby>現在<rt>げんざい</rt></ruby>state\/output、test、SVG、NOR IR |
| <ruby>共通<rt>きょうつう</rt></ruby> | <ruby>例選択<rt>れいせんたく</rt></ruby>、source<ruby>入出力<rt>にゅうしゅつりょく</rt></ruby>、<ruby>診断一覧<rt>しんだんいちらん</rt></ruby>と<ruby>位置移動<rt>いちいどう</rt></ruby>、<ruby>処理状態<rt>しょりじょうたい</rt></ruby>\/cancel、<ruby>成果物入出力<rt>せいかぶつにゅうしゅつりょく</rt></ruby> |

<ruby>共通<rt>きょうつう</rt></ruby>editorは、highlight、definition、references、rename、completion、<ruby>対応文<rt>たいおうぶん</rt></ruby>・<ruby>生成元<rt>せいせいもと</rt></ruby>への<ruby>移動<rt>いどう</rt></ruby>も<ruby>提供<rt>ていきょう</rt></ruby>する。<ruby>全文<rt>ぜんぶん</rt></ruby>sourceをdebug logへ<ruby>既定出力<rt>きていしゅつりょく</rt></ruby>しない。ブラウザに<ruby>存在<rt>そんざい</rt></ruby>しないproviderは<ruby>能力不足<rt>のうりょくぶそく</rt></ruby>として<ruby>示<rt>しめ</rt></ruby>し、workspaceからnative pluginや<ruby>任意<rt>にんい</rt></ruby>JSを<ruby>自動生成<rt>じどうせいせい</rt></ruby>・<ruby>実行<rt>じっこう</rt></ruby>しない。

<ruby>追加<rt>ついか</rt></ruby>DSLのeditor<ruby>支援<rt>しえん</rt></ruby>は、Grammarの<ruby>構文<rt>こうぶん</rt></ruby>・<ruby>束縛<rt>そくばく</rt></ruby>・<ruby>表示定義<rt>ひょうじていぎ</rt></ruby>と<ruby>共通<rt>きょうつう</rt></ruby>engineから<ruby>得<rt>え</rt></ruby>る。<ruby>追加<rt>ついか</rt></ruby>ごとのTypeScript keyword<ruby>表<rt>ひょう</rt></ruby>や<ruby>名前解決器<rt>なまえかいけつき</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>しない。<ruby>言語固有<rt>げんごこゆう</rt></ruby>の<ruby>操作<rt>そうさ</rt></ruby>panelは<ruby>許<rt>ゆる</rt></ruby>すが、<ruby>未知<rt>みち</rt></ruby>DSLのsimulatorやrendererを<ruby>自動生成<rt>じどうせいせい</rt></ruby>した<ruby>扱<rt>あつか</rt></ruby>いにはしない。<ruby>機能<rt>きのう</rt></ruby>のない<ruby>操作<rt>そうさ</rt></ruby>は、<ruby>選択肢<rt>せんたくし</rt></ruby>として<ruby>成功<rt>せいこう</rt></ruby>stubを<ruby>表示<rt>ひょうじ</rt></ruby>しない。

<a name="n-70726576696577"></a>

<a name="6-previewとsourceの保護"></a>

## 6\. Previewとsourceの<ruby>保護<rt>ほご</rt></ruby>

source<ruby>表示<rt>ひょうじ</rt></ruby>からguest<ruby>評価<rt>ひょうか</rt></ruby>を<ruby>開始<rt>かいし</rt></ruby>しない。Doc\/Math\/Circuitの<ruby>表示<rt>ひょうじ</rt></ruby>は<ruby>仕様<rt>しよう</rt></ruby>で<ruby>選<rt>えら</rt></ruby>んだ<ruby>操作<rt>そうさ</rt></ruby>だけを<ruby>実行<rt>じっこう</rt></ruby>し、Math<ruby>評価<rt>ひょうか</rt></ruby>・Circuit step・Grammar compileは<ruby>明示<rt>めいじ</rt></ruby>した<ruby>操作経路<rt>そうさけいろ</rt></ruby>に<ruby>置<rt>お</rt></ruby>く。<ruby>例<rt>れい</rt></ruby>の<ruby>読<rt>よ</rt></ruby>み<ruby>込<rt>こ</rt></ruby>みやURLの<ruby>変更<rt>へんこう</rt></ruby>だけで、<ruby>危険<rt>きけん</rt></ruby>なproviderを<ruby>許可<rt>きょか</rt></ruby>しない。

<ruby>自動更新<rt>じどうこうしん</rt></ruby>は、parse\/analyzeと<ruby>明示的<rt>めいじてき</rt></ruby>に<ruby>許可<rt>きょか</rt></ruby>されたbounded previewに<ruby>限<rt>かぎ</rt></ruby>る。<ruby>高負荷評価<rt>こうふかひょうか</rt></ruby>、<ruby>回路<rt>かいろ</rt></ruby>step\/test、<ruby>外部作用<rt>がいぶさよう</rt></ruby>は<ruby>明示<rt>めいじ</rt></ruby>Msgから<ruby>開始<rt>かいし</rt></ruby>し、<ruby>例<rt>れい</rt></ruby>の<ruby>表示<rt>ひょうじ</rt></ruby>を<ruby>自動評価<rt>じどうひょうか</rt></ruby>の<ruby>同意<rt>どうい</rt></ruby>として<ruby>扱<rt>あつか</rt></ruby>わない。

previewは<ruby>検査済<rt>けんさず</rt></ruby>みMarkup artifactだけを、scriptを<ruby>許<rt>ゆる</rt></ruby>さない<ruby>隔離<rt>かくり</rt></ruby>されたiframeへ<ruby>渡<rt>わた</rt></ruby>す。top navigation、form<ruby>送信<rt>そうしん</rt></ruby>、<ruby>任意<rt>にんい</rt></ruby>network、<ruby>同<rt>どう</rt></ruby>origin<ruby>権限<rt>けんげん</rt></ruby>の<ruby>付与<rt>ふよ</rt></ruby>を<ruby>避<rt>さ</rt></ruby>け、hostとの<ruby>連携<rt>れんけい</rt></ruby>は<ruby>検証<rt>けんしょう</rt></ruby>したmessageと<ruby>明示<rt>めいじ</rt></ruby>したIDだけを<ruby>受<rt>う</rt></ruby>ける。<ruby>例外的<rt>れいがいてき</rt></ruby>な<ruby>機能<rt>きのう</rt></ruby>を<ruby>必要<rt>ひつよう</rt></ruby>とするなら、trust<ruby>契約<rt>けいやく</rt></ruby>を<ruby>先<rt>さき</rt></ruby>に<ruby>変更<rt>へんこう</rt></ruby>する。<ruby>外部<rt>がいぶ</rt></ruby>providerの<ruby>許可<rt>きょか</rt></ruby>とpreviewの<ruby>隔離<rt>かくり</rt></ruby>は、<ruby>別<rt>べつ</rt></ruby>に<ruby>検査<rt>けんさ</rt></ruby>する。

sourceへの<ruby>移動<rt>いどう</rt></ruby>はhostの<ruby>診断<rt>しんだん</rt></ruby>・<ruby>構造<rt>こうぞう</rt></ruby>paneから<ruby>提供<rt>ていきょう</rt></ruby>でき、iframe<ruby>内<rt>ない</rt></ruby>のscriptやpostMessage<ruby>発行<rt>はっこう</rt></ruby>を<ruby>必要<rt>ひつよう</rt></ruby>としない。hostが<ruby>受信<rt>じゅしん</rt></ruby>するその<ruby>他<rt>ほか</rt></ruby>の<ruby>連携<rt>れんけい</rt></ruby>messageにも、origin・payload・<ruby>要求<rt>ようきゅう</rt></ruby>identity<ruby>検査<rt>けんさ</rt></ruby>を<ruby>適用<rt>てきよう</rt></ruby>する。

<ruby>数式<rt>すうしき</rt></ruby>は[17<ruby>章<rt>しょう</rt></ruby>](<17\-math\-html\.md>)に<ruby>従<rt>したが</rt></ruby>いWorker<ruby>内<rt>ない</rt></ruby>のKaTeX adapterで<ruby>生成<rt>せいせい</rt></ruby>し、<ruby>同<rt>おな</rt></ruby>じ<ruby>完成<rt>かんせい</rt></ruby>artifactをpreviewと<ruby>書出<rt>かきだ</rt></ruby>しへ<ruby>渡<rt>わた</rt></ruby>す。iframe<ruby>内<rt>ない</rt></ruby>でKaTeXを<ruby>再実行<rt>さいじっこう</rt></ruby>せず、CSS\/fontもその<ruby>文書<rt>ぶんしょ</rt></ruby>に<ruby>適用<rt>てきよう</rt></ruby>する。<ruby>完全要求<rt>かんぜんようきゅう</rt></ruby>identity・<ruby>停止<rt>ていし</rt></ruby>・<ruby>設定<rt>せってい</rt></ruby>・asset<ruby>診断<rt>しんだん</rt></ruby>を、TEAのMsg\/Cmdへ<ruby>統合<rt>とうごう</rt></ruby>する。JavaScript<ruby>無効時<rt>むこうじ</rt></ruby>の<ruby>対話生成不可<rt>たいわせいせいふか</rt></ruby>と、<ruby>書<rt>か</rt></ruby>き<ruby>出<rt>だ</rt></ruby>し<ruby>済<rt>ず</rt></ruby>み<ruby>文書<rt>ぶんしょ</rt></ruby>のJavaScript<ruby>不要<rt>ふよう</rt></ruby>な<ruby>閲覧<rt>えつらん</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>する。

<ruby>編集中<rt>へんしゅうちゅう</rt></ruby>sourceを、<ruby>無断<rt>むだん</rt></ruby>でnetwork<ruby>送信<rt>そうしん</rt></ruby>しない。<ruby>読込<rt>よみこ</rt></ruby>み・<ruby>保存<rt>ほぞん</rt></ruby>・download・<ruby>共有<rt>きょうゆう</rt></ruby>はhostの<ruby>明示操作<rt>めいじそうさ</rt></ruby>とし、<ruby>失敗<rt>しっぱい</rt></ruby>、<ruby>保存容量上限<rt>ほぞんようりょうじょうげん</rt></ruby>、<ruby>権限拒否<rt>けんげんきょひ</rt></ruby>を<ruby>状態<rt>じょうたい</rt></ruby>に<ruby>反映<rt>はんえい</rt></ruby>する。<ruby>未保存編集<rt>みほぞんへんしゅう</rt></ruby>を<ruby>例選択<rt>れいせんたく</rt></ruby>で<ruby>上書<rt>うわが</rt></ruby>きする<ruby>場合<rt>ばあい</rt></ruby>の<ruby>確認<rt>かくにん</rt></ruby>は、<ruby>実装契約<rt>じっそうけいやく</rt></ruby>として<ruby>設<rt>もう</rt></ruby>ける。

<ruby>標準<rt>ひょうじゅん</rt></ruby>assetは<ruby>同<rt>おな</rt></ruby>じ<ruby>公開<rt>こうかい</rt></ruby>artifactから<ruby>取得<rt>しゅとく</rt></ruby>し、runtimeのCDN<ruby>取得<rt>しゅとく</rt></ruby>を<ruby>前提<rt>ぜんてい</rt></ruby>にしない。local storageは、project\/document\/schema<ruby>版<rt>ばん</rt></ruby>でnamespaceを<ruby>分<rt>わ</rt></ruby>ける。<ruby>例共有<rt>れいきょうゆう</rt></ruby>は<ruby>例<rt>れい</rt></ruby>IDを<ruby>基本<rt>きほん</rt></ruby>にし、ユーザーsourceをURL queryへ<ruby>自動追加<rt>じどうついか</rt></ruby>しない。<ruby>明示共有<rt>めいじきょうゆう</rt></ruby>のfragment\/importも、<ruby>容量<rt>ようりょう</rt></ruby>とdecode<ruby>予算<rt>よさん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。

<a name="n-7265666572656e636573"></a>

<a name="7-根拠"></a>

## 7\. <ruby>根拠<rt>こんきょ</rt></ruby>

Model\/View\/Updateの<ruby>分離<rt>ぶんり</rt></ruby>は [Elm<ruby>公式<rt>こうしき</rt></ruby>ガイド](<https\:\/\/guide\.elm\-lang\.org\/architecture\/>) に<ruby>基<rt>もと</rt></ruby>づく。Workerの<ruby>終了<rt>しゅうりょう</rt></ruby>は [HTML Standard](<https\:\/\/html\.spec\.whatwg\.org\/multipage\/workers\.html\#dom\-worker\-terminate>) と [MDN](<https\:\/\/developer\.mozilla\.org\/en\-US\/docs\/Web\/API\/Worker\/terminate>) を<ruby>参照<rt>さんしょう</rt></ruby>する。<ruby>採用<rt>さいよう</rt></ruby>する<ruby>型境界<rt>かたきょうかい</rt></ruby>とcross\-platform<ruby>検証<rt>けんしょう</rt></ruby>は [ユーザーの<ruby>設計指針<rt>せっけいししん</rt></ruby>](<https\:\/\/zenn\.dev\/bem130\/articles\/1b352797de94e7>) に<ruby>合<rt>あ</rt></ruby>わせる。Web<ruby>専用<rt>せんよう</rt></ruby>の<ruby>副作用<rt>ふくさよう</rt></ruby>を<ruby>純粋<rt>じゅんすい</rt></ruby>coreへ<ruby>混<rt>ま</rt></ruby>ぜず、<ruby>未検証<rt>みけんしょう</rt></ruby>の<ruby>対応環境<rt>たいおうかんきょう</rt></ruby>を<ruby>完成扱<rt>かんせいあつか</rt></ruby>いしない。
