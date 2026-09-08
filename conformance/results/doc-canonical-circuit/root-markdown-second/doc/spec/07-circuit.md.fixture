<!-- Generated from doc/spec/07&#45;circuit.nepld; renderer nepl3-tools.markdown-annotated/2; source SHA-256 cf169539cb22eb1a7b21fc5c3f15d060975453e7fd72534126f894548c4fbe30; alias input SHA-256 7c23dac644010c8dfb51fae2b60e7ac78510e84492dd442386ff3679536eb53a; document digest 7a6f807d68ac693b1a1a50a2f0eacc930fbeb301ea2cae28f42014b123bff73e. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="07-circuit言語"></a>

# 07\. Circuit言語\[げんご\]

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

二値\[にち\]・固定幅\[こていはば\]・同期離散時間\[どうきりさんじかん\]の回路\[かいろ\]を正式\[せいしき\]な意味領域\[いみりょういき\]とする。構文木\[こうぶんき\]と回路\[かいろ\]graph、回路生成\[かいろせいせい\]と信号\[しんごう\]の実行値\[じっこうち\]、定義\[ていぎ\]の再利用\[さいりよう\]と状態\[じょうたい\]の共有\[きょうゆう\]を分離\[ぶんり\]する。

<a name="n-73757266616365"></a>

<a name="1-surfaceと型"></a>

## 1\. surfaceと型\[かた\]

Designはmodules、entry名\[めい\]、tests。Moduleはname、入力列\[にゅうりょくれつ\]、宣言列\[せんげんれつ\]、出力列\[しゅつりょくれつ\]。入力\[にゅうりょく\]\/出力\[しゅつりょく\]\/stateの幅\[はば\]は正\[せい\]のNat。upper boundは構成上\[こうせいじょう\]のLimitsで決\[き\]まり、暗黙\[あんもく\]の32\/64bitに制限\[せいげん\]しない。

bit\-vectorは `(width, unsigned value)`、`0 <= value < 2^width`。bits width valueの範囲外\[はんいがい\]はエラーで、literalの切捨\[きりす\]てをしない。true\/falseは1bit。

信号名\[しんごうめい\]はinput\/wire\/stateの共通空間\[きょうつうくうかん\]。instance名\[めい\]はInstance空間\[くうかん\]、module名\[めい\]はDesign内\[ない\]のModule空間\[くうかん\]、output名\[めい\]は各\[かく\]moduleのOutput空間\[くうかん\]。同\[おな\]じ空間\[くうかん\]の重複\[ちょうふく\]はエラー。outputは内部\[ないぶ\]で裸\[はだか\]の名前参照\[なまえさんしょう\]として再利用\[さいりよう\]しない。共有\[きょうゆう\]したい式\[しき\]はwireで宣言\[せんげん\]する。

Name leafは信号参照\[しんごうさんしょう\]。at instance outputは、そのinstanceが参照\[さんしょう\]するmoduleのoutputを参照\[さんしょう\]する。definition linkにはinstance siteとoutput definitionの双方\[そうほう\]を提示\[ていじ\]できる。

<a name="n-6465636c61726174696f6e73"></a>

<a name="2-各宣言"></a>

## 2\. 各宣言\[かくせんげん\]

- `wire name expr`\: exprの値\[あたい\]を共有\[きょうゆう\]する組合\[くみあわ\]せ信号\[しんごう\]。宣言順\[せんげんじゅん\]に依存\[いぞん\]せず前方参照\[ぜんぽうさんしょう\]を認\[みと\]める。
- `state name width initial`\: 初期値\[しょきち\]を持\[も\]つ状態\[じょうたい\]。initialはBitsのliteralで幅\[はば\]が一致\[いっち\]する必要\[ひつよう\]がある。
- `next name expr`\: 次\[つぎ\]のtickのstate値\[ち\]。対象\[たいしょう\]はstateに限\[かぎ\]り、各\[かく\]stateにちょうど一\[ひと\]つ必要\[ひつよう\]。省略\[しょうりゃく\]を暗黙\[あんもく\]の保持\[ほじ\]としない。保持\[ほじ\]は `next q q` と明示\[めいじ\]する。
- `inst name module arguments`\: moduleの入力列\[にゅうりょくれつ\]の順\[じゅん\]に引数\[ひきすう\]を接続\[せつぞく\]する。毎回独立\[まいかいどくりつ\]したinstanceを作\[つく\]る。定義\[ていぎ\]の構文\[こうぶん\]fragmentの共有\[きょうゆう\]は状態\[じょうたい\]の共有\[きょうゆう\]を意味\[いみ\]しない。
- `output name width expr`\: 外部出力\[がいぶしゅつりょく\]。明示幅\[めいじはば\]とexpr幅\[はば\]の一致\[いっち\]を要求\[ようきゅう\]する。

<a name="n-6f7065726174696f6e73"></a>

<a name="3-演算"></a>

## 3\. 演算\[えんざん\]

notはbitwise。and\/or\/xor\/nor\/addは等幅\[とうはば\]の2入力\[にゅうりょく\]。addはmod 2\^widthの加算\[かさん\]。符号付\[ふごうつ\]き解釈\[かいしゃく\]はしない。

muxは1bit selectと等幅\[とうはば\]のyes\/no。select\=1ならyes。

concat a bはaを上位\[じょうい\]、bを下位\[かい\]へ置\[お\]き、幅\[はば\]はwa\+wb。

slice value lo widthはLSB\=0として `[lo,lo+width)` を抜\[ぬ\]き出\[だ\]す。width\>0かつlo\+width\<\=入力幅\[にゅうりょくはば\]。全加算\[ぜんかさん\]・index計算\[けいさん\]をcheckedにする。

<a name="n-656c61626f726174696f6e"></a>

<a name="4-検査とelaboration"></a>

## 4\. 検査\[けんさ\]とelaboration

1. Design内\[ない\]のmodule signatureを収集\[しゅうしゅう\]する。entryと各\[かく\]instanceのmoduleを解決\[かいけつ\]する。
1. module\-instantiation graphがDAGであることを検査\[けんさ\]する。再帰\[さいき\]instantiationは有限\[ゆうげん\]な回路\[かいろ\]にならないので拒否\[きょひ\]する。
1. 各\[かく\]instanceを独立\[どくりつ\]した階層\[かいそう\]pathへ展開\[てんかい\]し、信号参照\[しんごうさんしょう\]と状態\[じょうたい\]を結\[むす\]ぶ。未使用\[みしよう\]moduleも単独\[たんどく\]に検査\[けんさ\]し、不正\[ふせい\]な定義\[ていぎ\]を放置\[ほうち\]しない。
1. 各\[かく\]state readを組合\[くみあわ\]せgraphの入力\[にゅうりょく\]として扱\[あつか\]い、next assignmentを別\[べつ\]のsinkにする。state readからnext exprへ逆\[ぎゃく\]の依存\[いぞん\]edgeを加\[くわ\]えない。
1. その実際\[じっさい\]のgraphで組合\[くみあわ\]せcycleを検査\[けんさ\]する。instanceの全入力\[ぜんにゅうりょく\]が全出力\[ぜんしゅつりょく\]に依存\[いぞん\]すると近似\[きんじ\]してはいけない。内部\[ないぶ\]stateで遮断\[しゃだん\]されるfeedbackを誤拒否\[ごきょひ\]しない。
1. DAGの順序\[じゅんじょ\]でwidthを推論\[すいろん\]・検査\[けんさ\]する。input\/state\/constantが既知幅\[きちはば\]のroot。各\[かく\]output\/state\-next\/instance\-inputの要求幅\[ようきゅうはば\]と照合\[しょうごう\]する。

共有\[きょうゆう\]wireは同\[おな\]じsignal identity。再利用\[さいりよう\]moduleは異\[こと\]なるinstance identity。instance pathはowner module instanceと宣言\[せんげん\]ordinalで一意\[いちい\]にし、表示名\[ひょうじめい\]の連結\[れんけつ\]だけでIDを作\[つく\]らない。

<a name="n-74696d65"></a>

<a name="5-stepと時間"></a>

## 5\. stepと時間\[じかん\]

状態\[じょうたい\]q\_k、入力\[にゅうりょく\]u\_kに対\[たい\]し、組合\[くみあわ\]せgraphからy\_kとnext値\[ち\]を計算\[けいさん\]し、結果\[けっか\]として\(q\_\{k\+1\}\,y\_k\)を返\[かえ\]す。全\[ぜん\]stateは同時更新\[どうじこうしん\]。stateを逐次上書\[ちくじうわが\]きして後続\[こうぞく\]のnext計算\[けいさん\]へ影響\[えいきょう\]させない。

`initial(PreparedNetlist)` は、entryを選\[えら\]んでelaborateした全\[ぜん\]instanceのstate初期値\[しょきち\]をslot順\[じゅん\]に返\[かえ\]す。CheckedDesignとentryはelaborateの入力\[にゅうりょく\]であり、initialは階層展開\[かいそうてんかい\]と組合\[くみあわ\]せDAG・幅\[はば\]の検査\[けんさ\]を済\[す\]ませた値\[あたい\]だけを受\[う\]ける。`step` は入力\[にゅうりょく\]stateを変更\[へんこう\]せず新\[あたら\]しい値\[あたい\]を返\[かえ\]す。`observe` は現在\[げんざい\]の出力\[しゅつりょく\]だけを返\[かえ\]す純粋操作\[じゅんすいそうさ\]。入力列\[にゅうりょくれつ\]と出力列\[しゅつりょくれつ\]の順\[じゅん\]はmodule宣言順\[せんげんじゅん\]。

test name module ticksはそのtest開始時\[かいしじ\]にinitial状態\[じょうたい\]へ戻\[もど\]す。tick inputs expectedOutputsはpre\-edgeのy\_kを比較\[ひかく\]してからstateを更新\[こうしん\]する。同\[おな\]じtest内\[ない\]のticksは連続時間\[れんぞくじかん\]。別\[べつ\]testでは状態\[じょうたい\]を共有\[きょうゆう\]しない。期待列\[きたいれつ\]の数\[かず\]\/幅\[はば\]の不一致\[ふいっち\]はtest失敗\[しっぱい\]と別\[べつ\]のInvalidTestShape。

この仕様\[しよう\]は非同期\[ひどうき\]latch、伝播遅延\[でんぱちえん\]、X\/Z、アナログ挙動\[きょどう\]をモデル化\[か\]しない。これらを適当\[てきとう\]に二値\[にち\]へ潰\[つぶ\]す変換\[へんかん\]は許可\[きょか\]しない。別\[べつ\]modelを導入\[どうにゅう\]するならschemaと操作\[そうさ\]の版\[ばん\]を分\[わ\]ける。

<a name="n-6e6f72"></a>

<a name="6-nor-ir"></a>

## 6\. NOR IRへの変換\[へんかん\]

正式\[せいしき\]なlowering先\[さき\]は4種\[しゅ\]のnode（InputBit、StateBit、ConstBit、Nor2）と2種\[しゅ\]のsink（nextBits、outputBits）から成\[な\]るNorNetlist。vector\-level演算\[えんざん\]をbit\-levelへ展開\[てんかい\]する。NextBit\/OutputBitという独立\[どくりつ\]node variantは作\[つく\]らず、sink配列\[はいれつ\]の各項\[かくこう\]が既存\[きそん\]nodeのindexを参照\[さんしょう\]する。StateBitとnextBitsを維持\[いじ\]するので順序回路\[じゅんじょかいろ\]も表現\[ひょうげん\]できる。stateの初期\[しょき\]bit列\[れつ\]、sinkの長\[なが\]さとLSBからの順序\[じゅんじょ\]はモデル不変条件\[ふへんじょうけん\]に従\[したが\]う。

not a \= nor a a。or a b \= not \(nor a b\)。and a b \= nor \(not a\) \(not b\)。xor a b \= nor \(nor a b\) \(and a b\)。mux \= \(select AND yes\) OR \(\(NOT select\) AND no\)。addはLSBからcarryを伝\[つた\]えるfull\-adderで最終\[さいしゅう\]carryを捨\[す\]てる。slice\/concat\/wireはbit参照\[さんしょう\]の配線\[はいせん\]になる。

式\[しき\]の入力順\[にゅうりょくじゅん\]、bit順\[じゅん\]、生成\[せいせい\]node ordinalを固定\[こてい\]し、同\[おな\]じ入力\[にゅうりょく\]から同\[おな\]じIRを出\[だ\]す。任意最適化\[にんいさいてきか\]をreference loweringへ混在\[こんざい\]させない。最適化\[さいてきか\]を提供\[ていきょう\]する場合\[ばあい\]は別\[べつ\]operationとして同値\[どうち\]テストとsource mapを要求\[ようきゅう\]する。

比較試験\[ひかくしけん\]は元\[もと\]のvector evaluatorとNOR evaluatorの両方\[りょうほう\]で、全小幅入力\[ぜんしょうはばにゅうりょく\]・複数\[ふくすう\]tickを比較\[ひかく\]する。NOR evaluatorを元\[もと\]のevaluatorと同\[おな\]じコードへの単\[たん\]なるaliasにしない。

<a name="n-737667"></a>

<a name="7-svg-backend"></a>

## 7\. SVG backendの配置\[はいち\]

elaborated vector\-level graphを対象\[たいしょう\]に、入力\[にゅうりょく\]\/stateをrank0、各組合\[かくくみあわ\]せnodeを前提\[ぜんてい\]nodeの最大\[さいだい\]rank\+1、出力\[しゅつりょく\]を最後\[さいご\]に配置\[はいち\]する。同\[どう\]rank内\[ない\]は定義\[ていぎ\]\/instance ordinal順\[じゅん\]。固定\[こてい\]paddingとlabelのscalar数\[すう\]から幅\[はば\]を決\[き\]め、SVG textLengthを使\[つか\]って描画幅\[びょうがはば\]を明示\[めいじ\]する。font依存\[いぞん\]でSVG byte列\[れつ\]を変\[か\]えない。

edgeはrank間\[かん\]のlaneをordinalで割\[わ\]り当\[あ\]てた直交線\[ちょっこうせん\]。stateへのnext edgeは破線\[はせん\]で表示\[ひょうじ\]し、組合\[くみあわ\]せedgeと区別\[くべつ\]する。node\/edgeにはEntityIdとOriginを対応付\[たいおうづ\]け、クリックでソースに戻\[もど\]せる。最小交差数\[さいしょうこうさすう\]や最適配置\[さいてきはいち\]を保証\[ほしょう\]する算法\[さんぽう\]とはしない。

全\[ぜん\]ラベルはescapeし、任意\[にんい\]scriptやURLを挿入\[そうにゅう\]しない。

<a name="n-7075626c6963"></a>

<a name="8-公開操作"></a>

## 8\. 公開操作\[こうかいそうさ\]

lower、check、elaborate、initial、observe、step、run\_tests、lower\_nor、print、diagram。CheckedDesignは必要\[ひつよう\]なmodule検査\[けんさ\]を含\[ふく\]む。PreparedNetlistはinstance展開\[てんかい\]と組合\[くみあわ\]せDAG\/幅検査済\[はばけんさず\]み。sourceをparseしただけでsimulationを開始\[かいし\]しない。
