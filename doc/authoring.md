<!-- Generated from doc/authoring.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page authoring; source SHA-256 f552023478dfb8ca8d5945521977a021c76f51c12aaede1d6f1047924aad2ac7; alias input SHA-256 e304d5b38c1d6d48588115a004c7bc8e4851a4c20e8411ef75893d68096aa795; page input SHA-256 b9541b5718d91f66658bf1797eee775a32b5471ed988310665a2f08fa932d482. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="doc文書の執筆指針"></a>

# Doc<ruby>文書<rt>ぶんしょ</rt></ruby>の<ruby>執筆指針<rt>しっぴつししん</rt></ruby>

[正本（NEPL3d）](<authoring.nepld>)

この<ruby>指針<rt>ししん</rt></ruby>は、NEPL3の<ruby>例<rt>れい</rt></ruby>・<ruby>仕様<rt>しよう</rt></ruby>・<ruby>解説<rt>かいせつ</rt></ruby>をDocで<ruby>執筆<rt>しっぴつ</rt></ruby>する<ruby>際<rt>さい</rt></ruby>の<ruby>表記選択<rt>ひょうきせんたく</rt></ruby>と<ruby>確認手順<rt>かくにんてじゅん</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。<ruby>構文<rt>こうぶん</rt></ruby>と<ruby>意味<rt>いみ</rt></ruby>は[Doc<ruby>仕様<rt>しよう</rt></ruby>](<spec\/05\-document\.md>)、<ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>の<ruby>正本切替<rt>せいほんきりか</rt></ruby>えは[<ruby>移行条件<rt>いこうじょうけん</rt></ruby>](<spec\/16\-doc\-migration\.md>)を<ruby>参照<rt>さんしょう</rt></ruby>する。

<a name="n-6c69746572616c"></a>

<a name="sentence-literalを標準にする場面"></a>

## Sentence literalを<ruby>標準<rt>ひょうじゅん</rt></ruby>にする<ruby>場面<rt>ばめん</rt></ruby>

Sentence literalは、Text・Ruby・Annoからなる<ruby>文章<rt>ぶんしょう</rt></ruby>を<ruby>引用符内<rt>いんようふない</rt></ruby>に<ruby>記述<rt>きじゅつ</rt></ruby>する<ruby>表記<rt>ひょうき</rt></ruby>である。<ruby>通常<rt>つうじょう</rt></ruby>の<ruby>説明文<rt>せつめいぶん</rt></ruby>、<ruby>見出<rt>みだ</rt></ruby>し、<ruby>表<rt>ひょう</rt></ruby>のセル、<ruby>対応文<rt>たいおうぶん</rt></ruby>のvariantでは、この<ruby>表記<rt>ひょうき</rt></ruby>を<ruby>基本<rt>きほん</rt></ruby>とする。Ruby・Annoの<ruby>入<rt>い</rt></ruby>れ<ruby>子<rt>こ</rt></ruby>も<ruby>表現<rt>ひょうげん</rt></ruby>できる。

```text
"これは{[文書/ぶんしょ]/document}の[例/れい]です。"
```

<ruby>一<rt>ひと</rt></ruby>つのliteralは<ruby>一<rt>ひと</rt></ruby>つのSentenceを<ruby>表<rt>あらわ</rt></ruby>す。<ruby>本文<rt>ほんぶん</rt></ruby>では、<ruby>著者<rt>ちょしゃ</rt></ruby>が<ruby>対応<rt>たいおう</rt></ruby>させる<ruby>文<rt>ぶん</rt></ruby>ごとにliteralを<ruby>記述<rt>きじゅつ</rt></ruby>する。<ruby>見出<rt>みだ</rt></ruby>しや<ruby>表<rt>ひょう</rt></ruby>のセルでは、<ruby>短<rt>みじか</rt></ruby>い<ruby>語句<rt>ごく</rt></ruby>も<ruby>一<rt>ひと</rt></ruby>つのSentenceとして<ruby>扱<rt>あつか</rt></ruby>える。Sentenceの<ruby>境界<rt>きょうかい</rt></ruby>は<ruby>著者<rt>ちょしゃ</rt></ruby>が<ruby>指定<rt>してい</rt></ruby>し、literal<ruby>内<rt>ない</rt></ruby>の<ruby>句点<rt>くてん</rt></ruby>は<ruby>本文<rt>ほんぶん</rt></ruby>の<ruby>一部<rt>いちぶ</rt></ruby>として<ruby>扱<rt>あつか</rt></ruby>う。

literalはソース<ruby>上<rt>じょう</rt></ruby>の<ruby>一行<rt>いちぎょう</rt></ruby>に<ruby>記述<rt>きじゅつ</rt></ruby>する。<ruby>引用符<rt>いんようふ</rt></ruby>は `\"`、バックスラッシュは `\\`、<ruby>文字<rt>もじ</rt></ruby>としての<ruby>注釈区切<rt>ちゅうしゃくくぎ</rt></ruby>りは `\[` などでescapeする。`\n`はTextのデータとしてLFを<ruby>保持<rt>ほじ</rt></ruby>するescapeである。

<ruby>文書構造<rt>ぶんしょこうぞう</rt></ruby>として<ruby>指定<rt>してい</rt></ruby>する<ruby>改行<rt>かいぎょう</rt></ruby>には、<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>の[`break`](<authoring\.md\#n\-627265616b73>)を<ruby>使<rt>つか</rt></ruby>う。<ruby>画面幅<rt>がめんはば</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じた<ruby>折返<rt>おりかえ</rt></ruby>しは<ruby>表示側<rt>ひょうじがわ</rt></ruby>が<ruby>処理<rt>しょり</rt></ruby>する。<ruby>長<rt>なが</rt></ruby>い<ruby>文<rt>ぶん</rt></ruby>も<ruby>著者<rt>ちょしゃ</rt></ruby>が<ruby>指定<rt>してい</rt></ruby>した<ruby>対応単位<rt>たいおうたんい</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>し、<ruby>編集上<rt>へんしゅうじょう</rt></ruby>の<ruby>都合<rt>つごう</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じて<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>を<ruby>選択<rt>せんたく</rt></ruby>する。

<a name="n-6578706c69636974"></a>

<a name="明示的なsentence構築を選ぶ場面"></a>

## <ruby>明示的<rt>めいじてき</rt></ruby>なsentence<ruby>構築<rt>こうちく</rt></ruby>を<ruby>選<rt>えら</rt></ruby>ぶ<ruby>場面<rt>ばめん</rt></ruby>

<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>は、`sentence`の<ruby>子<rt>こ</rt></ruby>としてInlineを<ruby>列挙<rt>れっきょ</rt></ruby>する<ruby>表記<rt>ひょうき</rt></ruby>である。<ruby>強調<rt>きょうちょう</rt></ruby>の`strong`・`em`、<ruby>参照<rt>さんしょう</rt></ruby>の`ref`・`anchor`・`link`、Inline<ruby>位置<rt>いち</rt></ruby>の`code`、<ruby>画像<rt>がぞう</rt></ruby>、Math、<ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>改行<rt>かいぎょう</rt></ruby>の`break`を<ruby>含<rt>ふく</rt></ruby>むSentenceに<ruby>使用<rt>しよう</rt></ruby>する。

```text
sentence
  cons anno ruby text "係数" text "けいすう" cons text "coefficient" nil
  cons text "の"
  cons strong text "すべて"
  cons text "の"
  cons ruby text "可能性" text "かのうせい"
  cons text "を"
  cons ruby text "考" text "かんが"
  cons text "えます。"
  nil
```

<ruby>複雑<rt>ふくざつ</rt></ruby>な<ruby>注釈<rt>ちゅうしゃく</rt></ruby>の<ruby>各部分<rt>かくぶぶん</rt></ruby>を<ruby>編集<rt>へんしゅう</rt></ruby>・<ruby>比較<rt>ひかく</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>や、constructor API・<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>解説<rt>かいせつ</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>にも<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>を<ruby>使用<rt>しよう</rt></ruby>できる。<ruby>通常<rt>つうじょう</rt></ruby>の<ruby>本文<rt>ほんぶん</rt></ruby>は、<ruby>含<rt>ふく</rt></ruby>まれるInlineと<ruby>編集目的<rt>へんしゅうもくてき</rt></ruby>に<ruby>応<rt>おう</rt></ruby>じて<ruby>表記<rt>ひょうき</rt></ruby>を<ruby>選択<rt>せんたく</rt></ruby>する。

プログラムから<ruby>文書<rt>ぶんしょ</rt></ruby>を<ruby>生成<rt>せいせい</rt></ruby>する<ruby>通常<rt>つうじょう</rt></ruby>の<ruby>経路<rt>けいろ</rt></ruby>では、<ruby>型付<rt>かたつ</rt></ruby>きconstructorで<ruby>意味構造<rt>いみこうぞう</rt></ruby>を<ruby>構築<rt>こうちく</rt></ruby>し、<ruby>既存<rt>きそん</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>・printerへ<ruby>渡<rt>わた</rt></ruby>す。ソース<ruby>文字列<rt>もじれつ</rt></ruby>の<ruby>出力<rt>しゅつりょく</rt></ruby>はprinterが<ruby>担当<rt>たんとう</rt></ruby>する。

`text "..."` の<ruby>引用部分<rt>いんようぶぶん</rt></ruby>は<ruby>通常<rt>つうじょう</rt></ruby>のText<ruby>値<rt>あたい</rt></ruby>として<ruby>保持<rt>ほじ</rt></ruby>される。Ruby・Annoは、それぞれのconstructorで<ruby>構築<rt>こうちく</rt></ruby>する。Sentence literalのcategoryはSentenceであり、`sentence`の<ruby>子<rt>こ</rt></ruby>に<ruby>使用<rt>しよう</rt></ruby>できるcategoryはInlineである。

<a name="n-627265616b73"></a>

<a name="明示的な改行はbreak"></a>

## <ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>改行<rt>かいぎょう</rt></ruby>はbreak

`break`は<ruby>引数<rt>ひきすう</rt></ruby>を<ruby>持<rt>も</rt></ruby>たないInline constructorであり、<ruby>同<rt>おな</rt></ruby>じSentence<ruby>内<rt>ない</rt></ruby>の<ruby>改行位置<rt>かいぎょういち</rt></ruby>を<ruby>指定<rt>してい</rt></ruby>する。HTMLでは`br`、plain text<ruby>抽出<rt>ちゅうしゅつ</rt></ruby>ではLFとして<ruby>出力<rt>しゅつりょく</rt></ruby>される。Sentenceの<ruby>対応単位<rt>たいおうたんい</rt></ruby>は<ruby>保持<rt>ほじ</rt></ruby>される。<ruby>段落<rt>だんらく</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>はparagraphで<ruby>表<rt>あらわ</rt></ruby>す。<ruby>空<rt>から</rt></ruby>の<ruby>段落<rt>だんらく</rt></ruby>や<ruby>複数<rt>ふくすう</rt></ruby>のbreakによる<ruby>余白調整<rt>よはくちょうせい</rt></ruby>は<ruby>禁止<rt>きんし</rt></ruby>する。

```text
sentence
  cons text "ここで"
  cons break
  cons ruby text "改行" text "かいぎょう"
  cons text "します。"
  nil
```

<ruby>既存<rt>きそん</rt></ruby>のText<ruby>内<rt>ない</rt></ruby>LFとRawCodeの<ruby>改行<rt>かいぎょう</rt></ruby>は、<ruby>元<rt>もと</rt></ruby>の<ruby>内容<rt>ないよう</rt></ruby>として<ruby>保持<rt>ほじ</rt></ruby>する。これらと`break`の<ruby>自動相互変換<rt>じどうそうごへんかん</rt></ruby>は<ruby>禁止<rt>きんし</rt></ruby>する。Compact printerは、breakを<ruby>含<rt>ふく</rt></ruby>むSentenceを<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>で<ruby>出力<rt>しゅつりょく</rt></ruby>する。[<ruby>改行<rt>かいぎょう</rt></ruby>の<ruby>例<rt>れい</rt></ruby>](<\.\.\/examples\/document\/line\-break\.nepld>)では、<ruby>日英<rt>にちえい</rt></ruby>の<ruby>対応文<rt>たいおうぶん</rt></ruby>それぞれにbreakを<ruby>配置<rt>はいち</rt></ruby>している。

<a name="n-706172616c6c656c"></a>

<a name="sentenceとparallelの使い分け"></a>

## sentenceとparallelの<ruby>使<rt>つか</rt></ruby>い<ruby>分<rt>わ</rt></ruby>け

`parallel`は、<ruby>意味<rt>いみ</rt></ruby>が<ruby>対応<rt>たいおう</rt></ruby>する<ruby>各言語<rt>かくげんご</rt></ruby>のSentenceをまとめる<ruby>構造<rt>こうぞう</rt></ruby>である。<ruby>本文<rt>ほんぶん</rt></ruby>では<ruby>文<rt>ぶん</rt></ruby>ごとに<ruby>作成<rt>さくせい</rt></ruby>し、<ruby>各<rt>かく</rt></ruby>`variant`へ<ruby>対応<rt>たいおう</rt></ruby>する<ruby>言語<rt>げんご</rt></ruby>のSentenceを<ruby>配置<rt>はいち</rt></ruby>する。<ruby>複数<rt>ふくすう</rt></ruby>の<ruby>文<rt>ぶん</rt></ruby>からなる<ruby>段落<rt>だんらく</rt></ruby>は、<ruby>文<rt>ぶん</rt></ruby>ごとのparallelを<ruby>並<rt>なら</rt></ruby>べて<ruby>表<rt>あらわ</rt></ruby>す。<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>語順<rt>ごじゅん</rt></ruby>と<ruby>語数<rt>ごすう</rt></ruby>は、その<ruby>言語<rt>げんご</rt></ruby>で<ruby>自然<rt>しぜん</rt></ruby>に<ruby>意味<rt>いみ</rt></ruby>を<ruby>表現<rt>ひょうげん</rt></ruby>するために<ruby>選択<rt>せんたく</rt></ruby>する。

```text
parallel
  cons variant ja "これは{[文書/ぶんしょ]/document}の[例/れい]です。"
  cons variant en "This is an example of a document."
  nil
```

<ruby>表記<rt>ひょうき</rt></ruby>はvariantごとに<ruby>選択<rt>せんたく</rt></ruby>する。<ruby>各言語<rt>かくげんご</rt></ruby>で<ruby>必要<rt>ひつよう</rt></ruby>なInlineに<ruby>応<rt>おう</rt></ruby>じて、literalと<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>を<ruby>組<rt>く</rt></ruby>み<ruby>合<rt>あ</rt></ruby>わせられる。<ruby>強調<rt>きょうちょう</rt></ruby>や<ruby>参照<rt>さんしょう</rt></ruby>を<ruby>付<rt>つ</rt></ruby>ける<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>対応言語<rt>たいおうげんご</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>と<ruby>参照先<rt>さんしょうさき</rt></ruby>も<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>空<rt>から</rt></ruby>Sentenceは、<ruby>空<rt>から</rt></ruby>の<ruby>翻訳<rt>ほんやく</rt></ruby>を<ruby>意図<rt>いと</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>に<ruby>限<rt>かぎ</rt></ruby>って<ruby>使用<rt>しよう</rt></ruby>する。

[<ruby>線型結合<rt>せんけいけつごう</rt></ruby>の<ruby>例<rt>れい</rt></ruby>](<\.\.\/examples\/document\/linear\-combination\.nepld>)では、<ruby>最初<rt>さいしょ</rt></ruby>の<ruby>対応文<rt>たいおうぶん</rt></ruby>をliteral、<ruby>続<rt>つづ</rt></ruby>く<ruby>対応文<rt>たいおうぶん</rt></ruby>を<ruby>明示的<rt>めいじてき</rt></ruby>なsentence<ruby>構築<rt>こうちく</rt></ruby>で<ruby>記述<rt>きじゅつ</rt></ruby>する。<ruby>両表記<rt>りょうひょうき</rt></ruby>とも、<ruby>文単位<rt>ぶんたんい</rt></ruby>の<ruby>意味対応<rt>いみたいおう</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>する<ruby>教材<rt>きょうざい</rt></ruby>として<ruby>使用<rt>しよう</rt></ruby>する。

<a name="n-727562795f616e6e6f"></a>

<a name="rubyとannoの対象"></a>

## RubyとAnnoの<ruby>対象<rt>たいしょう</rt></ruby>

Rubyは<ruby>本文<rt>ほんぶん</rt></ruby>に<ruby>読<rt>よ</rt></ruby>みを<ruby>付与<rt>ふよ</rt></ruby>する<ruby>構造<rt>こうぞう</rt></ruby>である。<ruby>日本語<rt>にほんご</rt></ruby>の<ruby>振<rt>ふ</rt></ruby>り<ruby>仮名<rt>がな</rt></ruby>は[GlossのRuby<ruby>指針<rt>ししん</rt></ruby>](<https\:\/\/github\.com\/neknaj\/gloss\#ruby>)に<ruby>従<rt>したが</rt></ruby>い、<ruby>漢字部分<rt>かんじぶぶん</rt></ruby>に<ruby>付<rt>つ</rt></ruby>ける。<ruby>送<rt>おく</rt></ruby>り<ruby>仮名<rt>がな</rt></ruby>・<ruby>助詞<rt>じょし</rt></ruby>・カタカナ・<ruby>数字<rt>すうじ</rt></ruby>・<ruby>記号<rt>きごう</rt></ruby>は<ruby>通常<rt>つうじょう</rt></ruby>のTextとして<ruby>配置<rt>はいち</rt></ruby>する。<ruby>読<rt>よ</rt></ruby>みの<ruby>単位<rt>たんい</rt></ruby>は、<ruby>文脈<rt>ぶんみゃく</rt></ruby>に<ruby>合<rt>あ</rt></ruby>う<ruby>語<rt>ご</rt></ruby>または<ruby>漢字部分<rt>かんじぶぶん</rt></ruby>とする。

<ruby>語句全体<rt>ごくぜんたい</rt></ruby>の<ruby>訳語<rt>やくご</rt></ruby>・<ruby>意味説明<rt>いみせつめい</rt></ruby>はAnnoに<ruby>置<rt>お</rt></ruby>く。<ruby>例<rt>たと</rt></ruby>えば `{[原点/げんてん]を[通/とお]る[直線/ちょくせん]/a line through the origin}` とする。<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>ではAnnoのbaseをConcatで<ruby>組<rt>く</rt></ruby>み、その<ruby>子<rt>こ</rt></ruby>をRubyとTextに<ruby>分<rt>わ</rt></ruby>ける。カタカナ<ruby>語<rt>ご</rt></ruby>は `{ベクトル/vector}` のように<ruby>意味注釈<rt>いみちゅうしゃく</rt></ruby>を<ruby>付<rt>つ</rt></ruby>けられる。

<ruby>漢字部分<rt>かんじぶぶん</rt></ruby>への<ruby>振<rt>ふ</rt></ruby>り<ruby>仮名<rt>がな</rt></ruby>という<ruby>規則<rt>きそく</rt></ruby>は、<ruby>日本語文書<rt>にほんごぶんしょ</rt></ruby>の<ruby>執筆<rt>しっぴつ</rt></ruby>に<ruby>適用<rt>てきよう</rt></ruby>する。Rubyのparserとschemaは、<ruby>中国語<rt>ちゅうごくご</rt></ruby>のピンインや<ruby>他言語<rt>たげんご</rt></ruby>の<ruby>転写<rt>てんしゃ</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>む<ruby>一般的<rt>いっぱんてき</rt></ruby>な<ruby>音韻注釈<rt>おんいんちゅうしゃく</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う。

<a name="n-737472756374757265"></a>

<a name="構造と確認"></a>

## <ruby>構造<rt>こうぞう</rt></ruby>と<ruby>確認<rt>かくにん</rt></ruby>

<ruby>話題<rt>わだい</rt></ruby>のまとまりはsection、<ruby>連続<rt>れんぞく</rt></ruby>する<ruby>本文<rt>ほんぶん</rt></ruby>はparagraph、<ruby>列挙<rt>れっきょ</rt></ruby>はlist、<ruby>同<rt>おな</rt></ruby>じ<ruby>項目<rt>こうもく</rt></ruby>を<ruby>比較<rt>ひかく</rt></ruby>する<ruby>情報<rt>じょうほう</rt></ruby>はtableで<ruby>表<rt>あらわ</rt></ruby>す。<ruby>参照<rt>さんしょう</rt></ruby>には<ruby>安定<rt>あんてい</rt></ruby>したsection\/anchor IDを<ruby>使<rt>つか</rt></ruby>う。<ruby>装飾<rt>そうしょく</rt></ruby>・<ruby>表<rt>ひょう</rt></ruby>・<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みは、<ruby>読者<rt>どくしゃ</rt></ruby>が<ruby>内容<rt>ないよう</rt></ruby>を<ruby>理解<rt>りかい</rt></ruby>するために<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>箇所<rt>かしょ</rt></ruby>へ<ruby>配置<rt>はいち</rt></ruby>する。

<ruby>表記変更後<rt>ひょうきへんこうご</rt></ruby>は、<ruby>本文<rt>ほんぶん</rt></ruby>・<ruby>読<rt>よ</rt></ruby>み・<ruby>注釈<rt>ちゅうしゃく</rt></ruby>・<ruby>対応文<rt>たいおうぶん</rt></ruby>・<ruby>強調<rt>きょうちょう</rt></ruby>・<ruby>参照<rt>さんしょう</rt></ruby>の<ruby>保持<rt>ほじ</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。literalと<ruby>前置構築<rt>ぜんちこうちく</rt></ruby>の<ruby>等価性<rt>とうかせい</rt></ruby>は<ruby>意味構造<rt>いみこうぞう</rt></ruby>で<ruby>判定<rt>はんてい</rt></ruby>する。ソース<ruby>位置<rt>いち</rt></ruby>は<ruby>各表記<rt>かくひょうき</rt></ruby>のsourceに<ruby>属<rt>ぞく</rt></ruby>し、<ruby>変更後<rt>へんこうご</rt></ruby>の<ruby>位置<rt>いち</rt></ruby>を<ruby>個別<rt>こべつ</rt></ruby>に<ruby>確認<rt>かくにん</rt></ruby>する。

<ruby>構造<rt>こうぞう</rt></ruby>audit、<ruby>正式<rt>せいしき</rt></ruby>なparser、lower、HTML<ruby>生成<rt>せいせい</rt></ruby>は、それぞれの<ruby>検証結果<rt>けんしょうけっか</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>する。<ruby>資源停止<rt>しげんていし</rt></ruby>や<ruby>未対応機能<rt>みたいおうきのう</rt></ruby>が<ruby>残<rt>のこ</rt></ruby>る<ruby>例<rt>れい</rt></ruby>には、その<ruby>制約<rt>せいやく</rt></ruby>を<ruby>明記<rt>めいき</rt></ruby>する。<ruby>実行可能性<rt>じっこうかのうせい</rt></ruby>の<ruby>説明<rt>せつめい</rt></ruby>は、<ruby>実際<rt>じっさい</rt></ruby>に<ruby>成功<rt>せいこう</rt></ruby>した<ruby>処理範囲<rt>しょりはんい</rt></ruby>に<ruby>限定<rt>げんてい</rt></ruby>する。

RustのDoc APIで<ruby>既存部分<rt>きそんぶぶん</rt></ruby>を<ruby>組<rt>く</rt></ruby>み<ruby>替<rt>か</rt></ruby>える<ruby>場合<rt>ばあい</rt></ruby>は、<ruby>正式<rt>せいしき</rt></ruby>にlowerした`DocumentSyntax`から<ruby>親子参照<rt>おやこさんしょう</rt></ruby>を<ruby>辿<rt>たど</rt></ruby>って<ruby>対象<rt>たいしょう</rt></ruby>を<ruby>選<rt>えら</rt></ruby>び、`fragment(DocRoot, registry, budget, admission)`で<ruby>抽出<rt>ちゅうしゅつ</rt></ruby>する。<ruby>抽出<rt>ちゅうしゅつ</rt></ruby>は<ruby>元文書全体<rt>もとぶんしょぜんたい</rt></ruby>を<ruby>検証<rt>けんしょう</rt></ruby>し、<ruby>到達可能<rt>とうたつかのう</rt></ruby>なnodeとembedを<ruby>再配置<rt>さいはいち</rt></ruby>する。<ruby>共有参照<rt>きょうゆうさんしょう</rt></ruby>とForeignClosureを<ruby>保持<rt>ほじ</rt></ruby>し、Source・Origin・View・source mapは<ruby>元<rt>もと</rt></ruby>identityのまま<ruby>保持<rt>ほじ</rt></ruby>する。このため、<ruby>費用<rt>ひよう</rt></ruby>と<ruby>保持<rt>ほじ</rt></ruby>するsource<ruby>集合<rt>しゅうごう</rt></ruby>は<ruby>元文書全体<rt>もとぶんしょぜんたい</rt></ruby>に<ruby>依存<rt>いぞん</rt></ruby>する。

<ruby>抽出後<rt>ちゅうしゅつご</rt></ruby>に<ruby>型付<rt>かたつ</rt></ruby>きconstructorでListItemなどを<ruby>構築<rt>こうちく</rt></ruby>し、<ruby>既存<rt>きそん</rt></ruby>printerへ<ruby>渡<rt>わた</rt></ruby>す。sourceの<ruby>局所編集<rt>きょくしょへんしゅう</rt></ruby>には、<ruby>元<rt>もと</rt></ruby>モデルが<ruby>示<rt>しめ</rt></ruby>すSpanと<ruby>元<rt>もと</rt></ruby>byte<ruby>列<rt>れつ</rt></ruby>のdigestを<ruby>持<rt>も</rt></ruby>つ`TextEdit`を`SourceStore::apply`へ<ruby>渡<rt>わた</rt></ruby>す。<ruby>変更後<rt>へんこうご</rt></ruby>の<ruby>位置<rt>いち</rt></ruby>は<ruby>新<rt>あたら</rt></ruby>しいsnapshotの<ruby>再<rt>さい</rt></ruby>parseで<ruby>確定<rt>かくてい</rt></ruby>し、<ruby>参照解決<rt>さんしょうかいけつ</rt></ruby>やprepareも<ruby>改<rt>あらた</rt></ruby>めて<ruby>行<rt>おこな</rt></ruby>う。<ruby>実行例<rt>じっこうれい</rt></ruby>は[Doc printer<ruby>統合試験<rt>とうごうしけん</rt></ruby>](<\.\.\/tools\/tests\/doc\/print\.rs>)の`paragraph_edit_uses_model_span_and_preserves_surrounding_source`にある。

<a name="n-6c696e6b73"></a>

<a name="urlと参照"></a>

## URLと<ruby>参照<rt>さんしょう</rt></ruby>

<ruby>外部<rt>がいぶ</rt></ruby>URLはDocの<ruby>明示的<rt>めいじてき</rt></ruby>なLinkとして<ruby>記述<rt>きじゅつ</rt></ruby>する。literal<ruby>内<rt>ない</rt></ruby>のURLは<ruby>通常<rt>つうじょう</rt></ruby>のTextであり、<ruby>自動<rt>じどう</rt></ruby>リンク<ruby>化<rt>か</rt></ruby>しない。

```text
sentence cons link external "https://example.org/docs?a=1&b=2" text "参考資料" nil
```

<ruby>通常<rt>つうじょう</rt></ruby>のHTML exportとSVG<ruby>付<rt>つ</rt></ruby>きHTML exportで<ruby>利用<rt>りよう</rt></ruby>でき、labelにはRuby・AnnoなどのInlineを<ruby>置<rt>お</rt></ruby>ける。http・https・mailtoの<ruby>共通<rt>きょうつう</rt></ruby>URI<ruby>規則<rt>きそく</rt></ruby>に<ruby>従<rt>したが</rt></ruby>い、<ruby>危険<rt>きけん</rt></ruby>なschemeや<ruby>不正<rt>ふせい</rt></ruby>なescapeは<ruby>生成前<rt>せいせいまえ</rt></ruby>に<ruby>拒否<rt>きょひ</rt></ruby>する。URLを<ruby>取得<rt>しゅとく</rt></ruby>したり、<ruby>参照先<rt>さんしょうさき</rt></ruby>の<ruby>閲覧権限<rt>えつらんけんげん</rt></ruby>や<ruby>公開可否<rt>こうかいかひ</rt></ruby>を<ruby>保証<rt>ほしょう</rt></ruby>したりする<ruby>機能<rt>きのう</rt></ruby>ではない。

<ruby>同一文書内<rt>どういつぶんしょない</rt></ruby>では `ref introduction text "はじめに"` の<ruby>参照先<rt>さんしょうさき</rt></ruby>を `section introduction` または `anchor introduction` で<ruby>定義<rt>ていぎ</rt></ruby>する。<ruby>文書間参照<rt>ぶんしょかんさんしょう</rt></ruby>は `link page "guide" some "usage" text "使い方"`、<ruby>相対参照<rt>そうたいさんしょう</rt></ruby>は `link relative "../guide.nepld" some "usage" text "使い方"` を<ruby>用<rt>もち</rt></ruby>い、<ruby>登録済<rt>とうろくず</rt></ruby>みPageSetから<ruby>生成<rt>せいせい</rt></ruby>する。<ruby>未登録<rt>みとうろく</rt></ruby>の<ruby>文書<rt>ぶんしょ</rt></ruby>や<ruby>存在<rt>そんざい</rt></ruby>しないfragmentを<ruby>推測<rt>すいそく</rt></ruby>して<ruby>成功<rt>せいこう</rt></ruby>にしない。

<ruby>独立<rt>どくりつ</rt></ruby>したSentence<ruby>言語<rt>げんご</rt></ruby>の<ruby>外部<rt>がいぶ</rt></ruby>リンク<ruby>構文<rt>こうぶん</rt></ruby>は `link "https://example.org/" text "参考資料"` であり、Docの `link external` と<ruby>引数<rt>ひきすう</rt></ruby>を<ruby>混同<rt>こんどう</rt></ruby>しない。

Markdownは<ruby>出力<rt>しゅつりょく</rt></ruby>profileごとに<ruby>対応範囲<rt>たいおうはんい</rt></ruby>が<ruby>異<rt>こと</rt></ruby>なる。annotated・footnotesの<ruby>外部<rt>がいぶ</rt></ruby>Linkは<ruby>既存対応<rt>きそんたいおう</rt></ruby>であり、plain MarkdownのLinkと<ruby>各<rt>かく</rt></ruby>Markdown profileのlocal ref\/anchorは<ruby>未対応<rt>みたいおう</rt></ruby>のため<ruby>明示<rt>めいじ</rt></ruby>エラーになる。HTMLでの<ruby>対応<rt>たいおう</rt></ruby>をMarkdown<ruby>全般<rt>ぜんぱん</rt></ruby>の<ruby>対応<rt>たいおう</rt></ruby>と<ruby>読<rt>よ</rt></ruby>み<ruby>替<rt>か</rt></ruby>えない。
