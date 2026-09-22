<!-- Generated from doc/tutorial/miniexpr.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page tutorial&#45;miniexpr; source SHA-256 e05a1b06f8a769f2fa5e6d8f9931125ee2e98a784f3466a462c93efc31557e5a; alias input SHA-256 37517e5f3dc66819f61f5a7bb8ace1921282415f10551d2defa5c3eb0985b570; page input SHA-256 e975d638bc80b348c54ebb1e60c9d2e8d17cc83ac90580d408f2e1f15b568802. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

# <ruby>再帰的<rt>さいきてき</rt></ruby>な<ruby>小言語<rt>しょうげんご</rt></ruby>を<ruby>定義<rt>ていぎ</rt></ruby>する

[正本（NEPL3d）](<miniexpr.nepld>)

<a name="n-707572706f7365"></a>

## この<ruby>章<rt>しょう</rt></ruby>の<ruby>到達点<rt>とうたつてん</rt></ruby>

MiniExprは、<ruby>自然数<rt>しぜんすう</rt></ruby>とneg・add・mulの<ruby>前置構文<rt>ぜんちこうぶん</rt></ruby>を<ruby>持<rt>も</rt></ruby>つ<ruby>教材用<rt>きょうざいよう</rt></ruby>の<ruby>言語<rt>げんご</rt></ruby>である。この<ruby>章<rt>しょう</rt></ruby>では、<ruby>入<rt>い</rt></ruby>れ<ruby>子<rt>こ</rt></ruby>の<ruby>式<rt>しき</rt></ruby>を<ruby>解析<rt>かいせき</rt></ruby>し、LanguagePackageの<ruby>宣言<rt>せんげん</rt></ruby>と<ruby>構文木<rt>こうぶんぎ</rt></ruby>を<ruby>対応<rt>たいおう</rt></ruby>させる。<ruby>実行環境<rt>じっこうかんきょう</rt></ruby>はHelloの<ruby>導入章<rt>どうにゅうしょう</rt></ruby>と<ruby>共通<rt>きょうつう</rt></ruby>であり、コマンドはリポジトリのrootで<ruby>実行<rt>じっこう</rt></ruby>する。

<a name="n-7061727365"></a>

## <ruby>式<rt>しき</rt></ruby>を<ruby>解析<rt>かいせき</rt></ruby>する

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example miniexpr -- "add 1 mul 2 3"
```

<ruby>出力<rt>しゅつりょく</rt></ruby>はComplete\; cursor\=13となり、Add・Natural・Mul・Natural・Naturalの5 nodeを<ruby>表示<rt>ひょうじ</rt></ruby>する。Addの<ruby>二<rt>ふた</rt></ruby>つの<ruby>子<rt>こ</rt></ruby>は1とMulであり、Mulの<ruby>子<rt>こ</rt></ruby>は2と3である。<ruby>数値<rt>すうち</rt></ruby>tokenのUTF\-8 byte<ruby>範囲<rt>はんい</rt></ruby>は、<ruby>順<rt>じゅん</rt></ruby>に4\.\.5、10\.\.11、12\.\.13となる。

```text
add
├── 1
└── mul
    ├── 2
    └── 3
```

この<ruby>図<rt>ず</rt></ruby>は<ruby>親子関係<rt>おやこかんけい</rt></ruby>の<ruby>説明用<rt>せつめいよう</rt></ruby>の<ruby>表記<rt>ひょうき</rt></ruby>である。<ruby>印字結果<rt>いんじけっか</rt></ruby>はsource print\: Complete\(\"add 1 mul 2 3\"\)となる。<ruby>構文検査<rt>こうぶんけんさ</rt></ruby>を<ruby>通過<rt>つうか</rt></ruby>した<ruby>木<rt>き</rt></ruby>をsource\-backed printerへ<ruby>渡<rt>わた</rt></ruby>し、<ruby>受理<rt>じゅり</rt></ruby>した<ruby>字句<rt>じく</rt></ruby>と<ruby>先行<rt>せんこう</rt></ruby>する<ruby>空白<rt>くうはく</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>して<ruby>出力<rt>しゅつりょく</rt></ruby>する。<ruby>算術評価<rt>さんじゅつひょうか</rt></ruby>と<ruby>意味的<rt>いみてき</rt></ruby>な<ruby>正規化<rt>せいきか</rt></ruby>は<ruby>後続<rt>こうぞく</rt></ruby>の<ruby>実装対象<rt>じっそうたいしょう</rt></ruby>である。

<a name="n-6465636c61726174696f6e"></a>

## <ruby>宣言<rt>せんげん</rt></ruby>と<ruby>子<rt>こ</rt></ruby>の<ruby>読取規則<rt>よみとりきそく</rt></ruby>

conformance\/extensions\/hello\/src\/miniexpr\.rsのlanguageは、LanguagePackageとSchemaRegistryを<ruby>構築<rt>こうちく</rt></ruby>する。categoryは<ruby>読取対象<rt>よみとりたいしょう</rt></ruby>の<ruby>構文<rt>こうぶん</rt></ruby>の<ruby>集合<rt>しゅうごう</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>し、この<ruby>例<rt>れい</rt></ruby>ではExprを<ruby>使用<rt>しよう</rt></ruby>する。formはheadの<ruby>表記<rt>ひょうき</rt></ruby>とfieldの<ruby>順序<rt>じゅんじょ</rt></ruby>を<ruby>定義<rt>ていぎ</rt></ruby>する。negは<ruby>一<rt>ひと</rt></ruby>つ、addとmulは<ruby>二<rt>ふた</rt></ruby>つのfieldを<ruby>持<rt>も</rt></ruby>ち、ReadSpec\:\:Localによって<ruby>同<rt>おな</rt></ruby>じpackageのExprを<ruby>再帰的<rt>さいきてき</rt></ruby>に<ruby>読<rt>よ</rt></ruby>む。NatとNameのreaderは、この<ruby>順序<rt>じゅんじょ</rt></ruby>で<ruby>数値<rt>すうち</rt></ruby>とheadを<ruby>識別<rt>しきべつ</rt></ruby>する。Natは<ruby>非負<rt>ひふ</rt></ruby>の<ruby>十進整数<rt>じっしんせいすう</rt></ruby>を<ruby>受理<rt>じゅり</rt></ruby>し、Integer payloadを<ruby>返<rt>かえ</rt></ruby>す。

<a name="n-6578657263697365"></a>

## <ruby>入力<rt>にゅうりょく</rt></ruby>と<ruby>宣言<rt>せんげん</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>する

<ruby>最初<rt>さいしょ</rt></ruby>に<ruby>入力<rt>にゅうりょく</rt></ruby>をneg add 1 2へ<ruby>変更<rt>へんこう</rt></ruby>する。Negの<ruby>子<rt>こ</rt></ruby>にAddが<ruby>配置<rt>はいち</rt></ruby>され、その<ruby>子<rt>こ</rt></ruby>に1と2が<ruby>配置<rt>はいち</rt></ruby>されることを<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>次<rt>つぎ</rt></ruby>にadd 1を<ruby>実行<rt>じっこう</rt></ruby>する。<ruby>入力終端<rt>にゅうりょくしゅうたん</rt></ruby>で<ruby>二番目<rt>にばんめ</rt></ruby>の<ruby>子<rt>こ</rt></ruby>が<ruby>不足<rt>ふそく</rt></ruby>し、Recoveredと<ruby>診断<rt>しんだん</rt></ruby>が<ruby>表示<rt>ひょうじ</rt></ruby>される。<ruby>成功<rt>せいこう</rt></ruby>したsource printは<ruby>出力<rt>しゅつりょく</rt></ruby>されない。

<ruby>宣言<rt>せんげん</rt></ruby>の<ruby>演習<rt>えんしゅう</rt></ruby>は<ruby>専用<rt>せんよう</rt></ruby>のbranchで<ruby>行<rt>おこな</rt></ruby>う。miniexpr\.rsのform<ruby>構築<rt>こうちく</rt></ruby>でaddのspellingをsumへ<ruby>変更<rt>へんこう</rt></ruby>し、<ruby>入力<rt>にゅうりょく</rt></ruby>sum 1 2を<ruby>実行<rt>じっこう</rt></ruby>する。<ruby>構文木<rt>こうぶんぎ</rt></ruby>のAddと<ruby>二<rt>ふた</rt></ruby>つのfieldは<ruby>維持<rt>いじ</rt></ruby>され、source printにはsumが<ruby>現<rt>あらわ</rt></ruby>れる。<ruby>標準<rt>ひょうじゅん</rt></ruby>の<ruby>言語<rt>げんご</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する<ruby>回帰試験<rt>かいきしけん</rt></ruby>の<ruby>期待値<rt>きたいち</rt></ruby>は<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>演習<rt>えんしゅう</rt></ruby>の<ruby>変更<rt>へんこう</rt></ruby>を<ruby>専用<rt>せんよう</rt></ruby>branchへcommitして<ruby>保存<rt>ほぞん</rt></ruby>し、<ruby>次章<rt>じしょう</rt></ruby>と<ruby>標準<rt>ひょうじゅん</rt></ruby>の<ruby>試験<rt>しけん</rt></ruby>はaddの<ruby>定義<rt>ていぎ</rt></ruby>を<ruby>持<rt>も</rt></ruby>つcheckoutで<ruby>実行<rt>じっこう</rt></ruby>する。この<ruby>章<rt>しょう</rt></ruby>の<ruby>完了条件<rt>かんりょうじょうけん</rt></ruby>は、headの<ruby>表記<rt>ひょうき</rt></ruby>、fieldの<ruby>順序<rt>じゅんじょ</rt></ruby>、<ruby>解析<rt>かいせき</rt></ruby>と<ruby>印字<rt>いんじ</rt></ruby>の<ruby>結果<rt>けっか</rt></ruby>を<ruby>説明<rt>せつめい</rt></ruby>できることである。

<a name="n-6e657874"></a>

## <ruby>次<rt>つぎ</rt></ruby>の<ruby>章<rt>しょう</rt></ruby>

[<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>二言語<rt>にげんご</rt></ruby>を<ruby>組<rt>く</rt></ruby>み<ruby>合<rt>あ</rt></ruby>わせる](<composition\.md>)
