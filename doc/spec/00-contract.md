<!-- Generated from doc/spec/00&#45;contract.nepld; renderer nepl3-tools.markdown-annotated/4; source SHA-256 c5d05dad1dfd9941c138ca3ec4ec778f6734eb6dc43f36e3433582df8a9a7236; alias input SHA-256 e272bc38006f1fc97f70928aa2963fe0245fa4575ddffadcc8d460b1f24a42cc. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="00-対象と設計上の決定"></a>

# 00\. <ruby>目的<rt>もくてき</rt></ruby>・<ruby>適用範囲<rt>てきようはんい</rt></ruby>・<ruby>設計原則<rt>せっけいげんそく</rt></ruby>

[正本（NEPL3d）](<00-contract.nepld>)

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## NEPL3の<ruby>目的<rt>もくてき</rt></ruby>と<ruby>仕様<rt>しよう</rt></ruby>の<ruby>役割<rt>やくわり</rt></ruby>

NEPL3は、<ruby>多数<rt>たすう</rt></ruby>の<ruby>独立<rt>どくりつ</rt></ruby>したDSLを、<ruby>括弧<rt>かっこ</rt></ruby>なし<ruby>前置記法<rt>ぜんちきほう</rt></ruby>の<ruby>共通規律<rt>きょうつうきりつ</rt></ruby>で<ruby>多階層<rt>たかいそう</rt></ruby>・<ruby>再帰的<rt>さいきてき</rt></ruby>に<ruby>相互<rt>そうご</rt></ruby><ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みする<ruby>言語基盤<rt>げんごきばん</rt></ruby>である。ある<ruby>言語<rt>げんご</rt></ruby>の<ruby>構文<rt>こうぶん</rt></ruby>の<ruby>内部<rt>ないぶ</rt></ruby>に<ruby>別<rt>べつ</rt></ruby>の<ruby>言語<rt>げんご</rt></ruby>を<ruby>使<rt>つか</rt></ruby>い、その<ruby>内部<rt>ないぶ</rt></ruby>でさらに<ruby>別<rt>べつ</rt></ruby>の<ruby>言語<rt>げんご</rt></ruby>を<ruby>使<rt>つか</rt></ruby>う<ruby>構成<rt>こうせい</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う。reader・<ruby>構文<rt>こうぶん</rt></ruby>・source・<ruby>診断<rt>しんだん</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>を<ruby>共有<rt>きょうゆう</rt></ruby>し、<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>意味論<rt>いみろん</rt></ruby>は<ruby>各言語<rt>かくげんご</rt></ruby>が<ruby>所有<rt>しょゆう</rt></ruby>する。<ruby>先行情報<rt>せんこうじょうほう</rt></ruby>から<ruby>後続<rt>こうぞく</rt></ruby>・<ruby>内側<rt>うちがわ</rt></ruby>の<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>確定<rt>かくてい</rt></ruby>し、その<ruby>後<rt>あと</rt></ruby>の<ruby>意味解決<rt>いみかいけつ</rt></ruby>は<ruby>個別言語<rt>こべつげんご</rt></ruby>へ<ruby>委<rt>ゆだ</rt></ruby>ねる。

この<ruby>仕様<rt>しよう</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>\{contract\}は、<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>意味<rt>いみ</rt></ruby>と、<ruby>操作<rt>そうさ</rt></ruby>の<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>出力<rt>しゅつりょく</rt></ruby>・<ruby>成功条件<rt>せいこうじょうけん</rt></ruby>・<ruby>失敗時<rt>しっぱいじ</rt></ruby>の<ruby>扱<rt>あつか</rt></ruby>いを<ruby>定<rt>さだ</rt></ruby>める。<ruby>実装<rt>じっそう</rt></ruby>は、<ruby>操作<rt>そうさ</rt></ruby>が<ruby>要求<rt>ようきゅう</rt></ruby>する<ruby>機能<rt>きのう</rt></ruby>を<ruby>満<rt>み</rt></ruby>たした<ruby>場合<rt>ばあい</rt></ruby>に<ruby>成功<rt>せいこう</rt></ruby>を<ruby>返<rt>かえ</rt></ruby>す。<ruby>未実装<rt>みじっそう</rt></ruby>の<ruby>機能<rt>きのう</rt></ruby>や<ruby>未確定<rt>みかくてい</rt></ruby>の<ruby>判断<rt>はんだん</rt></ruby>を<ruby>必要<rt>ひつよう</rt></ruby>とする<ruby>操作<rt>そうさ</rt></ruby>を、<ruby>成功<rt>せいこう</rt></ruby>として<ruby>返<rt>かえ</rt></ruby>してはならない。<ruby>拡張点<rt>かくちょうてん</rt></ruby>\{extension point\}についても、<ruby>入力<rt>にゅうりょく</rt></ruby>・<ruby>出力<rt>しゅつりょく</rt></ruby>・<ruby>失敗<rt>しっぱい</rt></ruby>・<ruby>許可範囲<rt>きょかはんい</rt></ruby>を<ruby>定義<rt>ていぎ</rt></ruby>する。

<a name="n-74617267657473"></a>

<a name="1-実装対象"></a>

## 1\. <ruby>実装対象<rt>じっそうたいしょう</rt></ruby>

このリポジトリは、<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>を<ruby>利用<rt>りよう</rt></ruby>するreference languageとして、Grammar・Doc・Math・Circuitの<ruby>契約<rt>けいやく</rt></ruby>を<ruby>定<rt>さだ</rt></ruby>める。<ruby>外部<rt>がいぶ</rt></ruby>の<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>言語<rt>げんご</rt></ruby>も、22<ruby>章<rt>しょう</rt></ruby>の<ruby>公開契約<rt>こうかいけいやく</rt></ruby>を<ruby>通<rt>とお</rt></ruby>して<ruby>追加<rt>ついか</rt></ruby>できる。<ruby>言語<rt>げんご</rt></ruby>の<ruby>追加<rt>ついか</rt></ruby>では、この<ruby>契約<rt>けいやく</rt></ruby>を<ruby>使<rt>つか</rt></ruby>い、foundationへの<ruby>言語名<rt>げんごめい</rt></ruby>による<ruby>特例<rt>とくれい</rt></ruby>の<ruby>追加<rt>ついか</rt></ruby>を<ruby>禁止<rt>きんし</rt></ruby>する。

- Grammarは、readerとprefix<ruby>構造<rt>こうぞう</rt></ruby>、<ruby>束縛<rt>そくばく</rt></ruby>\{binding\}、<ruby>表示分類<rt>ひょうじぶんるい</rt></ruby>、<ruby>外部<rt>がいぶ</rt></ruby>readerの<ruby>接続<rt>せつぞく</rt></ruby>を<ruby>定義<rt>ていぎ</rt></ruby>する。それらを<ruby>検査済<rt>けんさず</rt></ruby>みのLanguagePackageへcompileする。
- Docは、<ruby>再帰的<rt>さいきてき</rt></ruby>な<ruby>文書構造<rt>ぶんしょこうぞう</rt></ruby>、sentence literal、rubyとanno、sentence<ruby>単位<rt>たんい</rt></ruby>のparallel、<ruby>相互参照<rt>そうごさんしょう</rt></ruby>、<ruby>数式<rt>すうしき</rt></ruby>・<ruby>回路<rt>かいろ</rt></ruby>・コードの<ruby>埋<rt>う</rt></ruby>め<ruby>込<rt>こ</rt></ruby>みを<ruby>保持<rt>ほじ</rt></ruby>する。これらの<ruby>構造<rt>こうぞう</rt></ruby>からHTMLを<ruby>生成<rt>せいせい</rt></ruby>する。
- Mathは、<ruby>構造化<rt>こうぞうか</rt></ruby>した<ruby>数学表現<rt>すうがくひょうげん</rt></ruby>、<ruby>束縛<rt>そくばく</rt></ruby>、<ruby>厳密<rt>げんみつ</rt></ruby>な<ruby>有理数<rt>ゆうりすう</rt></ruby>と<ruby>配列<rt>はいれつ</rt></ruby>の<ruby>計算<rt>けいさん</rt></ruby>、MathML Coreへの<ruby>出力<rt>しゅつりょく</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う。
- Circuitは、<ruby>二値<rt>にち</rt></ruby>・<ruby>固定幅<rt>こていはば</rt></ruby>・<ruby>同期<rt>どうき</rt></ruby><ruby>離散時間<rt>りさんじかん</rt></ruby>の<ruby>階層回路<rt>かいそうかいろ</rt></ruby>を<ruby>宣言<rt>せんげん</rt></ruby>する。<ruby>検査<rt>けんさ</rt></ruby>、elaboration、step、テスト、NOR IR、SVGへの<ruby>出力<rt>しゅつりょく</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う。

<ruby>現在<rt>げんざい</rt></ruby>のworkspace<ruby>構成<rt>こうせい</rt></ruby>はCargo\.toml、<ruby>実装<rt>じっそう</rt></ruby>と<ruby>受入試験<rt>うけいれしけん</rt></ruby>の<ruby>状態<rt>じょうたい</rt></ruby>はimplementation\-status\.jsonに<ruby>記録<rt>きろく</rt></ruby>する。<ruby>上記<rt>じょうき</rt></ruby>の<ruby>一覧<rt>いちらん</rt></ruby>は<ruby>実装<rt>じっそう</rt></ruby>の<ruby>目標<rt>もくひょう</rt></ruby>を<ruby>示<rt>しめ</rt></ruby>し、<ruby>各機能<rt>かくきのう</rt></ruby>の<ruby>利用可否<rt>りようかひ</rt></ruby>はその<ruby>状態<rt>じょうたい</rt></ruby>に<ruby>従<rt>したが</rt></ruby>う。

SentenceとDocの<ruby>所有境界<rt>しょゆうきょうかい</rt></ruby>および<ruby>移行<rt>いこう</rt></ruby>は23<ruby>章<rt>しょう</rt></ruby>に<ruby>定<rt>さだ</rt></ruby>める。Circuitは7<ruby>章<rt>しょう</rt></ruby>の<ruby>回路言語<rt>かいろげんご</rt></ruby>を<ruby>指<rt>さ</rt></ruby>し、NEPL3cの<ruby>設計案<rt>せっけいあん</rt></ruby>は<ruby>別<rt>べつ</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>として<ruby>扱<rt>あつか</rt></ruby>う。

<ruby>完成品<rt>かんせいひん</rt></ruby>には、native CLI、wasm32\-wasip2 CLI、browser worker<ruby>用<rt>よう</rt></ruby>Wasm、<ruby>汎用<rt>はんよう</rt></ruby>LSP server、portable operation providerを<ruby>含<rt>ふく</rt></ruby>める。

r3では、<ruby>独立<rt>どくりつ</rt></ruby>した<ruby>言語<rt>げんご</rt></ruby>を<ruby>登録<rt>とうろく</rt></ruby>して<ruby>組<rt>く</rt></ruby>み<ruby>合<rt>あ</rt></ruby>わせるWeb Playgroundと、<ruby>純粋<rt>じゅんすい</rt></ruby>TEA UI coreを<ruby>含<rt>ふく</rt></ruby>める。Grammar・Doc・Math・Circuitの4<ruby>言語<rt>げんご</rt></ruby>は、Playgroundで<ruby>扱<rt>あつか</rt></ruby>うreference profileとして<ruby>提供<rt>ていきょう</rt></ruby>する。<ruby>静的<rt>せいてき</rt></ruby>な<ruby>文書<rt>ぶんしょ</rt></ruby>・<ruby>例<rt>れい</rt></ruby>・Rust APIサイトを<ruby>生成<rt>せいせい</rt></ruby>し、GitHub Pagesで<ruby>配布<rt>はいふ</rt></ruby>する。<ruby>正式文書<rt>せいしきぶんしょ</rt></ruby>を<ruby>最終的<rt>さいしゅうてき</rt></ruby>にNEPL3 Doc DSLへ<ruby>移行<rt>いこう</rt></ruby>する<ruby>計画<rt>けいかく</rt></ruby>と、その<ruby>受入<rt>うけいれ</rt></ruby>も<ruby>必須<rt>ひっす</rt></ruby>とする。<ruby>未移行<rt>みいこう</rt></ruby>のページはMarkdownを<ruby>正本<rt>せいほん</rt></ruby>\{canonical source\}とし、<ruby>移行審査<rt>いこうしんさ</rt></ruby>を<ruby>通過<rt>つうか</rt></ruby>したページはNEPL3dを<ruby>正本<rt>せいほん</rt></ruby>とする。<ruby>移行済<rt>いこうず</rt></ruby>みページの<ruby>正本<rt>せいほん</rt></ruby>と<ruby>生成<rt>せいせい</rt></ruby>するMarkdownはdoc\/canonical\.jsonで<ruby>管理<rt>かんり</rt></ruby>する。<ruby>未定義<rt>みていぎ</rt></ruby>のDoc<ruby>表現<rt>ひょうげん</rt></ruby>を、<ruby>情報<rt>じょうほう</rt></ruby>を<ruby>失<rt>うしな</rt></ruby>う<ruby>変換<rt>へんかん</rt></ruby>やRawHtmlで<ruby>埋<rt>う</rt></ruby>めてはならない。<ruby>詳細<rt>しょうさい</rt></ruby>は14〜16<ruby>章<rt>しょう</rt></ruby>に<ruby>定<rt>さだ</rt></ruby>める。

<ruby>汎用<rt>はんよう</rt></ruby>NEPL3プログラミング<ruby>言語<rt>げんご</rt></ruby>、<ruby>動画<rt>どうが</rt></ruby>DSL、<ruby>完全<rt>かんぜん</rt></ruby>なHTML<ruby>処理系<rt>しょりけい</rt></ruby>、アナログまたは<ruby>伝播遅延<rt>でんぱちえん</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う<ruby>回路<rt>かいろ</rt></ruby>、CAS、<ruby>証明器<rt>しょうめいき</rt></ruby>、<ruby>独自<rt>どくじ</rt></ruby>フォントrasterizerは、このパッケージの<ruby>言語<rt>げんご</rt></ruby>には<ruby>含<rt>ふく</rt></ruby>めない。これらの<ruby>名前<rt>なまえ</rt></ruby>でstubを<ruby>提供<rt>ていきょう</rt></ruby>してはならない。<ruby>追加実装<rt>ついかじっそう</rt></ruby>は、<ruby>登録済<rt>とうろくず</rt></ruby>みのschema・operation・readerの<ruby>公開契約<rt>こうかいけいやく</rt></ruby>を<ruby>通<rt>とお</rt></ruby>して<ruby>行<rt>おこな</rt></ruby>う。MathML・HTML・SVGの<ruby>出力<rt>しゅつりょく</rt></ruby>は、<ruby>正式<rt>せいしき</rt></ruby>な<ruby>出力<rt>しゅつりょく</rt></ruby>backendである。<ruby>後<rt>あと</rt></ruby>で<ruby>捨<rt>す</rt></ruby>てる<ruby>仮<rt>かり</rt></ruby>のrendererとして<ruby>実装<rt>じっそう</rt></ruby>してはならない。

<a name="n-626f756e646172696573"></a>

<a name="2-保存する意味上の境界"></a>

## 2\. <ruby>保存<rt>ほぞん</rt></ruby>する<ruby>意味上<rt>いみじょう</rt></ruby>の<ruby>境界<rt>きょうかい</rt></ruby>

- arityは、headを<ruby>識別<rt>しきべつ</rt></ruby>した<ruby>時点<rt>じてん</rt></ruby>で<ruby>既知<rt>きち</rt></ruby>のschemaとcontextから<ruby>確定<rt>かくてい</rt></ruby>する。<ruby>子<rt>こ</rt></ruby>の<ruby>評価結果<rt>ひょうかけっか</rt></ruby>によって、<ruby>親<rt>おや</rt></ruby>のarityを<ruby>変更<rt>へんこう</rt></ruby>してはならない。
- listは、<ruby>既知<rt>きち</rt></ruby>の `cons`（arity 2）と `nil`（arity 0）を<ruby>展開<rt>てんかい</rt></ruby>した<ruby>構文<rt>こうぶん</rt></ruby>で<ruby>表<rt>あらわ</rt></ruby>す。listofは、この<ruby>固定<rt>こてい</rt></ruby>arityの<ruby>構文<rt>こうぶん</rt></ruby>を<ruby>生成<rt>せいせい</rt></ruby>するschema compilerのcombinatorである。
- <ruby>構文<rt>こうぶん</rt></ruby>arityを<ruby>先行情報<rt>せんこうじょうほう</rt></ruby>から<ruby>確定<rt>かくてい</rt></ruby>できる<ruby>対象<rt>たいしょう</rt></ruby>は、<ruby>直接<rt>ちょくせつ</rt></ruby>headとして<ruby>読<rt>よ</rt></ruby>める。arityを<ruby>確定<rt>かくてい</rt></ruby>できない<ruby>関数値<rt>かんすうち</rt></ruby>への<ruby>適用<rt>てきよう</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う<ruby>言語<rt>げんご</rt></ruby>では、その<ruby>言語<rt>げんご</rt></ruby>のarity 0の<ruby>値参照<rt>ちさんしょう</rt></ruby>と、<ruby>固定<rt>こてい</rt></ruby>arity 2のapplyで<ruby>適用構造<rt>てきようこうぞう</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す。この<ruby>方式<rt>ほうしき</rt></ruby>で<ruby>複数<rt>ふくすう</rt></ruby>の<ruby>引数<rt>ひきすう</rt></ruby>を<ruby>渡<rt>わた</rt></ruby>す<ruby>場合<rt>ばあい</rt></ruby>は、binary applyを<ruby>反復<rt>はんぷく</rt></ruby>する。<ruby>今回<rt>こんかい</rt></ruby>のDSL constructorを、<ruby>関数値<rt>かんすうち</rt></ruby>の<ruby>適用<rt>てきよう</rt></ruby>へ<ruby>強制的<rt>きょうせいてき</rt></ruby>に<ruby>変換<rt>へんかん</rt></ruby>してはならない。
- <ruby>共通<rt>きょうつう</rt></ruby>prefix<ruby>構文木<rt>こうぶんぎ</rt></ruby>は、headとその<ruby>子<rt>こ</rt></ruby>の<ruby>関係<rt>かんけい</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す。token<ruby>内部<rt>ないぶ</rt></ruby>のreaderは、その<ruby>言語<rt>げんご</rt></ruby>が<ruby>定<rt>さだ</rt></ruby>める<ruby>構造<rt>こうぞう</rt></ruby>を<ruby>解析<rt>かいせき</rt></ruby>し、エディタ<ruby>向<rt>む</rt></ruby>けにviewを<ruby>公開<rt>こうかい</rt></ruby>できる。editor<ruby>用<rt>よう</rt></ruby>のviewを<ruby>公開<rt>こうかい</rt></ruby>しても、<ruby>共通<rt>きょうつう</rt></ruby>parserの<ruby>子<rt>こ</rt></ruby>の<ruby>数<rt>かず</rt></ruby>は<ruby>変<rt>か</rt></ruby>わらない。
- Parsed、Resolved、Checked、Preparedは、<ruby>構文<rt>こうぶん</rt></ruby>の<ruby>解析<rt>かいせき</rt></ruby>、<ruby>参照<rt>さんしょう</rt></ruby>の<ruby>解決<rt>かいけつ</rt></ruby>、<ruby>条件<rt>じょうけん</rt></ruby>の<ruby>検査<rt>けんさ</rt></ruby>、<ruby>操作<rt>そうさ</rt></ruby>に<ruby>必要<rt>ひつよう</rt></ruby>な<ruby>準備<rt>じゅんび</rt></ruby>を<ruby>表<rt>あらわ</rt></ruby>す<ruby>状態<rt>じょうたい</rt></ruby>である。<ruby>各操作<rt>かくそうさ</rt></ruby>が<ruby>要求<rt>ようきゅう</rt></ruby>する<ruby>状態<rt>じょうたい</rt></ruby>と、その<ruby>成立条件<rt>せいりつじょうけん</rt></ruby>は<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>仕様<rt>しよう</rt></ruby>に<ruby>定<rt>さだ</rt></ruby>める。<ruby>共通基盤<rt>きょうつうきばん</rt></ruby>は、すべての<ruby>言語<rt>げんご</rt></ruby>に<ruby>同<rt>おな</rt></ruby>じ<ruby>処理順序<rt>しょりじゅんじょ</rt></ruby>を<ruby>必須<rt>ひっす</rt></ruby>として<ruby>課<rt>か</rt></ruby>してはならない。
- <ruby>元<rt>もと</rt></ruby>の<ruby>表記<rt>ひょうき</rt></ruby>と<ruby>位置<rt>いち</rt></ruby>を<ruby>扱<rt>あつか</rt></ruby>う<ruby>操作<rt>そうさ</rt></ruby>は、<ruby>保存<rt>ほぞん</rt></ruby>したsourceとsource spanを<ruby>使<rt>つか</rt></ruby>う。<ruby>意味値<rt>いみち</rt></ruby>だけから、<ruby>元<rt>もと</rt></ruby>のsource<ruby>表記<rt>ひょうき</rt></ruby>やsource spanを<ruby>逆算<rt>ぎゃくさん</rt></ruby>してはならない。

<a name="n-70726f746f7479706573"></a>

<a name="3-試作の扱い"></a>

## 3\. <ruby>試作<rt>しさく</rt></ruby>の<ruby>扱<rt>あつか</rt></ruby>い

<ruby>段階的<rt>だんかいてき</rt></ruby>な<ruby>実装<rt>じっそう</rt></ruby>でも、API・モデル・エラー・<ruby>責務<rt>せきむ</rt></ruby>はこの<ruby>仕様<rt>しよう</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>に<ruby>従<rt>したが</rt></ruby>う。<ruby>後<rt>あと</rt></ruby>で<ruby>交換<rt>こうかん</rt></ruby>することを<ruby>前提<rt>ぜんてい</rt></ruby>に、<ruby>契約<rt>けいやく</rt></ruby>を<ruby>省略<rt>しょうりゃく</rt></ruby>した<ruby>仮設計<rt>かりせっけい</rt></ruby>を<ruby>実装<rt>じっそう</rt></ruby>することを<ruby>禁止<rt>きんし</rt></ruby>する。<ruby>設計<rt>せっけい</rt></ruby>の<ruby>誤<rt>あやま</rt></ruby>りは、<ruby>後方互換<rt>こうほうごかん</rt></ruby>の<ruby>維持<rt>いじ</rt></ruby>に<ruby>優先<rt>ゆうせん</rt></ruby>して<ruby>修正<rt>しゅうせい</rt></ruby>できる。<ruby>設計<rt>せっけい</rt></ruby>を<ruby>変更<rt>へんこう</rt></ruby>するときは、<ruby>新<rt>あたら</rt></ruby>しいdesign revisionを<ruby>与<rt>あた</rt></ruby>え、<ruby>関連<rt>かんれん</rt></ruby>するすべての<ruby>契約<rt>けいやく</rt></ruby>と<ruby>試験<rt>しけん</rt></ruby>を<ruby>同時<rt>どうじ</rt></ruby>に<ruby>更新<rt>こうしん</rt></ruby>する。

`v1` は<ruby>契約識別<rt>けいやくしきべつ</rt></ruby>に<ruby>用<rt>もち</rt></ruby>いるrevisionであり、<ruby>将来<rt>しょうらい</rt></ruby>の<ruby>無期限<rt>むきげん</rt></ruby>なABI<ruby>互換<rt>ごかん</rt></ruby>を<ruby>約束<rt>やくそく</rt></ruby>する<ruby>呼称<rt>こしょう</rt></ruby>ではない。

<a name="n-696e76617269616e7473"></a>

<a name="4-不変条件"></a>

## 4\. <ruby>不変条件<rt>ふへんじょうけん</rt></ruby>

<ruby>以下<rt>いか</rt></ruby>のINV01〜INV14は、<ruby>構文<rt>こうぶん</rt></ruby>・source・<ruby>言語<rt>げんご</rt></ruby>の<ruby>接続<rt>せつぞく</rt></ruby>・<ruby>操作結果<rt>そうさけっか</rt></ruby>を<ruby>通<rt>つう</rt></ruby>じて<ruby>保持<rt>ほじ</rt></ruby>する<ruby>条件<rt>じょうけん</rt></ruby>である。<ruby>意味<rt>いみ</rt></ruby>モデルと<ruby>中間表現<rt>ちゅうかんひょうげん</rt></ruby>の<ruby>補足<rt>ほそく</rt></ruby><ruby>不変条件<rt>ふへんじょうけん</rt></ruby>は12<ruby>章<rt>しょう</rt></ruby>、<ruby>受入試験<rt>うけいれしけん</rt></ruby>は11<ruby>章<rt>しょう</rt></ruby>に<ruby>定<rt>さだ</rt></ruby>める。

- INV01：prefixの<ruby>境界<rt>きょうかい</rt></ruby>が<ruby>一意<rt>いちい</rt></ruby>であること。
- INV02：<ruby>未知<rt>みち</rt></ruby>のarityを<ruby>推測<rt>すいそく</rt></ruby>しないこと。
- INV03：<ruby>読<rt>よ</rt></ruby>み<ruby>過<rt>す</rt></ruby>ぎを<ruby>禁止<rt>きんし</rt></ruby>すること。
- INV04：source snapshotへの<ruby>所属<rt>しょぞく</rt></ruby>を<ruby>保持<rt>ほじ</rt></ruby>すること。
- INV05：source mappingを<ruby>明示<rt>めいじ</rt></ruby>すること。
- INV06：domain\-specific kindを<ruby>使<rt>つか</rt></ruby>うこと。
- INV07：<ruby>値<rt>あたい</rt></ruby>の<ruby>生成<rt>せいせい</rt></ruby>と<ruby>実行対象<rt>じっこうたいしょう</rt></ruby>を<ruby>区別<rt>くべつ</rt></ruby>すること。
- INV08：<ruby>不正<rt>ふせい</rt></ruby>な<ruby>構文<rt>こうぶん</rt></ruby>も<ruby>保存<rt>ほぞん</rt></ruby>できること。
- INV09：<ruby>言語<rt>げんご</rt></ruby>coreの<ruby>依存関係<rt>いぞんかんけい</rt></ruby>をDAGにすること。
- INV10：<ruby>意味<rt>いみ</rt></ruby>と<ruby>操作<rt>そうさ</rt></ruby>の<ruby>契約<rt>けいやく</rt></ruby>をportableにすること。
- INV11：failureを<ruby>成功<rt>せいこう</rt></ruby>へ<ruby>変<rt>か</rt></ruby>えないこと。
- INV12：<ruby>同<rt>おな</rt></ruby>じresource snapshotから<ruby>決定的<rt>けっていてき</rt></ruby>な<ruby>結果<rt>けっか</rt></ruby>を<ruby>得<rt>え</rt></ruby>ること。
- INV13：sentenceの<ruby>対応単位<rt>たいおうたんい</rt></ruby>を<ruby>著者<rt>ちょしゃ</rt></ruby>が<ruby>指定<rt>してい</rt></ruby>すること。
- INV14：providerとworkspaceのtrustを<ruby>区別<rt>くべつ</rt></ruby>すること。

<a name="n-7370656c6c696e6773"></a>

<a name="5-具体的な綴り"></a>

## 5\. constructorと<ruby>名前参照<rt>なまえさんしょう</rt></ruby>

<ruby>構文<rt>こうぶん</rt></ruby>をプログラムから<ruby>生成<rt>せいせい</rt></ruby>するときは、<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>公開<rt>こうかい</rt></ruby>constructor APIで<ruby>構造<rt>こうぞう</rt></ruby>を<ruby>構築<rt>こうちく</rt></ruby>する。<ruby>追加<rt>ついか</rt></ruby>する<ruby>言語<rt>げんご</rt></ruby>も、<ruby>同<rt>おな</rt></ruby>じconstructor schemaを<ruby>呼<rt>よ</rt></ruby>び<ruby>出<rt>だ</rt></ruby>す<ruby>契約<rt>けいやく</rt></ruby>を<ruby>使<rt>つか</rt></ruby>う。Grammar・Doc・Math・Circuitの<ruby>組込<rt>くみこ</rt></ruby>みには `Fn`、`value`、`splice` を<ruby>含<rt>ふく</rt></ruby>めない。Grammarの `call` \/ `map` \/ `then` は、Rust providerへ<ruby>処理<rt>しょり</rt></ruby>を<ruby>接続<rt>せつぞく</rt></ruby>する<ruby>拡張点<rt>かくちょうてん</rt></ruby>である。

<ruby>各言語<rt>かくげんご</rt></ruby>の<ruby>全<rt>ぜん</rt></ruby>constructorとarityは `design/forms.json` に<ruby>定<rt>さだ</rt></ruby>める。そこで<ruby>定義<rt>ていぎ</rt></ruby>されていない<ruby>綴<rt>つづ</rt></ruby>りは、<ruby>該当<rt>がいとう</rt></ruby>カテゴリの<ruby>明示的<rt>めいじてき</rt></ruby>な<ruby>識別子<rt>しきべつし</rt></ruby>leaf<ruby>規則<rt>きそく</rt></ruby>に<ruby>一致<rt>いっち</rt></ruby>する<ruby>場合<rt>ばあい</rt></ruby>だけ、<ruby>名前参照<rt>なまえさんしょう</rt></ruby>として<ruby>読<rt>よ</rt></ruby>める。<ruby>未知<rt>みち</rt></ruby>のformを、arity 0と<ruby>推測<rt>すいそく</rt></ruby>してはならない。
