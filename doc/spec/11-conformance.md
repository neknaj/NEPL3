<!-- Generated from doc/spec/11&#45;conformance.nepld; renderer nepl3-tools.markdown-annotated-pages/3; page conformance; source SHA-256 3d847e397d5ff3f1b6b6048a7b281ad2ef5c13bd7713f1b6bd1b1081889794d2; alias input SHA-256 adb2a5929aaa53a705f0628d29a115d3891884f79f7ab2786608a4d947edbb42; document digest 0174565b6daeb9fc4b6107ff14e0240d820ada55da70d3575577b94f6861b0e1; page input SHA-256 e053a7af39776cfc7354a74f769ab4745c8b5e09dab89ebcd5f3ea7deae986b7. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="11-受入条件と検証"></a>

# 11\. <ruby>受入条件<rt>うけいれじょうけん</rt></ruby>と<ruby>検証<rt>けんしょう</rt></ruby>

[正本（NEPL3d）](<11-conformance.nepld>)

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## <ruby>方針<rt>ほうしん</rt></ruby>

<ruby>仕様<rt>しよう</rt></ruby>・データ・<ruby>実装<rt>じっそう</rt></ruby>・<ruby>実行結果<rt>じっこうけっか</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>する。schemaの<ruby>形式整合<rt>けいしきせいごう</rt></ruby>だけで<ruby>言語<rt>げんご</rt></ruby>が<ruby>完成<rt>かんせい</rt></ruby>したことにしない。<ruby>正例<rt>せいれい</rt></ruby>、<ruby>誤例<rt>ごれい</rt></ruby>、<ruby>境界<rt>きょうかい</rt></ruby>、<ruby>変換前後<rt>へんかんぜんご</rt></ruby>、<ruby>異<rt>こと</rt></ruby>なる<ruby>実装<rt>じっそう</rt></ruby>の<ruby>比較<rt>ひかく</rt></ruby>を<ruby>受入条件<rt>うけいれじょうけん</rt></ruby>とする。

<a name="n-7265717569726564"></a>

<a name="1-必須の試験群"></a>

## 1\. <ruby>必須<rt>ひっす</rt></ruby>の<ruby>試験群<rt>しけんぐん</rt></ruby>

- X01\: <ruby>外部<rt>がいぶ</rt></ruby>workspace\/repositoryの<ruby>新言語<rt>しんげんご</rt></ruby>がfoundationを<ruby>変更<rt>へんこう</rt></ruby>せず<ruby>公開<rt>こうかい</rt></ruby>APIで<ruby>解析<rt>かいせき</rt></ruby>・schema・source\/Origin・<ruby>診断<rt>しんだん</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>い、Rust\/nativeとNDF\/<ruby>実<rt>じつ</rt></ruby>process providerで<ruby>意味結果<rt>いみけっか</rt></ruby>・<ruby>失敗<rt>しっぱい</rt></ruby>・<ruby>停止<rt>ていし</rt></ruby>が<ruby>一致<rt>いっち</rt></ruby>する。foundation<ruby>単独<rt>たんどく</rt></ruby>の<ruby>配布<rt>はいふ</rt></ruby>\/build\/test\/conformanceとdomainへのproduction\/build<ruby>逆依存不在<rt>ぎゃくいぞんふざい</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>詳細<rt>しょうさい</rt></ruby>は22<ruby>章<rt>しょう</rt></ruby>。
- X02\: Rust source APIの<ruby>互換性<rt>ごかんせい</rt></ruby>とportable schema\/signatureの<ruby>互換性<rt>ごかんせい</rt></ruby>を<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>旧版<rt>きゅうはん</rt></ruby>consumer・<ruby>不正例<rt>ふせいれい</rt></ruby>・<ruby>互換更新<rt>ごかんこうしん</rt></ruby>で<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>単<rt>たん</rt></ruby>なるdigest<ruby>差分<rt>さぶん</rt></ruby>やpath<ruby>依存<rt>いぞん</rt></ruby>の<ruby>成功<rt>せいこう</rt></ruby>を<ruby>独立<rt>どくりつ</rt></ruby>リリース<ruby>検証<rt>けんしょう</rt></ruby>へ<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えない。<ruby>詳細<rt>しょうさい</rt></ruby>は22<ruby>章<rt>しょう</rt></ruby>。

<!-- -->

- G01\: Grammar<ruby>自身<rt>じしん</rt></ruby>をseedで<ruby>読<rt>よ</rt></ruby>み、compileしたpackageとseedの<ruby>意味正規形<rt>いみせいきけい</rt></ruby>が<ruby>一致<rt>いっち</rt></ruby>。
- G02\: declarative readerとRust direct readerの<ruby>結果<rt>けっか</rt></ruby>・<ruby>消費範囲<rt>しょうひはんい</rt></ruby>・viewが<ruby>一致<rt>いっち</rt></ruby>。
- G03\: <ruby>空反復<rt>くうはんぷく</rt></ruby>、<ruby>進捗<rt>しんちょく</rt></ruby>なし<ruby>再帰<rt>さいき</rt></ruby>、<ruby>未定義<rt>みていぎ</rt></ruby>reader、shape<ruby>衝突<rt>しょうとつ</rt></ruby>、provider<ruby>署名違反<rt>しょめいいはん</rt></ruby>を<ruby>正<rt>ただ</rt></ruby>しいcodeで<ruby>拒否<rt>きょひ</rt></ruby>。
- G04\: quoted<ruby>属性内<rt>ぞくせいない</rt></ruby>の `>` を<ruby>含<rt>ふく</rt></ruby>むAngleTag、<ruby>動的<rt>どうてき</rt></ruby>delimiter、<ruby>部分入力<rt>ぶぶんにゅうりょく</rt></ruby>のNeedMore、commit\/no\-matchの<ruby>差<rt>さ</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>。
- G05\: native provider \/ NDF loopback \/ <ruby>別<rt>べつ</rt></ruby>process providerから<ruby>同<rt>おな</rt></ruby>じ<ruby>結果<rt>けっか</rt></ruby>。

<!-- -->

- P01\: prefixのserialize\/parseで<ruby>木<rt>き</rt></ruby>が<ruby>同<rt>おな</rt></ruby>じ。cons\/nilを<ruby>含<rt>ふく</rt></ruby>む<ruby>全<rt>ぜん</rt></ruby>constructorを<ruby>網羅<rt>もうら</rt></ruby>。Numberの<ruby>有限十進制約<rt>ゆうげんじっしんせいやく</rt></ruby>、<ruby>任意有理数<rt>にんいゆうりすう</rt></ruby>からの<ruby>式構築<rt>しきこうちく</rt></ruby>、<ruby>著者<rt>ちょしゃ</rt></ruby>のFrac<ruby>保存<rt>ほぞん</rt></ruby>も<ruby>検査<rt>けんさ</rt></ruby>する。
- P02\: Doc→Math→DocとDoc→Circuitの<ruby>復帰直後<rt>ふっきちょくご</rt></ruby>のhost tokenを<ruby>読<rt>よ</rt></ruby>み<ruby>過<rt>す</rt></ruby>ぎない。
- P03\: headのarityを<ruby>既読<rt>きどく</rt></ruby>の<ruby>子<rt>こ</rt></ruby>の<ruby>値<rt>あたい</rt></ruby>で<ruby>変更<rt>へんこう</rt></ruby>できない。<ruby>局所<rt>きょくしょ</rt></ruby>schema<ruby>更新<rt>こうしん</rt></ruby>providerは<ruby>既読情報<rt>きどくじょうほう</rt></ruby>だけ<ruby>利用<rt>りよう</rt></ruby>。
- P04\: Recover parseがMissing\/Unparsedを<ruby>保持<rt>ほじ</rt></ruby>し、<ruby>未知<rt>みち</rt></ruby>arityを0としない。

<!-- -->

- D01\: sentence literalとprefix<ruby>構築<rt>こうちく</rt></ruby>が<ruby>意味正規形<rt>いみせいきけい</rt></ruby>で<ruby>等<rt>ひと</rt></ruby>しい。
- D02\: ネストしたruby\/anno、<ruby>多段<rt>ただん</rt></ruby>note、<ruby>全<rt>ぜん</rt></ruby>escape、<ruby>空<rt>から</rt></ruby>part、<ruby>閉<rt>と</rt></ruby>じ<ruby>忘<rt>わす</rt></ruby>れを<ruby>検査<rt>けんさ</rt></ruby>。
- D03\: paragraphの<ruby>深<rt>ふか</rt></ruby>さとparallelのsentence<ruby>対応<rt>たいおう</rt></ruby>が<ruby>独立<rt>どくりつ</rt></ruby>。<ruby>重複<rt>ちょうふく</rt></ruby>languageとparagraph variantは<ruby>拒否<rt>きょひ</rt></ruby>。
- D04\: <ruby>前方<rt>ぜんぽう</rt></ruby>label、<ruby>重複<rt>ちょうふく</rt></ruby>label、<ruby>未定義<rt>みていぎ</rt></ruby>ref、renameの<ruby>捕捉検査<rt>ほそくけんさ</rt></ruby>。
- D05\: HTMLにユーザー<ruby>由来<rt>ゆらい</rt></ruby>scriptが<ruby>出<rt>で</rt></ruby>ず、Doc paragraphのネストが<ruby>正<rt>ただ</rt></ruby>しい<ruby>構造<rt>こうぞう</rt></ruby>で<ruby>出力<rt>しゅつりょく</rt></ruby>される。Markup<ruby>文字集合違反<rt>もじしゅうごういはん</rt></ruby>の<ruby>拒否<rt>きょひ</rt></ruby>、`]]>`のescape、CRと<ruby>属性<rt>ぞくせい</rt></ruby>TAB\/LF\/CRの<ruby>保存<rt>ほぞん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。

<!-- -->

- M01\: 1\/2\+1\/3\=5\/6、0\.1\+0\.2\=3\/10、<ruby>大整数<rt>だいせいすう</rt></ruby>を<ruby>丸<rt>まる</rt></ruby>めず<ruby>計算<rt>けいさん</rt></ruby>。
- M02\: matrix<ruby>形状違反<rt>けいじょういはん</rt></ruby>、<ruby>次元不一致<rt>じげんふいっち</rt></ruby>、0<ruby>除算<rt>じょざん</rt></ruby>、<ruby>非整数指数等<rt>ひせいすうしすうとう</rt></ruby>を<ruby>仕様通<rt>しようどお</rt></ruby>り<ruby>分類<rt>ぶんるい</rt></ruby>。
- M03\: let\/sumのscope、<ruby>外側<rt>そとがわ</rt></ruby>と<ruby>内側<rt>うちがわ</rt></ruby>の<ruby>同名<rt>どうめい</rt></ruby>、free symbol、notation\-onlyのSymbolic。
- M04\: すべてのMath constructorをMathMLへ<ruby>描画<rt>びょうが</rt></ruby>。<ruby>弱<rt>よわ</rt></ruby>い<ruby>子<rt>こ</rt></ruby>の<ruby>括弧<rt>かっこ</rt></ruby>、sub\/powの<ruby>結合<rt>けつごう</rt></ruby>、source<ruby>式保存<rt>しきほぞん</rt></ruby>。mspaceの<ruby>単位付<rt>たんいつ</rt></ruby>き<ruby>非負<rt>ひふ</rt></ruby>em<ruby>長<rt>なが</rt></ruby>さと<ruby>無効値<rt>むこうち</rt></ruby>、Markupの<ruby>文字制約<rt>もじせいやく</rt></ruby>とXML escapeを<ruby>検査<rt>けんさ</rt></ruby>する。[17<ruby>章<rt>しょう</rt></ruby>](<17\-math\-html\.md>)の<ruby>純粋<rt>じゅんすい</rt></ruby>TeX<ruby>変換<rt>へんかん</rt></ruby>の<ruby>忠実性<rt>ちゅうじつせい</rt></ruby>・escape、<ruby>生成時<rt>せいせいじ</rt></ruby>KaTeX、<ruby>独立<rt>どくりつ</rt></ruby>MathML fallbackと<ruby>診断<rt>しんだん</rt></ruby>、macro<ruby>独立性<rt>どくりつせい</rt></ruby>、<ruby>出力検査<rt>しゅつりょくけんさ</rt></ruby>、<ruby>資源上限<rt>しげんじょうげん</rt></ruby>とStopped<ruby>保持<rt>ほじ</rt></ruby>も<ruby>要求<rt>ようきゅう</rt></ruby>する。host<ruby>能力<rt>のうりょく</rt></ruby>のないWASIでは<ruby>明示的<rt>めいじてき</rt></ruby>MathML<ruby>経路<rt>けいろ</rt></ruby>を、<ruby>対応<rt>たいおう</rt></ruby>hostでは<ruby>実<rt>じつ</rt></ruby>KaTeX<ruby>経路<rt>けいろ</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>する。

<!-- -->

- C01\: half\-adderを<ruby>全<rt>ぜん</rt></ruby>4<ruby>入力<rt>にゅうりょく</rt></ruby>、adderは<ruby>小幅<rt>こはば</rt></ruby>の<ruby>全入力<rt>ぜんにゅうりょく</rt></ruby>で<ruby>確認<rt>かくにん</rt></ruby>。
- C02\: <ruby>複数<rt>ふくすう</rt></ruby>stateの<ruby>同時更新<rt>どうじこうしん</rt></ruby>、testごとのreset、pre\-edge<ruby>出力<rt>しゅつりょく</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>。initialはPreparedNetlistを<ruby>受<rt>う</rt></ruby>けてslot<ruby>順<rt>じゅん</rt></ruby>の<ruby>初期値<rt>しょきち</rt></ruby>を<ruby>返<rt>かえ</rt></ruby>す。
- C03\: <ruby>未定義<rt>みていぎ</rt></ruby>signal、<ruby>重複<rt>ちょうふく</rt></ruby>driver、<ruby>幅違反<rt>はばいはん</rt></ruby>、<ruby>組合<rt>くみあわ</rt></ruby>せloop、<ruby>再帰<rt>さいき</rt></ruby>instantiationを<ruby>拒否<rt>きょひ</rt></ruby>。
- C04\: child moduleのstateを<ruby>介<rt>かい</rt></ruby>したfeedbackを<ruby>組合<rt>くみあわ</rt></ruby>せloopとして<ruby>誤拒否<rt>ごきょひ</rt></ruby>しない。
- C05\: instanceを2<ruby>個作<rt>こつく</rt></ruby>ればstateが<ruby>独立<rt>どくりつ</rt></ruby>。wire<ruby>参照<rt>さんしょう</rt></ruby>は<ruby>共有<rt>きょうゆう</rt></ruby>。
- C06\: vector evaluatorと<ruby>独立<rt>どくりつ</rt></ruby>したNOR evaluatorの<ruby>複数<rt>ふくすう</rt></ruby>tick<ruby>一致<rt>いっち</rt></ruby>。4<ruby>種<rt>しゅ</rt></ruby>のnodeとnextBits\/outputBitsのsink<ruby>参照<rt>さんしょう</rt></ruby>、<ruby>長<rt>なが</rt></ruby>さ・bit<ruby>順<rt>じゅん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。

<!-- -->

- E01\: <ruby>日本語<rt>にほんご</rt></ruby>\/<ruby>補助平面文字<rt>ほじょへいめんもじ</rt></ruby>\/CRLFにおけるUTF\-8\/16\/32の<ruby>位置変換<rt>いちへんかん</rt></ruby>。
- E02\: SentenceLiteralの<ruby>内部<rt>ないぶ</rt></ruby>を<ruby>正確<rt>せいかく</rt></ruby>にハイライトし、<ruby>外側<rt>そとがわ</rt></ruby>では1tokenを<ruby>維持<rt>いじ</rt></ruby>。
- E03\: definitionの<ruby>全範囲<rt>ぜんはんい</rt></ruby>とname<ruby>範囲<rt>はんい</rt></ruby>、<ruby>対応文関係<rt>たいおうぶんかんけい</rt></ruby>とdefinitionの<ruby>区別<rt>くべつ</rt></ruby>。
- E04\: <ruby>更新後<rt>こうしんご</rt></ruby>に<ruby>古<rt>ふる</rt></ruby>いdiagnostic\/renameを<ruby>適用<rt>てきよう</rt></ruby>しない。
- E05\: <ruby>増分解析<rt>ぞうぶんかいせき</rt></ruby>と<ruby>全解析<rt>ぜんかいせき</rt></ruby>で<ruby>意味<rt>いみ</rt></ruby>・<ruby>診断<rt>しんだん</rt></ruby>・<ruby>参照<rt>さんしょう</rt></ruby>・<ruby>位置<rt>いち</rt></ruby>が<ruby>一致<rt>いっち</rt></ruby>。
- E06\: code<ruby>表示<rt>ひょうじ</rt></ruby>が<ruby>不正<rt>ふせい</rt></ruby>\/<ruby>非停止<rt>ひていし</rt></ruby>のguestをlower・<ruby>意味<rt>いみ</rt></ruby>check・evaluateしない。Doc<ruby>自身<rt>じしん</rt></ruby>のCodeもForeignSyntaxのまま<ruby>表示<rt>ひょうじ</rt></ruby>する。

<!-- -->

- W01\: NDFの<ruby>全<rt>ぜん</rt></ruby>variant roundtrip、<ruby>未知<rt>みち</rt></ruby>tag、<ruby>非<rt>ひ</rt></ruby>canonical<ruby>整数<rt>せいすう</rt></ruby>、<ruby>負<rt>ふ</rt></ruby>zero、<ruby>分母<rt>ぶんぼ</rt></ruby>0、<ruby>壊<rt>こわ</rt></ruby>れた<ruby>参照<rt>さんしょう</rt></ruby>の<ruby>拒否<rt>きょひ</rt></ruby>。field arrayの<ruby>順序交換<rt>じゅんじょこうかん</rt></ruby>でschema digestが<ruby>変<rt>か</rt></ruby>わり、<ruby>名前付<rt>なまえつ</rt></ruby>きvariant mapのkey<ruby>順<rt>じゅん</rt></ruby>だけの<ruby>交換<rt>こうかん</rt></ruby>では<ruby>変<rt>か</rt></ruby>わらないことを<ruby>検査<rt>けんさ</rt></ruby>する。
- W02\: nativeとwire<ruby>経路<rt>けいろ</rt></ruby>で<ruby>一致<rt>いっち</rt></ruby>。Complete\/Invalid\/Stopped\/Awaitを<ruby>網羅<rt>もうら</rt></ruby>。
- W03\: continuationの<ruby>誤用<rt>ごよう</rt></ruby>、<ruby>未知<rt>みち</rt></ruby>operation、schema mismatch、<ruby>過大<rt>かだい</rt></ruby>frameを<ruby>拒否<rt>きょひ</rt></ruby>。

<!-- -->

- A01\: workspace DAG、<ruby>依存許可集合<rt>いぞんきょかしゅうごう</rt></ruby>、coreのno\_stdを<ruby>検査<rt>けんさ</rt></ruby>。
- A02\: native x86\_64\/aarch64、wasm32\-wasip2、wasm32\-unknown\-unknownでbuildし、<ruby>利用可能<rt>りようかのう</rt></ruby>なrunnerで<ruby>同一<rt>どういつ</rt></ruby>goldenを<ruby>実行<rt>じっこう</rt></ruby>。
- A03\: runner<ruby>不在<rt>ふざい</rt></ruby>を「テスト<ruby>成功<rt>せいこう</rt></ruby>」にしない。CIの<ruby>対応<rt>たいおう</rt></ruby>runnerを<ruby>設定<rt>せってい</rt></ruby>して<ruby>完了<rt>かんりょう</rt></ruby>にする。
- A04\: typed errorsのcatalog、<ruby>診断<rt>しんだん</rt></ruby>stage\/<ruby>位置<rt>いち</rt></ruby>、resource<ruby>上限<rt>じょうげん</rt></ruby>とcancelを<ruby>検査<rt>けんさ</rt></ruby>。sourceBytes<ruby>超過<rt>ちょうか</rt></ruby>はUTF\-8 byte<ruby>数<rt>すう</rt></ruby>で<ruby>判定<rt>はんてい</rt></ruby>しStopped\(SourceLimit\)を<ruby>返<rt>かえ</rt></ruby>す。

<!-- -->

- U01\: TEAのinit\/update\/view\/subscriptionsが<ruby>決定的<rt>けっていてき</rt></ruby>で<ruby>作用<rt>さよう</rt></ruby>を<ruby>直接実行<rt>ちょくせつじっこう</rt></ruby>せず、native\/WASI\/browserおよび<ruby>別実装<rt>べつじっそう</rt></ruby>replayでModel\/Cmd\/Viewの<ruby>意味<rt>いみ</rt></ruby>が<ruby>一致<rt>いっち</rt></ruby>する。
- U02\: <ruby>全要求<rt>ぜんようきゅう</rt></ruby>identityの<ruby>照合<rt>しょうごう</rt></ruby>により<ruby>応答逆転<rt>おうとうぎゃくてん</rt></ruby>、close\/reopen、profile\/provider\/options\/resources<ruby>変更後<rt>へんこうご</rt></ruby>の<ruby>旧結果<rt>きゅうけっか</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>する。
- U03\: cancel、<ruby>停止期限<rt>ていしきげん</rt></ruby>、Worker terminate\/recreate、epoch<ruby>更新<rt>こうしん</rt></ruby>と<ruby>再準備<rt>さいじゅんび</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>。<ruby>終了後<rt>しゅうりょうご</rt></ruby>の<ruby>完了応答<rt>かんりょうおうとう</rt></ruby>を<ruby>採用<rt>さいよう</rt></ruby>しない。
- U04\: document<ruby>閉鎖<rt>へいさ</rt></ruby>・<ruby>再読込<rt>さいよみこみ</rt></ruby>、worker\/session epoch、listener\/timer<ruby>再登録<rt>さいとうろく</rt></ruby>と<ruby>解除<rt>かいじょ</rt></ruby>で<ruby>旧処理<rt>きゅうしょり</rt></ruby>や<ruby>二重購読<rt>にじゅうこうどく</rt></ruby>が<ruby>漏<rt>も</rt></ruby>れない。
- U05\: <ruby>日本語<rt>にほんご</rt></ruby>IME、<ruby>補助平面文字<rt>ほじょへいめんもじ</rt></ruby>、CRLF、<ruby>選択<rt>せんたく</rt></ruby>、undo\/redo、format\/rename transaction、programmatic edit feedbackとsnapshot<ruby>競合<rt>きょうごう</rt></ruby>で<ruby>入力<rt>にゅうりょく</rt></ruby>を<ruby>失<rt>うしな</rt></ruby>わない。
- U06\: Grammarと<ruby>対象<rt>たいしょう</rt></ruby>DSLの2editorを<ruby>提供<rt>ていきょう</rt></ruby>し、<ruby>追加<rt>ついか</rt></ruby>の<ruby>文法<rt>ぶんぽう</rt></ruby>・<ruby>束縛<rt>そくばく</rt></ruby>・<ruby>表示定義<rt>ひょうじていぎ</rt></ruby>だけで<ruby>共通<rt>きょうつう</rt></ruby>highlight\/definition<ruby>等<rt>とう</rt></ruby>を<ruby>得<rt>え</rt></ruby>る。TypeScript<ruby>再実装<rt>さいじっそう</rt></ruby>を<ruby>不要<rt>ふよう</rt></ruby>にし、<ruby>外部<rt>がいぶ</rt></ruby>provider<ruby>不足<rt>ふそく</rt></ruby>は<ruby>明示<rt>めいじ</rt></ruby>する。
- U07\: preview<ruby>隔離<rt>かくり</rt></ruby>、<ruby>未承認<rt>みしょうにん</rt></ruby>provider<ruby>拒否<rt>きょひ</rt></ruby>、source<ruby>表示<rt>ひょうじ</rt></ruby>からの<ruby>非評価<rt>ひひょうか</rt></ruby>、<ruby>保存<rt>ほぞん</rt></ruby>・<ruby>読込<rt>よみこみ</rt></ruby>・downloadの<ruby>成功<rt>せいこう</rt></ruby>\/<ruby>拒否<rt>きょひ</rt></ruby>\/<ruby>容量超過<rt>ようりょうちょうか</rt></ruby>、<ruby>未保存編集<rt>みほぞんへんしゅう</rt></ruby>の<ruby>保護<rt>ほご</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。
- U08\: UI\/Worker<ruby>型<rt>がた</rt></ruby>のschema closure、native\/Wasm\/codecと<ruby>別実装<rt>べつじっそう</rt></ruby>replayの<ruby>対応<rt>たいおう</rt></ruby>、core\/domain\/engineへのUI<ruby>依存逆流<rt>いぞんぎゃくりゅう</rt></ruby>がないことを<ruby>検査<rt>けんさ</rt></ruby>する。

<!-- -->

- S01\: SiteConfigの \/NEPL3\/ と<ruby>別<rt>べつ</rt></ruby>の<ruby>非<rt>ひ</rt></ruby>root baseでトップ\/docs\/Playground\/assets\/Worker\/Wasm\/rustdoc\/<ruby>例<rt>れい</rt></ruby>manifestを<ruby>生成<rt>せいせい</rt></ruby>・ロードし、deep linkと<ruby>再読込<rt>さいよみこみ</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。
- S02\: docs・<ruby>例<rt>れい</rt></ruby>manifest・runtime・profile・assetsの<ruby>版<rt>ばん</rt></ruby>を<ruby>照合<rt>しょうごう</rt></ruby>する。cache<ruby>混在<rt>こんざい</rt></ruby>、<ruby>欠<rt>か</rt></ruby>けた<ruby>例<rt>れい</rt></ruby>、digest<ruby>不一致<rt>ふいっち</rt></ruby>、<ruby>版違<rt>ばんちが</rt></ruby>いを<ruby>検出<rt>けんしゅつ</rt></ruby>し、<ruby>同一入力<rt>どういつにゅうりょく</rt></ruby>buildの<ruby>決定性<rt>けっていせい</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。
- S03\: <ruby>配信予定<rt>はいしんよてい</rt></ruby>artifactを<ruby>実<rt>じつ</rt></ruby>browserで<ruby>開<rt>ひら</rt></ruby>き、4<ruby>言語<rt>げんご</rt></ruby>の<ruby>代表操作<rt>だいひょうそうさ</rt></ruby>、<ruby>入出力<rt>にゅうしゅつりょく</rt></ruby>、<ruby>診断選択<rt>しんだんせんたく</rt></ruby>、editor query、<ruby>停止<rt>ていし</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>する。<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>やUIの<ruby>言語名分岐<rt>げんごめいぶんき</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>せず<ruby>最小<rt>さいしょう</rt></ruby>の<ruby>独立<rt>どくりつ</rt></ruby>LanguagePackageを<ruby>登録<rt>とうろく</rt></ruby>し、parse\/check\/printと<ruby>別<rt>べつ</rt></ruby>packageのimport\/composition、Source\/Origin・<ruby>診断<rt>しんだん</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。sentenceとannotationを<ruby>別言語<rt>べつげんご</rt></ruby>として<ruby>組<rt>く</rt></ruby>み<ruby>込<rt>こ</rt></ruby>み、<ruby>注釈<rt>ちゅうしゃく</rt></ruby>の<ruby>対象関係<rt>たいしょうかんけい</rt></ruby>とbinding・domain<ruby>意味<rt>いみ</rt></ruby>の<ruby>保持<rt>ほじ</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>大<rt>おお</rt></ruby>きなsourceの<ruby>応答性<rt>おうとうせい</rt></ruby>とkeyboard\/focusも<ruby>確認<rt>かくにん</rt></ruby>する。
- S04\: docs<ruby>本文<rt>ほんぶん</rt></ruby>をJSなしで<ruby>読<rt>よ</rt></ruby>み、4<ruby>言語<rt>げんご</rt></ruby>tutorial\/reference\/<ruby>例<rt>れい</rt></ruby>から<ruby>同<rt>おな</rt></ruby>じsourceをPlaygroundで<ruby>開<rt>ひら</rt></ruby>く。page\/anchor\/<ruby>検索<rt>けんさく</rt></ruby>\/linkとaccessibilityを<ruby>検査<rt>けんさ</rt></ruby>し、<ruby>自動検査<rt>じどうけんさ</rt></ruby>と<ruby>手動確認<rt>しゅどうかくにん</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>する。
- S05\: <ruby>外部<rt>がいぶ</rt></ruby>backend・localhost・CDNなしで<ruby>基本<rt>きほん</rt></ruby>4<ruby>言語操作<rt>げんごそうさ</rt></ruby>を<ruby>行<rt>おこな</rt></ruby>い、sourceの<ruby>外部送信<rt>がいぶそうしん</rt></ruby>をせず、sandbox previewがscriptを<ruby>実行<rt>じっこう</rt></ruby>しないことを<ruby>検査<rt>けんさ</rt></ruby>する。
- S06\: <ruby>最小権限<rt>さいしょうけんげん</rt></ruby>、<ruby>同<rt>おな</rt></ruby>じ<ruby>検査済<rt>けんさず</rt></ruby>みSHA\/artifact、<ruby>配信直列化<rt>はいしんちょくれつか</rt></ruby>とfreshness、<ruby>失敗<rt>しっぱい</rt></ruby>log<ruby>保存<rt>ほぞん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>。<ruby>実<rt>じつ</rt></ruby>Pages<ruby>公開後<rt>こうかいご</rt></ruby>のHTTPS smokeとasset\/build identityを<ruby>照合<rt>しょうごう</rt></ruby>して<ruby>記録<rt>きろく</rt></ruby>する。<ruby>公開<rt>こうかい</rt></ruby>smoke<ruby>失敗時<rt>しっぱいじ</rt></ruby>は<ruby>検証済<rt>けんしょうず</rt></ruby>みLKGへの<ruby>有限復旧<rt>ゆうげんふっきゅう</rt></ruby>・<ruby>再<rt>さい</rt></ruby>smoke・<ruby>元<rt>もと</rt></ruby>run<ruby>失敗保持<rt>しっぱいほじ</rt></ruby>を15<ruby>章<rt>しょう</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>で<ruby>検査<rt>けんさ</rt></ruby>する。

S06の<ruby>失敗系<rt>しっぱいけい</rt></ruby>には、<ruby>次<rt>つぎ</rt></ruby>の<ruby>場合<rt>ばあい</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>める。

- \(a\)<ruby>最新<rt>さいしん</rt></ruby>candidateのpublic smokeだけが<ruby>失敗<rt>しっぱい</rt></ruby>し<ruby>元<rt>もと</rt></ruby>tarを<ruby>復旧<rt>ふっきゅう</rt></ruby>できる。
- \(b\)<ruby>後続<rt>こうぞく</rt></ruby>の<ruby>健康<rt>けんこう</rt></ruby>なdeploymentがあるため<ruby>旧<rt>きゅう</rt></ruby>candidateの<ruby>復旧<rt>ふっきゅう</rt></ruby>を<ruby>拒否<rt>きょひ</rt></ruby>する。
- \(c\)cache\/API\/journal<ruby>不一致<rt>ふいっち</rt></ruby>・timeout・<ruby>外部<rt>がいぶ</rt></ruby>writer<ruby>疑<rt>うたが</rt></ruby>いで<ruby>書込<rt>かきこ</rt></ruby>みを<ruby>停止<rt>ていし</rt></ruby>する。
- \(d\)<ruby>初回公開<rt>しょかいこうかい</rt></ruby>でLKGがない。
- \(e\)Actions artifact<ruby>失効後<rt>しっこうご</rt></ruby>も<ruby>永続<rt>えいぞく</rt></ruby>snapshotから<ruby>復旧<rt>ふっきゅう</rt></ruby>する。
- \(f\)<ruby>復旧<rt>ふっきゅう</rt></ruby>payload<ruby>消失<rt>しょうしつ</rt></ruby>・<ruby>改変<rt>かいへん</rt></ruby>。
- \(g\)<ruby>復旧<rt>ふっきゅう</rt></ruby>deploy・<ruby>再<rt>さい</rt></ruby>smokeの<ruby>失敗<rt>しっぱい</rt></ruby>。
- \(h\)smoke<ruby>合格後<rt>ごうかくご</rt></ruby>の<ruby>保存<rt>ほぞん</rt></ruby>\/journal<ruby>昇格失敗<rt>しょうかくしっぱい</rt></ruby>。
- \(i\)deploy<ruby>後<rt>ご</rt></ruby>にrunが<ruby>強制<rt>きょうせい</rt></ruby>cancelされ<ruby>次<rt>つぎ</rt></ruby>のwriterがreconcileを<ruby>要求<rt>ようきゅう</rt></ruby>する。

<ruby>自動復旧回数<rt>じどうふっきゅうかいすう</rt></ruby>・<ruby>時間上限<rt>じかんじょうげん</rt></ruby>、lock<ruby>保持<rt>ほじ</rt></ruby>、<ruby>失敗<rt>しっぱい</rt></ruby>run\/incident<ruby>記録<rt>きろく</rt></ruby>、<ruby>現行<rt>げんこう</rt></ruby>LKGと<ruby>前世代<rt>ぜんせだい</rt></ruby>の<ruby>保持<rt>ほじ</rt></ruby>も<ruby>検証<rt>けんしょう</rt></ruby>する。<ruby>模擬失敗系<rt>もぎしっぱいけい</rt></ruby>だけで<ruby>実<rt>じつ</rt></ruby>Pages<ruby>公開<rt>こうかい</rt></ruby>\/<ruby>復旧<rt>ふっきゅう</rt></ruby>のrunner<ruby>要件<rt>ようけん</rt></ruby>を<ruby>満<rt>み</rt></ruby>たした<ruby>扱<rt>あつか</rt></ruby>いにしない。

- J01\: <ruby>全対象文書<rt>ぜんたいしょうぶんしょ</rt></ruby>のinventoryとDoc<ruby>表現<rt>ひょうげん</rt></ruby>gapを<ruby>独立<rt>どくりつ</rt></ruby>レビューし、<ruby>表<rt>ひょう</rt></ruby>\/list\/link\/<ruby>汎用<rt>はんよう</rt></ruby>code\/<ruby>図<rt>ず</rt></ruby>の<ruby>必要<rt>ひつよう</rt></ruby>なschema・<ruby>文法<rt>ぶんぽう</rt></ruby>・backend・wire・conformanceを<ruby>完成<rt>かんせい</rt></ruby>させる。
- J02\: <ruby>全移行<rt>ぜんいこう</rt></ruby>ページを<ruby>元<rt>もと</rt></ruby>の<ruby>固定<rt>こてい</rt></ruby>snapshotと<ruby>比較<rt>ひかく</rt></ruby>し、<ruby>意味<rt>いみ</rt></ruby>・<ruby>表<rt>ひょう</rt></ruby>・<ruby>参照<rt>さんしょう</rt></ruby>・<ruby>数式<rt>すうしき</rt></ruby>・<ruby>図<rt>ず</rt></ruby>・コードbyte<ruby>列<rt>れつ</rt></ruby>の<ruby>同等性<rt>どうとうせい</rt></ruby>を<ruby>独立<rt>どくりつ</rt></ruby>に<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>一<rt>ひと</rt></ruby>つの<ruby>正本<rt>せいほん</rt></ruby>と<ruby>生成<rt>せいせい</rt></ruby>Markdownの<ruby>差分検査<rt>さぶんけんさ</rt></ruby>を<ruby>行<rt>おこな</rt></ruby>う。
- J03\: <ruby>移行<rt>いこう</rt></ruby>ページの<ruby>安定<rt>あんてい</rt></ruby>page ID\/URL\/anchorと<ruby>文書<rt>ぶんしょ</rt></ruby>namespace・<ruby>参照<rt>さんしょう</rt></ruby>の<ruby>対応<rt>たいおう</rt></ruby>をDoc<ruby>意味構造<rt>いみこうぞう</rt></ruby>で<ruby>確認<rt>かくにん</rt></ruby>する。Doc<ruby>正本<rt>せいほん</rt></ruby>からのHTMLでは、escaping、<ruby>必要<rt>ひつよう</rt></ruby>なCSS\/font、<ruby>主要<rt>しゅよう</rt></ruby>リンク、<ruby>例<rt>れい</rt></ruby>の<ruby>内容<rt>ないよう</rt></ruby>・revision、<ruby>見出<rt>みだ</rt></ruby>しや<ruby>表<rt>ひょう</rt></ruby>の<ruby>基本構造<rt>きほんこうぞう</rt></ruby>と<ruby>可読性<rt>かどくせい</rt></ruby>を、<ruby>非<rt>ひ</rt></ruby>root<ruby>配信<rt>はいしん</rt></ruby>と<ruby>主要<rt>しゅよう</rt></ruby>browserで<ruby>確認<rt>かくにん</rt></ruby>する。rustdoc・<ruby>全<rt>ぜん</rt></ruby>サイトの<ruby>検索<rt>けんさく</rt></ruby>・<ruby>全<rt>ぜん</rt></ruby>fragmentの<ruby>完全閉包<rt>かんぜんへいほう</rt></ruby>は、この<ruby>移行受入<rt>いこううけいれ</rt></ruby>の<ruby>前提<rt>ぜんてい</rt></ruby>にしない。
- J04\: <ruby>旧版<rt>きゅうはん</rt></ruby>の<ruby>検証済<rt>けんしょうず</rt></ruby>みrendererによる<ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>文書<rt>ぶんしょ</rt></ruby>buildと<ruby>現行<rt>げんこう</rt></ruby>runtime<ruby>受入<rt>うけいれ</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>し、bootstrap<ruby>循環<rt>じゅんかん</rt></ruby>がないこと、<ruby>全<rt>ぜん</rt></ruby>ページのDoc<ruby>正本<rt>せいほん</rt></ruby>への<ruby>切替<rt>きりかえ</rt></ruby>、<ruby>決定的生成<rt>けっていてきせいせい</rt></ruby>、<ruby>欠落<rt>けつらく</rt></ruby>・<ruby>未対応時<rt>みたいおうじ</rt></ruby>の<ruby>停止<rt>ていし</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。

<ruby>必須群<rt>ひっすぐん</rt></ruby>と<ruby>必須<rt>ひっす</rt></ruby>targetの<ruby>正本<rt>せいほん</rt></ruby>は `design/acceptance.json`。<ruby>本文<rt>ほんぶん</rt></ruby>のID、catalog、implementation\-statusの<ruby>群集合<rt>ぐんしゅうごう</rt></ruby>を<ruby>一致<rt>いっち</rt></ruby>させ、T16は<ruby>全<rt>ぜん</rt></ruby>required<ruby>群<rt>ぐん</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>する。r3は55<ruby>群<rt>ぐん</rt></ruby>だが、この<ruby>数<rt>かず</rt></ruby>をcheckerの<ruby>完了条件<rt>かんりょうじょうけん</rt></ruby>へ<ruby>固定<rt>こてい</rt></ruby>しない。nativeはLinux x86\_64、Windows x86\_64、macOS aarch64、WASIはwasm32\-wasip2をWasmtimeで<ruby>実行<rt>じっこう</rt></ruby>、browserは<ruby>実<rt>じつ</rt></ruby>Chromium\/Firefox\/WebKitを<ruby>対象<rt>たいしょう</rt></ruby>とし、<ruby>正確<rt>せいかく</rt></ruby>な<ruby>版<rt>ばん</rt></ruby>をlogへ<ruby>記録<rt>きろく</rt></ruby>する。native process<ruby>固有群<rt>こゆうぐん</rt></ruby>はnative targetで<ruby>検査<rt>けんさ</rt></ruby>する。WebKitの<ruby>成功<rt>せいこう</rt></ruby>から<ruby>実<rt>じつ</rt></ruby>Safari device QAを<ruby>推定<rt>すいてい</rt></ruby>しない。

<a name="n-70726f70657274696573"></a>

## 2\. propertyとfuzz

<ruby>任意<rt>にんい</rt></ruby>のvalidな<ruby>有限<rt>ゆうげん</rt></ruby>prefix treeからprint→parse→lowerの<ruby>意味一致<rt>いみいっち</rt></ruby>。<ruby>任意<rt>にんい</rt></ruby>のUTF\-8<ruby>入力<rt>にゅうりょく</rt></ruby>でpanicしない。case<ruby>分割<rt>ぶんかつ</rt></ruby>した<ruby>入力<rt>にゅうりょく</rt></ruby>streamと<ruby>一括入力<rt>いっかつにゅうりょく</rt></ruby>が<ruby>同<rt>おな</rt></ruby>じ<ruby>最終<rt>さいしゅう</rt></ruby>token<ruby>列<rt>れつ</rt></ruby>。<ruby>消費範囲<rt>しょうひはんい</rt></ruby>の<ruby>単調性<rt>たんちょうせい</rt></ruby>。Origin DAGの<ruby>閉路<rt>へいろ</rt></ruby>なし。<ruby>可逆<rt>かぎゃく</rt></ruby>なSourceMapだけがrenameを<ruby>許<rt>ゆる</rt></ruby>す。<ruby>小幅回路<rt>こはばかいろ</rt></ruby>の<ruby>原式<rt>げんしき</rt></ruby>とNORの<ruby>一致<rt>いっち</rt></ruby>。

fuzz<ruby>入力<rt>にゅうりょく</rt></ruby>でも<ruby>上限<rt>じょうげん</rt></ruby>を<ruby>共有<rt>きょうゆう</rt></ruby>する。<ruby>大量<rt>たいりょう</rt></ruby>diagnosticを<ruby>発生<rt>はっせい</rt></ruby>させる<ruby>入力<rt>にゅうりょく</rt></ruby>で<ruby>無制限<rt>むせいげん</rt></ruby>allocしない。OOMを<ruby>完全<rt>かんぜん</rt></ruby>に<ruby>回避<rt>かいひ</rt></ruby>できると<ruby>虚偽<rt>きょぎ</rt></ruby>の<ruby>保証<rt>ほしょう</rt></ruby>をしない。

<a name="n-6369"></a>

<a name="3-不変条件のci"></a>

## 3\. <ruby>不変条件<rt>ふへんじょうけん</rt></ruby>のCI

`cargo fmt --check`、clippyの<ruby>対象<rt>たいしょう</rt></ruby>warningのdeny、workspace tests、doc tests、package<ruby>生成<rt>せいせい</rt></ruby>の<ruby>差分検査<rt>さぶんけんさ</rt></ruby>、dependency<ruby>検査<rt>けんさ</rt></ruby>、target<ruby>別<rt>べつ</rt></ruby>check、conformance runner、fuzz smoke、license\/asset\/unsafe<ruby>監査<rt>かんさ</rt></ruby>。

<ruby>一<rt>ひと</rt></ruby>つのarchitecture boundaryごとにtestがあり、<ruby>再設計時<rt>さいせっけいじ</rt></ruby>に<ruby>未更新<rt>みこうしん</rt></ruby>の<ruby>依存<rt>いぞん</rt></ruby>を<ruby>検出<rt>けんしゅつ</rt></ruby>できる。<ruby>新<rt>あたら</rt></ruby>しいconstructorを<ruby>追加<rt>ついか</rt></ruby>した<ruby>際<rt>さい</rt></ruby>はparser、lower、check、print、render、wire、editorのcoverage<ruby>表<rt>ひょう</rt></ruby>に<ruby>行<rt>ぎょう</rt></ruby>が<ruby>増<rt>ふ</rt></ruby>える。None\/unsupportedで<ruby>黙<rt>だま</rt></ruby>って<ruby>網羅扱<rt>もうらあつか</rt></ruby>いにしない。

<a name="n-737461676573"></a>

<a name="4-開発の段階"></a>

## 4\. <ruby>開発<rt>かいはつ</rt></ruby>の<ruby>段階<rt>だんかい</rt></ruby>

<ruby>順序<rt>じゅんじょ</rt></ruby>はtasksで<ruby>管理<rt>かんり</rt></ruby>する。<ruby>前段<rt>ぜんだん</rt></ruby>を<ruby>先<rt>さき</rt></ruby>に<ruby>完成<rt>かんせい</rt></ruby>させることは<ruby>許可<rt>きょか</rt></ruby>するが、<ruby>未完<rt>みかん</rt></ruby>の<ruby>後段<rt>こうだん</rt></ruby>をその<ruby>時点<rt>じてん</rt></ruby>の「<ruby>完成仕様<rt>かんせいしよう</rt></ruby>」と<ruby>呼<rt>よ</rt></ruby>ばない。<ruby>最終受入<rt>さいしゅううけいれ</rt></ruby>は<ruby>全必須試験<rt>ぜんひっすしけん</rt></ruby>に<ruby>対<rt>たい</rt></ruby>する<ruby>実行証拠<rt>じっこうしょうこ</rt></ruby>が<ruby>揃<rt>そろ</rt></ruby>った<ruby>時点<rt>じてん</rt></ruby>。

`design/tasks.json` の<ruby>各<rt>かく</rt></ruby>task\.acceptanceは、そのタスクが<ruby>寄与<rt>きよ</rt></ruby>する<ruby>試験群<rt>しけんぐん</rt></ruby>のcoverage<ruby>参照<rt>さんしょう</rt></ruby>である。<ruby>試験群<rt>しけんぐん</rt></ruby>には<ruby>後続<rt>こうぞく</rt></ruby>タスクの<ruby>責務<rt>せきむ</rt></ruby>も<ruby>含<rt>ふく</rt></ruby>むため、<ruby>参照<rt>さんしょう</rt></ruby>した<ruby>群全体<rt>ぐんぜんたい</rt></ruby>のpassedを<ruby>前段<rt>ぜんだん</rt></ruby>タスクの<ruby>完了条件<rt>かんりょうじょうけん</rt></ruby>にしない。T16<ruby>以外<rt>いがい</rt></ruby>のタスクのcompleteには、<ruby>当該<rt>とうがい</rt></ruby>deliverableの<ruby>実装<rt>じっそう</rt></ruby>、scopeを<ruby>限定<rt>げんてい</rt></ruby>した<ruby>検証証拠<rt>けんしょうしょうこ</rt></ruby>、<ruby>依存<rt>いぞん</rt></ruby>タスクのcomplete、および<ruby>関連<rt>かんれん</rt></ruby>する<ruby>未解消<rt>みかいしょう</rt></ruby>の<ruby>設計<rt>せっけい</rt></ruby>blockerがないことを<ruby>要求<rt>ようきゅう</rt></ruby>する。<ruby>証拠<rt>しょうこ</rt></ruby>は `conformance/results/` <ruby>以下<rt>いか</rt></ruby>のJSONとし、`task_id`、`checks`（<ruby>空<rt>から</rt></ruby>でない<ruby>文字列<rt>もじれつ</rt></ruby>の<ruby>非空<rt>ひくう</rt></ruby>list）、`commands`（<ruby>同<rt>どう</rt></ruby>）、`targets`（<ruby>同<rt>どう</rt></ruby>）、`result`（passed）、`excluded_acceptance_portions`（<ruby>未検証範囲<rt>みけんしょうはんい</rt></ruby>の<ruby>文字列<rt>もじれつ</rt></ruby>list、<ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>空<rt>から</rt></ruby>listを<ruby>許可<rt>きょか</rt></ruby>）を<ruby>持<rt>も</rt></ruby>つ。implementation\-statusの<ruby>当該<rt>とうがい</rt></ruby>タスクからファイルを<ruby>参照<rt>さんしょう</rt></ruby>する。<ruby>形<rt>かたち</rt></ruby>だけの<ruby>証拠<rt>しょうこ</rt></ruby>ファイルではなく、<ruby>記載<rt>きさい</rt></ruby>したコマンドの<ruby>実行結果<rt>じっこうけっか</rt></ruby>をレビューする。

<ruby>例<rt>たと</rt></ruby>えばT01のsource<ruby>契約試験<rt>けいやくしけん</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>してT01をcompleteにしても、E03\/E04のエディタ<ruby>操作<rt>そうさ</rt></ruby>を<ruby>実装<rt>じっそう</rt></ruby>・<ruby>検査<rt>けんさ</rt></ruby>していなければ<ruby>当該群<rt>とうがいぐん</rt></ruby>はnot\-runのままとする。<ruby>群全体<rt>ぐんぜんたい</rt></ruby>のpassedは<ruby>全要件<rt>ぜんようけん</rt></ruby>の<ruby>実行証拠<rt>じっこうしょうこ</rt></ruby>がそろってから<ruby>記録<rt>きろく</rt></ruby>する。T16のcompleteには<ruby>依存<rt>いぞん</rt></ruby>タスクの<ruby>完了<rt>かんりょう</rt></ruby>に<ruby>加<rt>くわ</rt></ruby>えて、task\.acceptanceに<ruby>記載<rt>きさい</rt></ruby>した<ruby>一部<rt>いちぶ</rt></ruby>の<ruby>群<rt>ぐん</rt></ruby>だけでなく<ruby>登録<rt>とうろく</rt></ruby>された<ruby>全必須群<rt>ぜんひっすぐん</rt></ruby>のpassedとその<ruby>証拠<rt>しょうこ</rt></ruby>を<ruby>必須<rt>ひっす</rt></ruby>とする。

<a name="n-65766964656e6365"></a>

<a name="5-群全体の型付き証拠"></a>

## 5\. <ruby>群全体<rt>ぐんぜんたい</rt></ruby>の<ruby>型付<rt>かたつ</rt></ruby>き<ruby>証拠<rt>しょうこ</rt></ruby>

scope<ruby>付<rt>つ</rt></ruby>きTaskEvidenceと<ruby>群全体<rt>ぐんぜんたい</rt></ruby>のAcceptanceEvidenceを<ruby>分<rt>わ</rt></ruby>ける。<ruby>群<rt>ぐん</rt></ruby>の<ruby>証拠<rt>しょうこ</rt></ruby>schemaは `interfaces/acceptance-evidence.schema.json`、<ruby>配置<rt>はいち</rt></ruby>は `conformance/results/`。`schema`、`acceptance_id`、`design_revision`、`identity`、`result`、`runs` を<ruby>必須<rt>ひっす</rt></ruby>とする。identityはprofile `nepl3.repository-inputs/1` とsource\_sha256\/spec\_sha256を<ruby>持<rt>も</rt></ruby>ち、<ruby>現在<rt>げんざい</rt></ruby>の<ruby>検査入力<rt>けんさにゅうりょく</rt></ruby>へ<ruby>照合<rt>しょうごう</rt></ruby>する。source identityの<ruby>収集<rt>しゅうしゅう</rt></ruby>・<ruby>除外規則<rt>じょがいきそく</rt></ruby>は<ruby>開発<rt>かいはつ</rt></ruby>toolsのidentity<ruby>操作<rt>そうさ</rt></ruby>と<ruby>開発手順<rt>かいはつてじゅん</rt></ruby>で<ruby>固定<rt>こてい</rt></ruby>する。

runはcatalogのtarget\.kindに<ruby>一致<rt>いっち</rt></ruby>するtag<ruby>付<rt>つ</rt></ruby>き<ruby>型<rt>がた</rt></ruby>とする。`kind: command` はcommand、target、result、exit\_code、<ruby>非空<rt>ひくう</rt></ruby>checks、environment（runnerのname\/versionとtoolsのname\/version<ruby>一覧<rt>いちらん</rt></ruby>）、log、log\_sha256を<ruby>持<rt>も</rt></ruby>つ。`kind: review` はtarget、reviewer<ruby>識別子<rt>しきべつし</rt></ruby>、independent\=true、decision（approved\/rejected）、<ruby>非空<rt>ひくう</rt></ruby>scope、log、log\_sha256を<ruby>持<rt>も</rt></ruby>つ。<ruby>意味同等性<rt>いみどうとうせい</rt></ruby>レビューのために<ruby>架空<rt>かくう</rt></ruby>のshell commandや<ruby>終了<rt>しゅうりょう</rt></ruby>コードを<ruby>作<rt>つく</rt></ruby>らない。

logは `conformance/results/` <ruby>内<rt>ない</rt></ruby>の<ruby>非空<rt>ひくう</rt></ruby>\.txtまたは\.logファイルとし、<ruby>証拠<rt>しょうこ</rt></ruby>JSON<ruby>自身<rt>じしん</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>logにしない。<ruby>実在<rt>じつざい</rt></ruby>logとdigestを<ruby>検査<rt>けんさ</rt></ruby>し、<ruby>別<rt>べつ</rt></ruby>ID、<ruby>古<rt>ふる</rt></ruby>いsource\/spec\/design、<ruby>未登録<rt>みとうろく</rt></ruby>target、target<ruby>種別不一致<rt>しゅべつふいっち</rt></ruby>、<ruby>改変<rt>かいへん</rt></ruby>logを<ruby>拒否<rt>きょひ</rt></ruby>する。passedには<ruby>全<rt>ぜん</rt></ruby>command runがpassedかつexit\_code\=0、<ruby>全<rt>ぜん</rt></ruby>reviewがapproved、<ruby>全<rt>ぜん</rt></ruby>required targetの<ruby>実行<rt>じっこう</rt></ruby>・レビューを<ruby>要求<rt>ようきゅう</rt></ruby>する。failedは<ruby>少<rt>すく</rt></ruby>なくとも<ruby>一<rt>ひと</rt></ruby>つの<ruby>非<rt>ひ</rt></ruby>zero<ruby>終了<rt>しゅうりょう</rt></ruby>の<ruby>失敗<rt>しっぱい</rt></ruby>runまたはrejected reviewを<ruby>含<rt>ふく</rt></ruby>み、<ruby>早期停止<rt>そうきていし</rt></ruby>による<ruby>未実行<rt>みじっこう</rt></ruby>targetを<ruby>許<rt>ゆる</rt></ruby>すがpassedへ<ruby>変更<rt>へんこう</rt></ruby>できない。<ruby>未実行<rt>みじっこう</rt></ruby>を<ruby>空<rt>から</rt></ruby>logやbuild<ruby>成功<rt>せいこう</rt></ruby>で<ruby>置<rt>お</rt></ruby>き<ruby>換<rt>か</rt></ruby>えない。

<ruby>証拠<rt>しょうこ</rt></ruby>の<ruby>形式<rt>けいしき</rt></ruby>・hash<ruby>検査<rt>けんさ</rt></ruby>は<ruby>記述<rt>きじゅつ</rt></ruby>された<ruby>挙動<rt>きょどう</rt></ruby>の<ruby>正<rt>ただ</rt></ruby>しさそのものを<ruby>証明<rt>しょうめい</rt></ruby>しない。<ruby>期待値<rt>きたいち</rt></ruby>の<ruby>根拠<rt>こんきょ</rt></ruby>、<ruby>実行<rt>じっこう</rt></ruby>コマンドとlog、<ruby>対象<rt>たいしょう</rt></ruby>source、coverageを<ruby>独立<rt>どくりつ</rt></ruby>レビューする。source\/doc<ruby>仕様<rt>しよう</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>した<ruby>後<rt>あと</rt></ruby>は<ruby>古<rt>ふる</rt></ruby>い<ruby>証拠<rt>しょうこ</rt></ruby>でcurrent passedを<ruby>維持<rt>いじ</rt></ruby>せず<ruby>再実行<rt>さいじっこう</rt></ruby>する。T16は<ruby>全<rt>ぜん</rt></ruby>registered required<ruby>群<rt>ぐん</rt></ruby>の<ruby>型付<rt>かたつ</rt></ruby>き<ruby>証拠<rt>しょうこ</rt></ruby>を<ruby>動的<rt>どうてき</rt></ruby>に<ruby>確認<rt>かくにん</rt></ruby>する。

この<ruby>設計<rt>せっけい</rt></ruby>パッケージの<ruby>検査<rt>けんさ</rt></ruby>は<ruby>別扱<rt>べつあつか</rt></ruby>い。`doc/history/design-validation.json` に、<ruby>構文例<rt>こうぶんれい</rt></ruby>の<ruby>構造検査<rt>こうぞうけんさ</rt></ruby>、<ruby>依存<rt>いぞん</rt></ruby>DAG、task ID、JSON、<ruby>例<rt>れい</rt></ruby>の<ruby>独立算術<rt>どくりつさんじゅつ</rt></ruby>\/<ruby>回路検算<rt>かいろけんざん</rt></ruby>などの<ruby>実施範囲<rt>じっしはんい</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>する。Rust compiler\/editor\/browserが<ruby>完成<rt>かんせい</rt></ruby>しているという<ruby>証拠<rt>しょうこ</rt></ruby>に<ruby>使<rt>つか</rt></ruby>わない。
