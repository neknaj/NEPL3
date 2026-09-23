<!-- Generated from doc/tutorial/composition.nepld; renderer nepl3-tools.markdown-annotated/4; source SHA-256 71094fb117763a223c36295a9240e716942d4d83640cf385cb55c87ad4ee8340; alias input SHA-256 37517e5f3dc66819f61f5a7bb8ace1921282415f10551d2defa5c3eb0985b570. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

# <ruby>独立<rt>どくりつ</rt></ruby>した<ruby>二言語<rt>にげんご</rt></ruby>を<ruby>組<rt>く</rt></ruby>み<ruby>合<rt>あ</rt></ruby>わせる

[正本（NEPL3d）](<composition.nepld>)

<a name="n-707572706f7365"></a>

## この<ruby>章<rt>しょう</rt></ruby>の<ruby>到達点<rt>とうたつてん</rt></ruby>

compositionは、<ruby>複数<rt>ふくすう</rt></ruby>のLanguagePackageが<ruby>定義<rt>ていぎ</rt></ruby>する<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>接続<rt>せつぞく</rt></ruby>する<ruby>構成<rt>こうせい</rt></ruby>である。この<ruby>章<rt>しょう</rt></ruby>では、MiniExprからFrameへ<ruby>入<rt>はい</rt></ruby>り、Frameの<ruby>内部<rt>ないぶ</rt></ruby>でMiniExprへ<ruby>再入<rt>さいにゅう</rt></ruby>する<ruby>入力<rt>にゅうりょく</rt></ruby>を<ruby>解析<rt>かいせき</rt></ruby>する。<ruby>使用<rt>しよう</rt></ruby>するpackageとaliasはhostがParseProfileへ<ruby>静的<rt>せいてき</rt></ruby>に<ruby>登録<rt>とうろく</rt></ruby>する。<ruby>前章<rt>ぜんしょう</rt></ruby>の<ruby>演習<rt>えんしゅう</rt></ruby>によるsumへの<ruby>変更<rt>へんこう</rt></ruby>は<ruby>保存<rt>ほぞん</rt></ruby>し、この<ruby>章<rt>しょう</rt></ruby>では<ruby>標準<rt>ひょうじゅん</rt></ruby>のadd<ruby>定義<rt>ていぎ</rt></ruby>を<ruby>使用<rt>しよう</rt></ruby>する。source<ruby>上<rt>じょう</rt></ruby>のimportとSentence・annotationとの<ruby>統合<rt>とうごう</rt></ruby>は<ruby>後続<rt>こうぞく</rt></ruby>の<ruby>実装対象<rt>じっそうたいしょう</rt></ruby>である。suite consumerは、<ruby>同<rt>おな</rt></ruby>じ<ruby>言語構成<rt>げんごこうせい</rt></ruby>をnativeのInvoke・Await・Resumeへ<ruby>接続<rt>せつぞく</rt></ruby>して<ruby>算術評価<rt>さんじゅつひょうか</rt></ruby>を<ruby>実行<rt>じっこう</rt></ruby>する。[suite consumerの<ruby>実行手順<rt>じっこうてじゅん</rt></ruby>](<https\:\/\/github\.com\/neknaj\/NEPL3\/blob\/main\/conformance\/extensions\/suite\/README\.md>)

<a name="n-6f627365727665"></a>

## <ruby>言語境界<rt>げんごきょうかい</rt></ruby>と<ruby>再入<rt>さいにゅう</rt></ruby>を<ruby>観察<rt>かんさつ</rt></ruby>する

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example composition -- "add framed frame neg 7 2"
```

<ruby>出力<rt>しゅつりょく</rt></ruby>はComplete\; cursor\=24となり、bundleは<ruby>三<rt>みっ</rt></ruby>つのcontextを<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>外側<rt>そとがわ</rt></ruby>のaddは<ruby>二<rt>ふた</rt></ruby>つのExprを<ruby>読<rt>よ</rt></ruby>む。<ruby>最初<rt>さいしょ</rt></ruby>のExprにあるframedはFrameへ<ruby>移行<rt>いこう</rt></ruby>し、frameは<ruby>内部<rt>ないぶ</rt></ruby>のExprとしてneg 7を<ruby>読<rt>よ</rt></ruby>む。<ruby>最後<rt>さいご</rt></ruby>の2は<ruby>外側<rt>そとがわ</rt></ruby>のaddの<ruby>二番目<rt>にばんめ</rt></ruby>の<ruby>子<rt>こ</rt></ruby>となる。

```text
MiniExpr: add
├── MiniExpr: framed
│   └── Frame: frame
│       └── MiniExpr: neg 7
└── MiniExpr: 2
```

<ruby>図<rt>ず</rt></ruby>は<ruby>各部分<rt>かくぶぶん</rt></ruby>を<ruby>担当<rt>たんとう</rt></ruby>する<ruby>言語<rt>げんご</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>す。7のtokenは<ruby>元<rt>もと</rt></ruby>のsourceのbyte<ruby>範囲<rt>はんい</rt></ruby>21\.\.22を<ruby>保持<rt>ほじ</rt></ruby>する。source printの<ruby>結果<rt>けっか</rt></ruby>は<ruby>入力<rt>にゅうりょく</rt></ruby>と<ruby>一致<rt>いっち</rt></ruby>する。

<a name="n-646566696e6974696f6e"></a>

## packageとcontextの<ruby>対応<rt>たいおう</rt></ruby>

conformance\/extensions\/hello\/src\/composition\.rsは、<ruby>拡張<rt>かくちょう</rt></ruby>したMiniExprとFrameを<ruby>登録<rt>とうろく</rt></ruby>する。<ruby>拡張版<rt>かくちょうばん</rt></ruby>のschemaはorg\.example\.miniexpr\.framedであり、<ruby>単独版<rt>たんどくばん</rt></ruby>のorg\.example\.miniexprは<ruby>独立<rt>どくりつ</rt></ruby>して<ruby>維持<rt>いじ</rt></ruby>される。ReadSpec\:\:Foreignは<ruby>外部<rt>がいぶ</rt></ruby>packageのcategoryを<ruby>指定<rt>してい</rt></ruby>し、hostはExprとFrameというaliasを<ruby>解決<rt>かいけつ</rt></ruby>する。ExprはCode、FrameはFrameCodeというreader modeを<ruby>使用<rt>しよう</rt></ruby>する。contextのpathは<ruby>言語境界<rt>げんごきょうかい</rt></ruby>を<ruby>記録<rt>きろく</rt></ruby>し、<ruby>内部<rt>ないぶ</rt></ruby>の<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>担当<rt>たんとう</rt></ruby>するpackageと<ruby>読取条件<rt>よみとりじょうけん</rt></ruby>を<ruby>識別<rt>しきべつ</rt></ruby>する。

<a name="n-646961676e6f7374696373"></a>

## <ruby>内部<rt>ないぶ</rt></ruby>の<ruby>誤入力<rt>ごにゅうりょく</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example composition -- "framed unknown"
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example composition -- "framed frame"
```

<ruby>一番目<rt>いちばんめ</rt></ruby>の<ruby>入力<rt>にゅうりょく</rt></ruby>はFrameの<ruby>未知<rt>みち</rt></ruby>のhead、<ruby>二番目<rt>にばんめ</rt></ruby>は<ruby>内部<rt>ないぶ</rt></ruby>のExprの<ruby>不足<rt>ふそく</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>両方<rt>りょうほう</rt></ruby>ともRecoveredと<ruby>診断<rt>しんだん</rt></ruby>を<ruby>返<rt>かえ</rt></ruby>し、<ruby>成功<rt>せいこう</rt></ruby>したsource printは<ruby>出力<rt>しゅつりょく</rt></ruby>されない。<ruby>診断<rt>しんだん</rt></ruby>のsource<ruby>範囲<rt>はんい</rt></ruby>を<ruby>入力<rt>にゅうりょく</rt></ruby>へ<ruby>対応<rt>たいおう</rt></ruby>させ、<ruby>問題<rt>もんだい</rt></ruby>が<ruby>発生<rt>はっせい</rt></ruby>した<ruby>言語<rt>げんご</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。exampleの<ruby>終了<rt>しゅうりょう</rt></ruby>コード0は<ruby>観察処理<rt>かんさつしょり</rt></ruby>の<ruby>完了<rt>かんりょう</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>し、<ruby>解析状態<rt>かいせきじょうたい</rt></ruby>は<ruby>表示<rt>ひょうじ</rt></ruby>されたoutcomeで<ruby>判断<rt>はんだん</rt></ruby>する。

<a name="n-6578657263697365"></a>

## <ruby>演習<rt>えんしゅう</rt></ruby>と<ruby>交換試験<rt>こうかんしけん</rt></ruby>

<ruby>正常入力<rt>せいじょうにゅうりょく</rt></ruby>のneg 7をneg add 1 2へ<ruby>変更<rt>へんこう</rt></ruby>し、<ruby>内側<rt>うちがわ</rt></ruby>のMiniExprの<ruby>構文<rt>こうぶん</rt></ruby>とsource<ruby>範囲<rt>はんい</rt></ruby>を<ruby>確認<rt>かくにん</rt></ruby>する。<ruby>各<rt>かく</rt></ruby>nodeについて、<ruby>所属言語<rt>しょぞくげんご</rt></ruby>と<ruby>外側<rt>そとがわ</rt></ruby>の<ruby>親<rt>おや</rt></ruby>を<ruby>説明<rt>せつめい</rt></ruby>する。

```sh
cargo test --locked --manifest-path conformance/extensions/hello/Cargo.toml
```

<ruby>既存<rt>きそん</rt></ruby>の<ruby>試験<rt>しけん</rt></ruby>は、packageとProfileをNDFで<ruby>交換<rt>こうかん</rt></ruby>し、<ruby>受信側<rt>じゅしんがわ</rt></ruby>のcatalogとschemaに<ruby>照合<rt>しょうごう</rt></ruby>して<ruby>解析<rt>かいせき</rt></ruby>する。native<ruby>登録経路<rt>とうろくけいろ</rt></ruby>との<ruby>比較<rt>ひかく</rt></ruby>には、<ruby>再入<rt>さいにゅう</rt></ruby>、Unicodeを<ruby>含<rt>ふく</rt></ruby>む<ruby>未知<rt>みち</rt></ruby>のhead、<ruby>子<rt>こ</rt></ruby>の<ruby>不足<rt>ふそく</rt></ruby>、<ruby>診断<rt>しんだん</rt></ruby>とsource<ruby>位置<rt>いち</rt></ruby>を<ruby>含<rt>ふく</rt></ruby>む。この<ruby>章<rt>しょう</rt></ruby>の<ruby>完了条件<rt>かんりょうじょうけん</rt></ruby>は、<ruby>言語境界<rt>げんごきょうかい</rt></ruby>、<ruby>再入<rt>さいにゅう</rt></ruby>、<ruby>内部<rt>ないぶ</rt></ruby>の<ruby>診断<rt>しんだん</rt></ruby>を<ruby>元<rt>もと</rt></ruby>の<ruby>入力<rt>にゅうりょく</rt></ruby>へ<ruby>対応<rt>たいおう</rt></ruby>させられることである。
