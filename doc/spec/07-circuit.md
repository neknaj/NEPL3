<!-- Generated from doc/spec/07&#45;circuit.nepld; renderer nepl3-tools.markdown-annotated/3; source SHA-256 cf169539cb22eb1a7b21fc5c3f15d060975453e7fd72534126f894548c4fbe30; alias input SHA-256 7c23dac644010c8dfb51fae2b60e7ac78510e84492dd442386ff3679536eb53a; document digest 60aae4046a1cbf0e395a06db8ee5424939ae9a5d95e0a111cabe36954692e35d. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="07-circuit言語"></a>

# 07\. Circuit<ruby>言語<rt>げんご</rt></ruby>

[正本（NEPL3d）](<07-circuit.nepld>)

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## <ruby>方針<rt>ほうしん</rt></ruby>

<ruby>二値<rt>にち</rt></ruby>・<ruby>固定幅<rt>こていはば</rt></ruby>・<ruby>同期離散時間<rt>どうきりさんじかん</rt></ruby>の<ruby>回路<rt>かいろ</rt></ruby>を<ruby>正式<rt>せいしき</rt></ruby>な<ruby>意味領域<rt>いみりょういき</rt></ruby>とする。<ruby>構文木<rt>こうぶんき</rt></ruby>と<ruby>回路<rt>かいろ</rt></ruby>graph、<ruby>回路生成<rt>かいろせいせい</rt></ruby>と<ruby>信号<rt>しんごう</rt></ruby>の<ruby>実行値<rt>じっこうち</rt></ruby>、<ruby>定義<rt>ていぎ</rt></ruby>の<ruby>再利用<rt>さいりよう</rt></ruby>と<ruby>状態<rt>じょうたい</rt></ruby>の<ruby>共有<rt>きょうゆう</rt></ruby>を<ruby>分離<rt>ぶんり</rt></ruby>する。

<a name="n-73757266616365"></a>

<a name="1-surfaceと型"></a>

## 1\. surfaceと<ruby>型<rt>かた</rt></ruby>

Designはmodules、entry<ruby>名<rt>めい</rt></ruby>、tests。Moduleはname、<ruby>入力列<rt>にゅうりょくれつ</rt></ruby>、<ruby>宣言列<rt>せんげんれつ</rt></ruby>、<ruby>出力列<rt>しゅつりょくれつ</rt></ruby>。<ruby>入力<rt>にゅうりょく</rt></ruby>\/<ruby>出力<rt>しゅつりょく</rt></ruby>\/stateの<ruby>幅<rt>はば</rt></ruby>は<ruby>正<rt>せい</rt></ruby>のNat。upper boundは<ruby>構成上<rt>こうせいじょう</rt></ruby>のLimitsで<ruby>決<rt>き</rt></ruby>まり、<ruby>暗黙<rt>あんもく</rt></ruby>の32\/64bitに<ruby>制限<rt>せいげん</rt></ruby>しない。

bit\-vectorは `(width, unsigned value)`、`0 <= value < 2^width`。bits width valueの<ruby>範囲外<rt>はんいがい</rt></ruby>はエラーで、literalの<ruby>切捨<rt>きりす</rt></ruby>てをしない。true\/falseは1bit。

<ruby>信号名<rt>しんごうめい</rt></ruby>はinput\/wire\/stateの<ruby>共通空間<rt>きょうつうくうかん</rt></ruby>。instance<ruby>名<rt>めい</rt></ruby>はInstance<ruby>空間<rt>くうかん</rt></ruby>、module<ruby>名<rt>めい</rt></ruby>はDesign<ruby>内<rt>ない</rt></ruby>のModule<ruby>空間<rt>くうかん</rt></ruby>、output<ruby>名<rt>めい</rt></ruby>は<ruby>各<rt>かく</rt></ruby>moduleのOutput<ruby>空間<rt>くうかん</rt></ruby>。<ruby>同<rt>おな</rt></ruby>じ<ruby>空間<rt>くうかん</rt></ruby>の<ruby>重複<rt>ちょうふく</rt></ruby>はエラー。outputは<ruby>内部<rt>ないぶ</rt></ruby>で<ruby>裸<rt>はだか</rt></ruby>の<ruby>名前参照<rt>なまえさんしょう</rt></ruby>として<ruby>再利用<rt>さいりよう</rt></ruby>しない。<ruby>共有<rt>きょうゆう</rt></ruby>したい<ruby>式<rt>しき</rt></ruby>はwireで<ruby>宣言<rt>せんげん</rt></ruby>する。

Name leafは<ruby>信号参照<rt>しんごうさんしょう</rt></ruby>。at instance outputは、そのinstanceが<ruby>参照<rt>さんしょう</rt></ruby>するmoduleのoutputを<ruby>参照<rt>さんしょう</rt></ruby>する。definition linkにはinstance siteとoutput definitionの<ruby>双方<rt>そうほう</rt></ruby>を<ruby>提示<rt>ていじ</rt></ruby>できる。

<a name="n-6465636c61726174696f6e73"></a>

<a name="2-各宣言"></a>

## 2\. <ruby>各宣言<rt>かくせんげん</rt></ruby>

- `wire name expr`\: exprの<ruby>値<rt>あたい</rt></ruby>を<ruby>共有<rt>きょうゆう</rt></ruby>する<ruby>組合<rt>くみあわ</rt></ruby>せ<ruby>信号<rt>しんごう</rt></ruby>。<ruby>宣言順<rt>せんげんじゅん</rt></ruby>に<ruby>依存<rt>いぞん</rt></ruby>せず<ruby>前方参照<rt>ぜんぽうさんしょう</rt></ruby>を<ruby>認<rt>みと</rt></ruby>める。
- `state name width initial`\: <ruby>初期値<rt>しょきち</rt></ruby>を<ruby>持<rt>も</rt></ruby>つ<ruby>状態<rt>じょうたい</rt></ruby>。initialはBitsのliteralで<ruby>幅<rt>はば</rt></ruby>が<ruby>一致<rt>いっち</rt></ruby>する<ruby>必要<rt>ひつよう</rt></ruby>がある。
- `next name expr`\: <ruby>次<rt>つぎ</rt></ruby>のtickのstate<ruby>値<rt>ち</rt></ruby>。<ruby>対象<rt>たいしょう</rt></ruby>はstateに<ruby>限<rt>かぎ</rt></ruby>り、<ruby>各<rt>かく</rt></ruby>stateにちょうど<ruby>一<rt>ひと</rt></ruby>つ<ruby>必要<rt>ひつよう</rt></ruby>。<ruby>省略<rt>しょうりゃく</rt></ruby>を<ruby>暗黙<rt>あんもく</rt></ruby>の<ruby>保持<rt>ほじ</rt></ruby>としない。<ruby>保持<rt>ほじ</rt></ruby>は `next q q` と<ruby>明示<rt>めいじ</rt></ruby>する。
- `inst name module arguments`\: moduleの<ruby>入力列<rt>にゅうりょくれつ</rt></ruby>の<ruby>順<rt>じゅん</rt></ruby>に<ruby>引数<rt>ひきすう</rt></ruby>を<ruby>接続<rt>せつぞく</rt></ruby>する。<ruby>毎回独立<rt>まいかいどくりつ</rt></ruby>したinstanceを<ruby>作<rt>つく</rt></ruby>る。<ruby>定義<rt>ていぎ</rt></ruby>の<ruby>構文<rt>こうぶん</rt></ruby>fragmentの<ruby>共有<rt>きょうゆう</rt></ruby>は<ruby>状態<rt>じょうたい</rt></ruby>の<ruby>共有<rt>きょうゆう</rt></ruby>を<ruby>意味<rt>いみ</rt></ruby>しない。
- `output name width expr`\: <ruby>外部出力<rt>がいぶしゅつりょく</rt></ruby>。<ruby>明示幅<rt>めいじはば</rt></ruby>とexpr<ruby>幅<rt>はば</rt></ruby>の<ruby>一致<rt>いっち</rt></ruby>を<ruby>要求<rt>ようきゅう</rt></ruby>する。

<a name="n-6f7065726174696f6e73"></a>

<a name="3-演算"></a>

## 3\. <ruby>演算<rt>えんざん</rt></ruby>

notはbitwise。and\/or\/xor\/nor\/addは<ruby>等幅<rt>とうはば</rt></ruby>の2<ruby>入力<rt>にゅうりょく</rt></ruby>。addはmod 2\^widthの<ruby>加算<rt>かさん</rt></ruby>。<ruby>符号付<rt>ふごうつ</rt></ruby>き<ruby>解釈<rt>かいしゃく</rt></ruby>はしない。

muxは1bit selectと<ruby>等幅<rt>とうはば</rt></ruby>のyes\/no。select\=1ならyes。

concat a bはaを<ruby>上位<rt>じょうい</rt></ruby>、bを<ruby>下位<rt>かい</rt></ruby>へ<ruby>置<rt>お</rt></ruby>き、<ruby>幅<rt>はば</rt></ruby>はwa\+wb。

slice value lo widthはLSB\=0として `[lo,lo+width)` を<ruby>抜<rt>ぬ</rt></ruby>き<ruby>出<rt>だ</rt></ruby>す。width\>0かつlo\+width\<\=<ruby>入力幅<rt>にゅうりょくはば</rt></ruby>。<ruby>全加算<rt>ぜんかさん</rt></ruby>・index<ruby>計算<rt>けいさん</rt></ruby>をcheckedにする。

<a name="n-656c61626f726174696f6e"></a>

<a name="4-検査とelaboration"></a>

## 4\. <ruby>検査<rt>けんさ</rt></ruby>とelaboration

1. Design<ruby>内<rt>ない</rt></ruby>のmodule signatureを<ruby>収集<rt>しゅうしゅう</rt></ruby>する。entryと<ruby>各<rt>かく</rt></ruby>instanceのmoduleを<ruby>解決<rt>かいけつ</rt></ruby>する。
1. module\-instantiation graphがDAGであることを<ruby>検査<rt>けんさ</rt></ruby>する。<ruby>再帰<rt>さいき</rt></ruby>instantiationは<ruby>有限<rt>ゆうげん</rt></ruby>な<ruby>回路<rt>かいろ</rt></ruby>にならないので<ruby>拒否<rt>きょひ</rt></ruby>する。
1. <ruby>各<rt>かく</rt></ruby>instanceを<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>階層<rt>かいそう</rt></ruby>pathへ<ruby>展開<rt>てんかい</rt></ruby>し、<ruby>信号参照<rt>しんごうさんしょう</rt></ruby>と<ruby>状態<rt>じょうたい</rt></ruby>を<ruby>結<rt>むす</rt></ruby>ぶ。<ruby>未使用<rt>みしよう</rt></ruby>moduleも<ruby>単独<rt>たんどく</rt></ruby>に<ruby>検査<rt>けんさ</rt></ruby>し、<ruby>不正<rt>ふせい</rt></ruby>な<ruby>定義<rt>ていぎ</rt></ruby>を<ruby>放置<rt>ほうち</rt></ruby>しない。
1. <ruby>各<rt>かく</rt></ruby>state readを<ruby>組合<rt>くみあわ</rt></ruby>せgraphの<ruby>入力<rt>にゅうりょく</rt></ruby>として<ruby>扱<rt>あつか</rt></ruby>い、next assignmentを<ruby>別<rt>べつ</rt></ruby>のsinkにする。state readからnext exprへ<ruby>逆<rt>ぎゃく</rt></ruby>の<ruby>依存<rt>いぞん</rt></ruby>edgeを<ruby>加<rt>くわ</rt></ruby>えない。
1. その<ruby>実際<rt>じっさい</rt></ruby>のgraphで<ruby>組合<rt>くみあわ</rt></ruby>せcycleを<ruby>検査<rt>けんさ</rt></ruby>する。instanceの<ruby>全入力<rt>ぜんにゅうりょく</rt></ruby>が<ruby>全出力<rt>ぜんしゅつりょく</rt></ruby>に<ruby>依存<rt>いぞん</rt></ruby>すると<ruby>近似<rt>きんじ</rt></ruby>してはいけない。<ruby>内部<rt>ないぶ</rt></ruby>stateで<ruby>遮断<rt>しゃだん</rt></ruby>されるfeedbackを<ruby>誤拒否<rt>ごきょひ</rt></ruby>しない。
1. DAGの<ruby>順序<rt>じゅんじょ</rt></ruby>でwidthを<ruby>推論<rt>すいろん</rt></ruby>・<ruby>検査<rt>けんさ</rt></ruby>する。input\/state\/constantが<ruby>既知幅<rt>きちはば</rt></ruby>のroot。<ruby>各<rt>かく</rt></ruby>output\/state\-next\/instance\-inputの<ruby>要求幅<rt>ようきゅうはば</rt></ruby>と<ruby>照合<rt>しょうごう</rt></ruby>する。

<ruby>共有<rt>きょうゆう</rt></ruby>wireは<ruby>同<rt>おな</rt></ruby>じsignal identity。<ruby>再利用<rt>さいりよう</rt></ruby>moduleは<ruby>異<rt>こと</rt></ruby>なるinstance identity。instance pathはowner module instanceと<ruby>宣言<rt>せんげん</rt></ruby>ordinalで<ruby>一意<rt>いちい</rt></ruby>にし、<ruby>表示名<rt>ひょうじめい</rt></ruby>の<ruby>連結<rt>れんけつ</rt></ruby>だけでIDを<ruby>作<rt>つく</rt></ruby>らない。

<a name="n-74696d65"></a>

<a name="5-stepと時間"></a>

## 5\. stepと<ruby>時間<rt>じかん</rt></ruby>

<ruby>状態<rt>じょうたい</rt></ruby>q\_k、<ruby>入力<rt>にゅうりょく</rt></ruby>u\_kに<ruby>対<rt>たい</rt></ruby>し、<ruby>組合<rt>くみあわ</rt></ruby>せgraphからy\_kとnext<ruby>値<rt>ち</rt></ruby>を<ruby>計算<rt>けいさん</rt></ruby>し、<ruby>結果<rt>けっか</rt></ruby>として\(q\_\{k\+1\}\,y\_k\)を<ruby>返<rt>かえ</rt></ruby>す。<ruby>全<rt>ぜん</rt></ruby>stateは<ruby>同時更新<rt>どうじこうしん</rt></ruby>。stateを<ruby>逐次上書<rt>ちくじうわが</rt></ruby>きして<ruby>後続<rt>こうぞく</rt></ruby>のnext<ruby>計算<rt>けいさん</rt></ruby>へ<ruby>影響<rt>えいきょう</rt></ruby>させない。

`initial(PreparedNetlist)` は、entryを<ruby>選<rt>えら</rt></ruby>んでelaborateした<ruby>全<rt>ぜん</rt></ruby>instanceのstate<ruby>初期値<rt>しょきち</rt></ruby>をslot<ruby>順<rt>じゅん</rt></ruby>に<ruby>返<rt>かえ</rt></ruby>す。CheckedDesignとentryはelaborateの<ruby>入力<rt>にゅうりょく</rt></ruby>であり、initialは<ruby>階層展開<rt>かいそうてんかい</rt></ruby>と<ruby>組合<rt>くみあわ</rt></ruby>せDAG・<ruby>幅<rt>はば</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>を<ruby>済<rt>す</rt></ruby>ませた<ruby>値<rt>あたい</rt></ruby>だけを<ruby>受<rt>う</rt></ruby>ける。`step` は<ruby>入力<rt>にゅうりょく</rt></ruby>stateを<ruby>変更<rt>へんこう</rt></ruby>せず<ruby>新<rt>あたら</rt></ruby>しい<ruby>値<rt>あたい</rt></ruby>を<ruby>返<rt>かえ</rt></ruby>す。`observe` は<ruby>現在<rt>げんざい</rt></ruby>の<ruby>出力<rt>しゅつりょく</rt></ruby>だけを<ruby>返<rt>かえ</rt></ruby>す<ruby>純粋操作<rt>じゅんすいそうさ</rt></ruby>。<ruby>入力列<rt>にゅうりょくれつ</rt></ruby>と<ruby>出力列<rt>しゅつりょくれつ</rt></ruby>の<ruby>順<rt>じゅん</rt></ruby>はmodule<ruby>宣言順<rt>せんげんじゅん</rt></ruby>。

test name module ticksはそのtest<ruby>開始時<rt>かいしじ</rt></ruby>にinitial<ruby>状態<rt>じょうたい</rt></ruby>へ<ruby>戻<rt>もど</rt></ruby>す。tick inputs expectedOutputsはpre\-edgeのy\_kを<ruby>比較<rt>ひかく</rt></ruby>してからstateを<ruby>更新<rt>こうしん</rt></ruby>する。<ruby>同<rt>おな</rt></ruby>じtest<ruby>内<rt>ない</rt></ruby>のticksは<ruby>連続時間<rt>れんぞくじかん</rt></ruby>。<ruby>別<rt>べつ</rt></ruby>testでは<ruby>状態<rt>じょうたい</rt></ruby>を<ruby>共有<rt>きょうゆう</rt></ruby>しない。<ruby>期待列<rt>きたいれつ</rt></ruby>の<ruby>数<rt>かず</rt></ruby>\/<ruby>幅<rt>はば</rt></ruby>の<ruby>不一致<rt>ふいっち</rt></ruby>はtest<ruby>失敗<rt>しっぱい</rt></ruby>と<ruby>別<rt>べつ</rt></ruby>のInvalidTestShape。

この<ruby>仕様<rt>しよう</rt></ruby>は<ruby>非同期<rt>ひどうき</rt></ruby>latch、<ruby>伝播遅延<rt>でんぱちえん</rt></ruby>、X\/Z、アナログ<ruby>挙動<rt>きょどう</rt></ruby>をモデル<ruby>化<rt>か</rt></ruby>しない。これらを<ruby>適当<rt>てきとう</rt></ruby>に<ruby>二値<rt>にち</rt></ruby>へ<ruby>潰<rt>つぶ</rt></ruby>す<ruby>変換<rt>へんかん</rt></ruby>は<ruby>許可<rt>きょか</rt></ruby>しない。<ruby>別<rt>べつ</rt></ruby>modelを<ruby>導入<rt>どうにゅう</rt></ruby>するならschemaと<ruby>操作<rt>そうさ</rt></ruby>の<ruby>版<rt>ばん</rt></ruby>を<ruby>分<rt>わ</rt></ruby>ける。

<a name="n-6e6f72"></a>

<a name="6-nor-ir"></a>

## 6\. NOR IRへの<ruby>変換<rt>へんかん</rt></ruby>

<ruby>正式<rt>せいしき</rt></ruby>なlowering<ruby>先<rt>さき</rt></ruby>は4<ruby>種<rt>しゅ</rt></ruby>のnode（InputBit、StateBit、ConstBit、Nor2）と2<ruby>種<rt>しゅ</rt></ruby>のsink（nextBits、outputBits）から<ruby>成<rt>な</rt></ruby>るNorNetlist。vector\-level<ruby>演算<rt>えんざん</rt></ruby>をbit\-levelへ<ruby>展開<rt>てんかい</rt></ruby>する。NextBit\/OutputBitという<ruby>独立<rt>どくりつ</rt></ruby>node variantは<ruby>作<rt>つく</rt></ruby>らず、sink<ruby>配列<rt>はいれつ</rt></ruby>の<ruby>各項<rt>かくこう</rt></ruby>が<ruby>既存<rt>きそん</rt></ruby>nodeのindexを<ruby>参照<rt>さんしょう</rt></ruby>する。StateBitとnextBitsを<ruby>維持<rt>いじ</rt></ruby>するので<ruby>順序回路<rt>じゅんじょかいろ</rt></ruby>も<ruby>表現<rt>ひょうげん</rt></ruby>できる。stateの<ruby>初期<rt>しょき</rt></ruby>bit<ruby>列<rt>れつ</rt></ruby>、sinkの<ruby>長<rt>なが</rt></ruby>さとLSBからの<ruby>順序<rt>じゅんじょ</rt></ruby>はモデル<ruby>不変条件<rt>ふへんじょうけん</rt></ruby>に<ruby>従<rt>したが</rt></ruby>う。

not a \= nor a a。or a b \= not \(nor a b\)。and a b \= nor \(not a\) \(not b\)。xor a b \= nor \(nor a b\) \(and a b\)。mux \= \(select AND yes\) OR \(\(NOT select\) AND no\)。addはLSBからcarryを<ruby>伝<rt>つた</rt></ruby>えるfull\-adderで<ruby>最終<rt>さいしゅう</rt></ruby>carryを<ruby>捨<rt>す</rt></ruby>てる。slice\/concat\/wireはbit<ruby>参照<rt>さんしょう</rt></ruby>の<ruby>配線<rt>はいせん</rt></ruby>になる。

<ruby>式<rt>しき</rt></ruby>の<ruby>入力順<rt>にゅうりょくじゅん</rt></ruby>、bit<ruby>順<rt>じゅん</rt></ruby>、<ruby>生成<rt>せいせい</rt></ruby>node ordinalを<ruby>固定<rt>こてい</rt></ruby>し、<ruby>同<rt>おな</rt></ruby>じ<ruby>入力<rt>にゅうりょく</rt></ruby>から<ruby>同<rt>おな</rt></ruby>じIRを<ruby>出<rt>だ</rt></ruby>す。<ruby>任意最適化<rt>にんいさいてきか</rt></ruby>をreference loweringへ<ruby>混在<rt>こんざい</rt></ruby>させない。<ruby>最適化<rt>さいてきか</rt></ruby>を<ruby>提供<rt>ていきょう</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>は<ruby>別<rt>べつ</rt></ruby>operationとして<ruby>同値<rt>どうち</rt></ruby>テストとsource mapを<ruby>要求<rt>ようきゅう</rt></ruby>する。

<ruby>比較試験<rt>ひかくしけん</rt></ruby>は<ruby>元<rt>もと</rt></ruby>のvector evaluatorとNOR evaluatorの<ruby>両方<rt>りょうほう</rt></ruby>で、<ruby>全小幅入力<rt>ぜんしょうはばにゅうりょく</rt></ruby>・<ruby>複数<rt>ふくすう</rt></ruby>tickを<ruby>比較<rt>ひかく</rt></ruby>する。NOR evaluatorを<ruby>元<rt>もと</rt></ruby>のevaluatorと<ruby>同<rt>おな</rt></ruby>じコードへの<ruby>単<rt>たん</rt></ruby>なるaliasにしない。

<a name="n-737667"></a>

<a name="7-svg-backend"></a>

## 7\. SVG backendの<ruby>配置<rt>はいち</rt></ruby>

elaborated vector\-level graphを<ruby>対象<rt>たいしょう</rt></ruby>に、<ruby>入力<rt>にゅうりょく</rt></ruby>\/stateをrank0、<ruby>各組合<rt>かくくみあわ</rt></ruby>せnodeを<ruby>前提<rt>ぜんてい</rt></ruby>nodeの<ruby>最大<rt>さいだい</rt></ruby>rank\+1、<ruby>出力<rt>しゅつりょく</rt></ruby>を<ruby>最後<rt>さいご</rt></ruby>に<ruby>配置<rt>はいち</rt></ruby>する。<ruby>同<rt>どう</rt></ruby>rank<ruby>内<rt>ない</rt></ruby>は<ruby>定義<rt>ていぎ</rt></ruby>\/instance ordinal<ruby>順<rt>じゅん</rt></ruby>。<ruby>固定<rt>こてい</rt></ruby>paddingとlabelのscalar<ruby>数<rt>すう</rt></ruby>から<ruby>幅<rt>はば</rt></ruby>を<ruby>決<rt>き</rt></ruby>め、SVG textLengthを<ruby>使<rt>つか</rt></ruby>って<ruby>描画幅<rt>びょうがはば</rt></ruby>を<ruby>明示<rt>めいじ</rt></ruby>する。font<ruby>依存<rt>いぞん</rt></ruby>でSVG byte<ruby>列<rt>れつ</rt></ruby>を<ruby>変<rt>か</rt></ruby>えない。

edgeはrank<ruby>間<rt>かん</rt></ruby>のlaneをordinalで<ruby>割<rt>わ</rt></ruby>り<ruby>当<rt>あ</rt></ruby>てた<ruby>直交線<rt>ちょっこうせん</rt></ruby>。stateへのnext edgeは<ruby>破線<rt>はせん</rt></ruby>で<ruby>表示<rt>ひょうじ</rt></ruby>し、<ruby>組合<rt>くみあわ</rt></ruby>せedgeと<ruby>区別<rt>くべつ</rt></ruby>する。node\/edgeにはEntityIdとOriginを<ruby>対応付<rt>たいおうづ</rt></ruby>け、クリックでソースに<ruby>戻<rt>もど</rt></ruby>せる。<ruby>最小交差数<rt>さいしょうこうさすう</rt></ruby>や<ruby>最適配置<rt>さいてきはいち</rt></ruby>を<ruby>保証<rt>ほしょう</rt></ruby>する<ruby>算法<rt>さんぽう</rt></ruby>とはしない。

<ruby>全<rt>ぜん</rt></ruby>ラベルはescapeし、<ruby>任意<rt>にんい</rt></ruby>scriptやURLを<ruby>挿入<rt>そうにゅう</rt></ruby>しない。

<a name="n-7075626c6963"></a>

<a name="8-公開操作"></a>

## 8\. <ruby>公開操作<rt>こうかいそうさ</rt></ruby>

lower、check、elaborate、initial、observe、step、run\_tests、lower\_nor、print、diagram。CheckedDesignは<ruby>必要<rt>ひつよう</rt></ruby>なmodule<ruby>検査<rt>けんさ</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>む。PreparedNetlistはinstance<ruby>展開<rt>てんかい</rt></ruby>と<ruby>組合<rt>くみあわ</rt></ruby>せDAG\/<ruby>幅検査済<rt>はばけんさず</rt></ruby>み。sourceをparseしただけでsimulationを<ruby>開始<rt>かいし</rt></ruby>しない。
