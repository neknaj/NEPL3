<!-- Generated from doc/tutorial/hello.nepld; renderer nepl3-tools.markdown-annotated/3; source SHA-256 310aa046ddd9598b309c50d56e0c8239781330d79df8780a28266fc3e233adfa; alias input SHA-256 37517e5f3dc66819f61f5a7bb8ace1921282415f10551d2defa5c3eb0985b570; document digest 33cd149fde20796625f6188e0b077e1905892cc3a3140caf39aeee2be178569a. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

# NEPL3 Tutorial — <ruby>入力<rt>にゅうりょく</rt></ruby>と<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>観察<rt>かんさつ</rt></ruby>する

[正本（NEPL3d）](<hello.nepld>)

<a name="n-707572706f7365"></a>

## この<ruby>章<rt>しょう</rt></ruby>の<ruby>到達点<rt>とうたつてん</rt></ruby>

NEPL3は、<ruby>独立<rt>どくりつ</rt></ruby>したDSLの<ruby>構文<rt>こうぶん</rt></ruby>・<ruby>位置<rt>いち</rt></ruby>・<ruby>診断<rt>しんだん</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>を<ruby>共有<rt>きょうゆう</rt></ruby>し、<ruby>言語<rt>げんご</rt></ruby>の<ruby>再帰的<rt>さいきてき</rt></ruby>な<ruby>相互埋込<rt>そうごうめこ</rt></ruby>みを<ruby>扱<rt>あつか</rt></ruby>う<ruby>基盤<rt>きばん</rt></ruby>である。この<ruby>章<rt>しょう</rt></ruby>では、Helloという<ruby>小<rt>ちい</rt></ruby>さな<ruby>言語<rt>げんご</rt></ruby>へ<ruby>入力<rt>にゅうりょく</rt></ruby>を<ruby>与<rt>あた</rt></ruby>え、<ruby>構文木<rt>こうぶんぎ</rt></ruby>・token・<ruby>診断<rt>しんだん</rt></ruby>を<ruby>観察<rt>かんさつ</rt></ruby>する。Helloの<ruby>文法<rt>ぶんぽう</rt></ruby>はLanguagePackageとして<ruby>定義<rt>ていぎ</rt></ruby>され、<ruby>公開<rt>こうかい</rt></ruby>APIを<ruby>利用<rt>りよう</rt></ruby>する<ruby>独立<rt>どくりつ</rt></ruby>したconsumerから<ruby>実行<rt>じっこう</rt></ruby>される。<ruby>言語<rt>げんご</rt></ruby>の<ruby>作成<rt>さくせい</rt></ruby>、<ruby>別<rt>べつ</rt></ruby>のpackageとのcomposition、Sentence・annotation、<ruby>多階層<rt>たかいそう</rt></ruby>の<ruby>埋込<rt>うめこ</rt></ruby>みは、<ruby>基本課程<rt>きほんかてい</rt></ruby>の<ruby>後続<rt>こうぞく</rt></ruby>の<ruby>到達点<rt>とうたつてん</rt></ruby>である。

<a name="n-7365747570"></a>

## <ruby>実行環境<rt>じっこうかんきょう</rt></ruby>

NEPL3リポジトリをcheckoutし、rustupとRustのbuildに<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>環境<rt>かんきょう</rt></ruby>を<ruby>用意<rt>ようい</rt></ruby>する。<ruby>以下<rt>いか</rt></ruby>のコマンドはリポジトリのrootで<ruby>実行<rt>じっこう</rt></ruby>する。Rustのversionはrust\-toolchain\.toml、<ruby>依存<rt>いぞん</rt></ruby>ライブラリはconsumerのCargo\.lockで<ruby>固定<rt>こてい</rt></ruby>される。<ruby>初回<rt>しょかい</rt></ruby>のbuildではtoolchainと<ruby>依存<rt>いぞん</rt></ruby>ライブラリの<ruby>取得<rt>しゅとく</rt></ruby>にネットワーク<ruby>接続<rt>せつぞく</rt></ruby>を<ruby>使用<rt>しよう</rt></ruby>する。

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example inspect -- "hello 世界"
```

<a name="n-74726565"></a>

## <ruby>入力<rt>にゅうりょく</rt></ruby>と<ruby>構文木<rt>こうぶんぎ</rt></ruby>

tokenは、readerが<ruby>入力<rt>にゅうりょく</rt></ruby>から<ruby>読<rt>よ</rt></ruby>み<ruby>取<rt>と</rt></ruby>る<ruby>単位<rt>たんい</rt></ruby>である。headは<ruby>構文<rt>こうぶん</rt></ruby>の<ruby>先頭<rt>せんとう</rt></ruby>に<ruby>置<rt>お</rt></ruby>かれ、その<ruby>位置<rt>いち</rt></ruby>で<ruby>有効<rt>ゆうこう</rt></ruby>な<ruby>規則<rt>きそく</rt></ruby>に<ruby>従<rt>したが</rt></ruby>って<ruby>子<rt>こ</rt></ruby>の<ruby>読取<rt>よみと</rt></ruby>りを<ruby>定<rt>さだ</rt></ruby>める。Helloはheadであるhelloと、その<ruby>子<rt>こ</rt></ruby>である<ruby>一<rt>ひと</rt></ruby>つの<ruby>名前<rt>なまえ</rt></ruby>を<ruby>読<rt>よ</rt></ruby>む。<ruby>解析結果<rt>かいせきけっか</rt></ruby>のGreeting nodeはName nodeを<ruby>子<rt>こ</rt></ruby>として<ruby>保持<rt>ほじ</rt></ruby>する。<ruby>次<rt>つぎ</rt></ruby>は<ruby>出力<rt>しゅつりょく</rt></ruby>の<ruby>冒頭<rt>ぼうとう</rt></ruby>である。

```text
Complete; cursor=12
root: NodeRef(0)
node 0: org.example.hello::Greeting [Child(NodeRef(1))]
node 1: org.example.hello::Name []
```

Completeは<ruby>入力<rt>にゅうりょく</rt></ruby>の<ruby>解析<rt>かいせき</rt></ruby>が<ruby>完了<rt>かんりょう</rt></ruby>した<ruby>状態<rt>じょうたい</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す。cursorは<ruby>読取<rt>よみと</rt></ruby>りが<ruby>進<rt>すす</rt></ruby>んだUTF\-8のbyte<ruby>位置<rt>いち</rt></ruby>である。tokenのpayloadにはhelloと<ruby>世界<rt>せかい</rt></ruby>が<ruby>保持<rt>ほじ</rt></ruby>される。helloと<ruby>空白<rt>くうはく</rt></ruby>は6 bytes、<ruby>世界<rt>せかい</rt></ruby>は6 bytesであるため、<ruby>名前<rt>なまえ</rt></ruby>の<ruby>範囲<rt>はんい</rt></ruby>は6\.\.12となる。OriginのDirectは、その<ruby>構造<rt>こうぞう</rt></ruby>が<ruby>入力<rt>にゅうりょく</rt></ruby>sourceの<ruby>範囲<rt>はんい</rt></ruby>に<ruby>直接<rt>ちょくせつ</rt></ruby><ruby>対応<rt>たいおう</rt></ruby>することを<ruby>示<rt>しめ</rt></ruby>す。

<a name="n-6578657263697365"></a>

## <ruby>名前<rt>なまえ</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>する

コマンドの<ruby>入力引数<rt>にゅうりょくひきすう</rt></ruby>をhello NEPL3へ<ruby>変更<rt>へんこう</rt></ruby>し、<ruby>再実行<rt>さいじっこう</rt></ruby>する。GreetingとNameの<ruby>親子関係<rt>おやこかんけい</rt></ruby>は<ruby>維持<rt>いじ</rt></ruby>され、<ruby>最後<rt>さいご</rt></ruby>のtokenのpayloadはNEPL3、<ruby>範囲<rt>はんい</rt></ruby>は6\.\.11となる。<ruby>次<rt>つぎ</rt></ruby>に<ruby>別<rt>べつ</rt></ruby>の<ruby>名前<rt>なまえ</rt></ruby>を<ruby>指定<rt>してい</rt></ruby>し、そのUTF\-8 byte<ruby>数<rt>すう</rt></ruby>とcursorを<ruby>比較<rt>ひかく</rt></ruby>する。<ruby>演習<rt>えんしゅう</rt></ruby>では<ruby>入力引数<rt>にゅうりょくひきすう</rt></ruby>を<ruby>編集<rt>へんしゅう</rt></ruby>する。

<a name="n-646961676e6f7374696373"></a>

## <ruby>回復<rt>かいふく</rt></ruby>と<ruby>未完入力<rt>みかんにゅうりょく</rt></ruby>

goodbye <ruby>世界<rt>せかい</rt></ruby>を<ruby>指定<rt>してい</rt></ruby>すると、<ruby>未知<rt>みち</rt></ruby>のheadに<ruby>対<rt>たい</rt></ruby>してRecoveredとUnparsedInputの<ruby>診断<rt>しんだん</rt></ruby>が<ruby>表示<rt>ひょうじ</rt></ruby>される。helloだけを<ruby>指定<rt>してい</rt></ruby>すると、<ruby>入力終端<rt>にゅうりょくしゅうたん</rt></ruby>で<ruby>子<rt>こ</rt></ruby>が<ruby>不足<rt>ふそく</rt></ruby>するため、RecoveredとMissingChildが<ruby>表示<rt>ひょうじ</rt></ruby>される。Recoveredは、<ruby>問題<rt>もんだい</rt></ruby>を<ruby>診断<rt>しんだん</rt></ruby>に<ruby>記録<rt>きろく</rt></ruby>し、<ruby>回復<rt>かいふく</rt></ruby>した<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>返<rt>かえ</rt></ruby>した<ruby>状態<rt>じょうたい</rt></ruby>である。

```sh
cargo run --locked --manifest-path conformance/extensions/hello/Cargo.toml --example inspect -- --partial "hello "
```

\-\-partialは<ruby>追加<rt>ついか</rt></ruby>の<ruby>入力<rt>にゅうりょく</rt></ruby>が<ruby>続<rt>つづ</rt></ruby>く<ruby>条件<rt>じょうけん</rt></ruby>を<ruby>指定<rt>してい</rt></ruby>する。この<ruby>例<rt>れい</rt></ruby>ではNeedMoreが<ruby>返<rt>かえ</rt></ruby>される。<ruby>末尾<rt>まつび</rt></ruby>の<ruby>空白<rt>くうはく</rt></ruby>も<ruby>引用符<rt>いんようふ</rt></ruby>の<ruby>内部<rt>ないぶ</rt></ruby>へ<ruby>含<rt>ふく</rt></ruby>める。exampleは<ruby>解析結果<rt>かいせきけっか</rt></ruby>を<ruby>観察<rt>かんさつ</rt></ruby>するため、RecoveredやStoppedもデータとして<ruby>表示<rt>ひょうじ</rt></ruby>する。<ruby>終了<rt>しゅうりょう</rt></ruby>コード0は<ruby>観察処理<rt>かんさつしょり</rt></ruby>の<ruby>完了<rt>かんりょう</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>す。<ruby>解析<rt>かいせき</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>は<ruby>出力<rt>しゅつりょく</rt></ruby>のoutcomeと<ruby>診断<rt>しんだん</rt></ruby>で<ruby>確認<rt>かくにん</rt></ruby>する。

<a name="n-6e657874"></a>

## <ruby>実装<rt>じっそう</rt></ruby>との<ruby>対応<rt>たいおう</rt></ruby>

conformance\/extensions\/hello\/src\/lib\.rsのlanguageはschemaとLanguagePackageを<ruby>構築<rt>こうちく</rt></ruby>する。src\/parse\.rsはParseProfileとsource・environmentを<ruby>準備<rt>じゅんび</rt></ruby>し、ParseSessionを<ruby>実行<rt>じっこう</rt></ruby>する。examples\/inspect\.rsは<ruby>結果<rt>けっか</rt></ruby>を<ruby>表示<rt>ひょうじ</rt></ruby>し、src\/tests\.rsは<ruby>位置<rt>いち</rt></ruby>・<ruby>回復<rt>かいふく</rt></ruby>・<ruby>停止<rt>ていし</rt></ruby>とNDF<ruby>交換<rt>こうかん</rt></ruby>を<ruby>検査<rt>けんさ</rt></ruby>する。この<ruby>章<rt>しょう</rt></ruby>の<ruby>完了条件<rt>かんりょうじょうけん</rt></ruby>は、<ruby>入力<rt>にゅうりょく</rt></ruby>の<ruby>変更<rt>へんこう</rt></ruby>とtoken・<ruby>位置<rt>いち</rt></ruby>の<ruby>変化<rt>へんか</rt></ruby>を<ruby>対応<rt>たいおう</rt></ruby>させ、Complete・Recovered・NeedMoreを<ruby>説明<rt>せつめい</rt></ruby>できることである。
