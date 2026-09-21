<!-- Generated from doc/spec/05&#45;document.nepld; renderer nepl3-tools.markdown-annotated-pages/1; page document; source SHA-256 fe97e6f48fa1f3422786448dc5210fc2edd85a857fe4bf5fc5ecb722f5a1a55f; alias input SHA-256 64a9b57605880e4195a0e4916ed09a70e2f29ee9b9c0f2430644eca1f267b2b8; document digest 691416c6ea8f5a800b70affe30b5552fda77b6bc8ced368e180fbd5e9987bce7; input PageSet digest 1b5e3849c4d1f347ca6b59e97d9d1ae4c0244cc6657f3d39aa12480838b9c40d; input context SHA-256 9a8c521bcbe028c7be3bf71358a9de1352f11057e9787ac4bf04245bcd480b31. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="05-doc言語"></a>

# 05\. Doc言語\[げんご\]

<a name="n-706f6c696379"></a>

<a name="方針"></a>

## 方針\[ほうしん\]

本文\[ほんぶん\]の階層\[かいそう\]、文単位\[ぶんたんい\]の翻訳対応\[ほんやくたいおう\]、inlineの注釈\[ちゅうしゃく\]を独立\[どくりつ\]した構造\[こうぞう\]として扱\[あつか\]う。sentence literalとprefix constructorを同\[おな\]じ意味\[いみ\]モデルへlowerする。source表記\[ひょうき\]は別\[べつ\]に保持\[ほじ\]する。

文章\[ぶんしょう\]の最終的\[さいしゅうてき\]な所有者\[しょゆうしゃ\]は独立\[どくりつ\]したNEPL3sentenceであり、Docは文書構造\[ぶんしょこうぞう\]を所有\[しょゆう\]する。現在\[げんざい\]の開発\[かいはつ\]hostはSentenceのliteral readerから明示\[めいじ\]adapterでDocのSentencePayloadへ変換\[へんかん\]する段階\[だんかい\]にある。本章\[ほんしょう\]のDocValue・DocView・payloadは現在\[げんざい\]のDoc consumerの契約\[けいやく\]であり、Sentenceの独立\[どくりつ\]schemaと同一視\[どういつし\]しない。所有関係\[しょゆうかんけい\]と未完了\[みかんりょう\]のconsumer移行\[いこう\]は[23章](<23\-sentence\-annotation\.md>)を参照\[さんしょう\]する。

<a name="n-7479706573"></a>

<a name="1-主要な型"></a>

## 1\. 主要\[しゅよう\]な型\[かた\]

Article\(language\, title\:Sentence\, body\:Body\)。BodyはBlockの列\[れつ\]。BlockはParagraph、Section、DisplayMath、CircuitFigure、Code、Table、List、RawCode、Image。ParagraphはFlowの列\[れつ\]。FlowはSentence、ParallelとBlockの各\[かく\]variant。したがってParagraphを再帰的\[さいきてき\]に含\[ふく\]められる。SentenceはInlineの列\[れつ\]。Parallelは2個以上\[こいじょう\]のVariant\(language\, sentence\)の列\[れつ\]。VariantへParagraphやParallelは入\[い\]れられない。InlineはText、Concat、Ruby\(base\,reading\)、Anno\(base\,notes\)、InlineMath、Anchor\(id\,label\)、Reference\(target\,label\)、Emphasis、Strong、Break、Link、InlineCode、InlineImage。

全\[ぜん\]signatureはdoc\-signaturesを参照\[さんしょう\]。bare quoted文字列\[もじれつ\]をSentence\/Flowの位置\[いち\]で読\[よ\]むとSentenceLiteralになる。`text` の子\[こ\]は通常\[つうじょう\]Textであり、その内部\[ないぶ\]の `[` や `{` を注釈\[ちゅうしゃく\]と解釈\[かいしゃく\]しない。

<a name="n-6172656e61"></a>

<a name="11-意味モデルと公開arena"></a>

### 1\.1 意味\[いみ\]モデルと公開\[こうかい\]arena

`interfaces/model.json` の `Doc:*` recordと `Doc/*` unionはconstructorの論理的\[ろんりてき\]な意味展開\[いみてんかい\]であり、独立\[どくりつ\]した再帰\[さいき\]wire layoutではない。Doc操作\[そうさ\]で送受信\[そうじゅしん\]する値\[あたい\]の正本\[せいほん\]は `interfaces/doc.json` の `DocumentSyntax` \/ `DocValue`。`DocValue.nodes` の各\[かく\] `DocNode.kind` がconstructorを表\[あらわ\]し、子\[こ\]はカテゴリ別\[べつ\]のindex参照\[さんしょう\]を使\[つか\]う。Rust enumの並\[なら\]びをwire tagへ転用\[てんよう\]せず、schemaの明示\[めいじ\]variant名\[めい\]とfield列\[れつ\]を対応\[たいおう\]させる。

Article以外\[いがい\]のBody、Block、Flow、Sentence、Inline、Variant、Row、ListItem、MathGuest、CircuitGuest、Guestも明示\[めいじ\] `DocRoot` として単独\[たんどく\]のfragmentを構成\[こうせい\]できる。Alignment、ListStyle、Check、LinkTarget、Asset、OptionalRow、OptionalSentence、OptionalTextは表層\[ひょうそう\]の固定\[こてい\]arity補助\[ほじょ\]constructorである。単独\[たんどく\]lower時\[じ\]には対応\[たいおう\]するarena wrapperをrootとする。親\[おや\]constructorのoperandである場合\[ばあい\]はAlignment、ListKind、Option、LinkTarget、AssetRefという型付\[かたつ\]き値\[あたい\]へ取\[と\]り込\[こ\]み、補助\[ほじょ\]wrapperを意味的\[いみてき\]な子\[こ\]として残\[のこ\]さない。元\[もと\]operandのView、Origin、source byte宣言\[せんげん\]はこの取\[と\]り込\[こ\]みで破棄\[はき\]しない。Optionやlistの暗黙文法\[あんもくぶんぽう\]は導入\[どうにゅう\]せず、signatureにある `none` \/ `some`、`cons` \/ `nil` を読\[よ\]む。

arenaの構造検査\[こうぞうけんさ\]はroot\/childカテゴリ、index範囲\[はんい\]、循環\[じゅんかん\]、到達性\[とうたつせい\]、注釈内容\[ちゅうしゃくないよう\]、表\[ひょう\]の列数等\[れつすうなど\]を検査\[けんさ\]する。共有\[きょうゆう\]DAGを許\[ゆる\]すが全経路\[ぜんけいろ\]の最大\[さいだい\]Depthを検査\[けんさ\]し、ForeignClosureの内側深\[うちがわふか\]さもその所有\[しょゆう\]nodeまでの深\[ふか\]さに合成\[ごうせい\]する。この構造\[こうぞう\]proofはlabel解決\[かいけつ\]、guest意味\[いみ\]check、PreparedArticleを意味\[いみ\]しない。tokenごとのViewは `DocView.head` に束縛\[そくばく\]し、ViewRefとrelationのID空間\[くうかん\]をtoken間\[かん\]で混\[ま\]ぜない。Text正規化後\[せいきかご\]も元\[もと\]ViewとOriginを保存\[ほぞん\]する。sourceを持\[も\]つTextとsource\-less Textを結合\[けつごう\]する場合\[ばあい\]、後者\[こうしゃ\]を明示\[めいじ\]Synthetic OriginとしてCompositeに含\[ふく\]め、既知\[きち\]spanを全体\[ぜんたい\]のspanに偽装\[ぎそう\]しない。

<a name="n-656c656d656e7473"></a>

<a name="12-文書入力に必要な要素"></a>

### 1\.2 文書入力\[ぶんしょにゅうりょく\]に必要\[ひつよう\]な要素\[ようそ\]

DG01のTableは列\[れつ\]ごとのDefault\/Left\/Center\/Right、任意\[にんい\]header、順序付\[じゅんじょつ\]きRowを持\[も\]つ。全\[ぜん\]Rowのcell数\[すう\]は列数\[れつすう\]と一致\[いっち\]する。cellはSentenceであり空\[くう\]Sentenceも明示値\[めいじち\]として許\[ゆる\]す。ゼロ列\[れつ\]のTableは構造上\[こうぞうじょう\]の空表\[くうひょう\]として表現可能\[ひょうげんかのう\]で、内容\[ないよう\]のある列\[れつ\]を補\[おぎな\]わない。

DG02のListはUnorderedまたはOrdered\(start\:Nat\)を明示\[めいじ\]し、ListItemは任意\[にんい\]のchecked状態\[じょうたい\]とBodyを持\[も\]つ。Bodyを介\[かい\]した入\[い\]れ子\[こ\]を保持\[ほじ\]する。Orderedのstartは公開\[こうかい\]arenaのU64で表\[あらわ\]せる範囲\[はんい\]をlower時\[じ\]に検査\[けんさ\]し、範囲外\[はんいがい\]を丸\[まる\]めない。

DG03のLinkはPage\(page\,fragment\)、Relative\(path\,fragment\)、External\(uri\)を区別\[くべつ\]し、表示\[ひょうじ\]labelを保持\[ほじ\]する。DG06のSection\/Anchor\/Referenceのarticle内\[ない\]IDとは区別\[くべつ\]し、ページ名\[めい\]、外部\[がいぶ\]URI、相対\[そうたい\]pathを同\[おな\]じ名前空間\[なまえくうかん\]で自動解決\[じどうかいけつ\]しない。公開\[こうかい\]URIへの写像\[しゃぞう\]とリンク解決\[かいけつ\]はcheck\/prepare側\[がわ\]の明示入力\[めいじにゅうりょく\]であり、構造値\[こうぞうち\]だけからファイル読出\[よみだ\]し権限\[けんげん\]や安全\[あんぜん\]なHTML属性\[ぞくせい\]を得\[え\]ない。

DG04のInlineCodeとRawCodeは意味解析\[いみかいせき\]しないTextを保持\[ほじ\]する。RawCodeのlanguageHintは任意\[にんい\]の表示情報\[ひょうじじょうほう\]で、parserや評価器\[ひょうかき\]の自動実行要求\[じどうじっこうようきゅう\]ではない。DG05のImage\/InlineImageはAssetRef\(id\,任意\[にんい\]digest\)とaltを持\[も\]ち、block画像\[がぞう\]は任意\[にんい\]captionを持\[も\]つ。表層\[ひょうそう\]のdigestはOptionalTextの64桁\[けた\]hexを32byteのDigestへlowerし、無効\[むこう\]な桁数\[けたすう\]・文字\[もじ\]を拒否\[きょひ\]する。assetの読出\[よみだ\]し、digest照合\[しょうごう\]、表示準備\[ひょうじじゅんび\]は別\[べつ\]の解決操作\[かいけつそうさ\]で行\[おこな\]う。RawHtmlは追加\[ついか\]しない。

この要素選択\[ようそせんたく\]はmain `b5295cef655aa59affffd6644f2902268d071953` の文書入力監査\[ぶんしょにゅうりょくかんさ\]（inventory SHA\-256 `daf94085913930f05c1655d2adbef4f56651864f9a97ac4d261400449c98499e`）に基\[もと\]づく。61 Markdown、52表\[ひょう\]\/1283cell、39list\/233item、1134 inline code、12 code block、249link、1imageを含\[ふく\]む。監査\[かんさ\]は実装中差分\[じっそうちゅうさぶん\]やrustdocの意味監査\[いみかんさ\]の完了\[かんりょう\]を示\[しめ\]さず、実装済\[じっそうず\]み操作\[そうさ\]は `implementation-status.json` と実行済\[じっこうず\]み受入\[うけいれ\]で区別\[くべつ\]する。

<a name="n-696e7370656374696f6e"></a>

<a name="13-文書準備の要求発見"></a>

### 1\.3 文書準備\[ぶんしょじゅんび\]の要求発見\[ようきゅうはっけん\]

`prepare::inspect` はArticleの構造\[こうぞう\]・source閉包\[へいほう\]・article内\[ない\]labelを検査\[けんさ\]し、`DocPreparationPlan` を返\[かえ\]す。これは必要\[ひつよう\]な外部入力\[がいぶにゅうりょく\]の列挙\[れっきょ\]であり、完全\[かんぜん\]なPreparedArticleではない。全\[ぜん\]Parallel variantを対象\[たいしょう\]とし、表示言語\[ひょうじげんご\]の選択\[せんたく\]やguest操作\[そうさ\]を実行\[じっこう\]しない。

planは `documentDigest` と順序付\[じゅんじょつ\]き `requirements` を持\[も\]つ。Linkは意味\[いみ\]node indexと元\[もと\]LinkTarget、Assetは意味\[いみ\]node indexと元\[もと\]AssetRef、ForeignはEmbedRef・EmbedKind・guestDigestを持\[も\]つ。Link\/Assetはarena順\[じゅん\]に一度\[いちど\]ずつ列挙\[れっきょ\]し、その後\[ご\]owner embed表順\[ひょうじゅん\]にForeignを列挙\[れっきょ\]する。同\[おな\]じ資源\[しげん\]を参照\[さんしょう\]する別\[べつ\]nodeは別\[べつ\]の要求\[ようきゅう\]であり、共有\[きょうゆう\]nodeの表示出現\[ひょうじしゅつげん\]ごとには増\[ふ\]やさない。Codeも構文\[こうぶん\]のままForeignとして保持\[ほじ\]し、意味\[いみ\]エラーを含\[ふく\]む例\[れい\]の表示\[ひょうじ\]を可能\[かのう\]にする。

documentDigestは `SHA-256("NEPL3.Doc.Prepare.Document.v1\0" || canonical-NDF/1-CBOR(DocumentSyntax))`、guestDigestは `SHA-256("NEPL3.Doc.Prepare.Guest.v1\0" || canonical-NDF/1-CBOR(ForeignClosure))`。domainの `\0` はゼロbyte、digestは32byte。owner環境\[かんきょう\]・source・Originを含\[ふく\]め、型名\[かためい\]やURIだけをidentityにしない。Doc nodeとOrigin IDは保持\[ほじ\]し、共通\[きょうつう\]codecのsource表順\[ひょうじゅん\]・guest NodeRef正準化\[せいじゅんか\]に従\[したが\]う。

portable planの送受信\[そうじゅしん\]には対象\[たいしょう\]DocumentSyntaxを明示\[めいじ\]し、同\[おな\]じ検査\[けんさ\]と要求列挙\[ようきゅうれっきょ\]を再実行\[さいじっこう\]して全\[ぜん\]fieldを比較\[ひかく\]する。schema\-validでも古\[ふる\]い文書\[ぶんしょ\]、欠落\[けつらく\]\/追加\[ついか\]\/重複\[じゅうふく\]\/順序違\[じゅんじょちが\]い、異\[こと\]なるnode\/guest\/digestを拒否\[きょひ\]する。source admissionとBudgetを操作内\[そうさない\]で共有\[きょうゆう\]し、停止\[ていし\]は元\[もと\]StopReasonを保持\[ほじ\]する。native helperのPreparationErrorは構造\[こうぞう\]\/label\/boundary\/停止\[ていし\]を区別\[くべつ\]するが、完全\[かんぜん\]なcheck\/prepareのReport操作包絡\[そうさほうらく\]として広告\[こうこく\]しない。

この発見段階\[はっけんだんかい\]はURIの安全性\[あんぜんせい\]、page\/fragmentの存在\[そんざい\]、asset byte列\[れつ\]・MIME・digestの適合\[てきごう\]、guest生成物\[せいせいぶつ\]や表示言語\[ひょうじげんご\]の準備\[じゅんび\]を証明\[しょうめい\]しない。後続\[こうぞく\]prepareで明示資源\[めいじしげん\]・解決結果\[かいけつけっか\]を同\[おな\]じdocument\/guest identityへ束縛\[そくばく\]し、必要条件\[ひつようじょうけん\]がすべて満\[み\]たされてからHTML backendへ渡\[わた\]す。

<a name="n-6c69746572616c"></a>

<a name="2-sentence-literalの完全な規則"></a>

## 2\. sentence literalの完全\[かんぜん\]な規則\[きそく\]

説明用\[せつめいよう\]EBNF\:

```text
SentenceLiteral = '"' InlineSequence '"'
InlineSequence  = InlineItem*
InlineItem      = TextRun | Escape | Ruby | Anno
Ruby            = '[' NonemptySequence '/' NonemptySequence ']'
Anno            = '{' NonemptySequence ('/' NonemptySequence)+ '}'
```

InlineSequenceの終端集合\[しゅうたんしゅうごう\]は呼出\[よびだ\]し位置\[いち\]で決\[き\]まる。topでは未\[み\]escapeの `"`、Rubyのbaseでは `/`、readingでは `]`、Annoの各\[かく\]fieldでは `/` または `}`。入\[い\]れ子\[こ\]のRuby\/Annoは一\[ひと\]つのInlineItemとして読\[よ\]むため、内側\[うちがわ\]の `/` を外側\[そとがわ\]の区切\[くぎ\]りにしない。

未\[み\]escapeの `[` と `{` は常\[つね\]に注釈開始\[ちゅうしゃくかいし\]。対応\[たいおう\]しない `]` \/ `}` はエラー。topの `/` は通常文字\[つうじょうもじ\]。注釈\[ちゅうしゃく\]field内\[ない\]のliteral `/` は `\/` と書\[か\]く。Rubyはtop\-level separatorがちょうど一\[ひと\]つ。Annoは一\[ひと\]つ以上\[いじょう\]。

escapeは通常\[つうじょう\]Textのものに加\[くわ\]え `\[`、`\]`、`\{`、`\}`、`\/`。escapeの出力\[しゅつりょく\]を改\[あらた\]めて注釈開始\[ちゅうしゃくかいし\]として解析\[かいせき\]しない。直接\[ちょくせつ\]のCR\/LFは禁止\[きんし\]、`\n` は内容\[ないよう\]の改行\[かいぎょう\]。EOF・改行\[かいぎょう\]・終了引用符\[しゅうりょういんようふ\]に達\[たっ\]した時点\[じてん\]で閉\[と\]じていない注釈\[ちゅうしゃく\]は、その開始位置\[かいしいち\]をrelatedに持\[も\]つUnclosedAnnotation。

Nonemptyはlower後\[ご\]に可視内容\[かしないよう\]が存在\[そんざい\]すること。空\[くう\]Textや空\[くう\]Concatだけのbase\/reading\/noteはEmptyAnnotationPart。Annoのnotesは非空\[ひくう\]。Sentence自体\[じたい\]は空\[から\]を許\[ゆる\]す。

ここで可視内容\[かしないよう\]はfontや描画幅\[びょうがはば\]に依存\[いぞん\]させず、保持\[ほじ\]する内容\[ないよう\]で判定\[はんてい\]する。Text（および明示\[めいじ\]inline code）は非空\[ひくう\]なら内容\[ないよう\]を持\[も\]ち、空白\[くうはく\]だけのTextやescapeで生成\[せいせい\]した改行\[かいぎょう\]も含\[ふく\]める。明示\[めいじ\]Break、InlineMath、inline画像\[がぞう\]は表示要素\[ひょうじようそ\]として内容\[ないよう\]を持\[も\]つ。Concatは子\[こ\]のいずれか、装飾\[そうしょく\]・Anchor・Reference・Linkはlabel\/子\[こ\]、Ruby\/Annoはbaseの内容\[ないよう\]を用\[もち\]いる。空要素\[くうようそ\]を隠\[かく\]れた空白\[くうはく\]へ補\[おぎな\]うことはない。literal、prefix、portable constructorは同\[おな\]じ判定\[はんてい\]を用\[もち\]いる。

例\[れい\]\: `"これは{[文書/ぶんしょ]/document}を記述する。"` はText \+ Anno\(Ruby\(\.\.\.\)\,\[Text\]\) \+ Text。

<a name="n-6c696e65627265616b"></a>

<a name="21-文書構造の明示改行"></a>

### 2\.1\. 文書構造\[ぶんしょこうぞう\]の明示改行\[めいじかいぎょう\]

前置構文\[ぜんちこうぶん\]の `break`（Doc\/Inline、arity 0）はDoc\.Breakを構築\[こうちく\]する。同\[おな\]じSentence内\[ない\]の明示的\[めいじてき\]な改行\[かいぎょう\]であり、新\[あたら\]しいSentence・Paragraph・Parallel variantを作\[つく\]らない。例\[たと\]えば `sentence cons text "a" cons break cons text "b" nil` と書\[か\]く。HTMLは`br`、plain\_textはLFを出力\[しゅつりょく\]し、source printerは`break`を保持\[ほじ\]する。

著者\[ちょしゃ\]が指定\[してい\]する文書\[ぶんしょ\]の改行\[かいぎょう\]にはbreakを用\[もち\]いる。Text中\[ちゅう\]の`\n`は文字\[もじ\]データのLFとして保持\[ほじ\]する既存\[きそん\]escapeであり、Break nodeを生成\[せいせい\]しない。literalにはBreak constructorを追加\[ついか\]せず、明示改行\[めいじかいぎょう\]を含\[ふく\]むSentenceは前置構築\[ぜんちこうちく\]を使\[つか\]う。TextのLF、明示\[めいじ\]Break、Paragraph境界\[きょうかい\]、表示時\[ひょうじじ\]の自動折返\[じどうおりかえ\]しを相互\[そうご\]に推測変換\[すいそくへんかん\]しない。

<a name="n-6571756976616c656e6365"></a>

<a name="3-prefix経路との等価性"></a>

## 3\. prefix経路\[けいろ\]との等価性\[とうかせい\]

```text
sentence
  cons text "これは"
  cons anno
    ruby text "文書" text "ぶんしょ"
    cons text "document" nil
  cons text "を記述する。"
  nil
```

上例\[じょうれい\]と前節\[ぜんせつ\]のliteralは、Origin\/sourceを除\[のぞ\]いた意味正規形\[いみせいきけい\]が等\[ひと\]しい。正規化\[せいきか\]はConcatの平坦化\[へいたんか\]、空\[くう\]Textの除去\[じょきょ\]、同\[おな\]じinline列内\[れつない\]の隣接\[りんせつ\]Textの結合\[けつごう\]だけ。Ruby\/Annoの境界\[きょうかい\]、noteの順\[じゅん\]、明示\[めいじ\]Break、Math等\[など\]の埋\[う\]め込\[こ\]みは保存\[ほぞん\]する。

受理済\[じゅりず\]みSentenceLiteralとprefix constructorの混在\[こんざい\]lowerでは、literal payloadを明示\[めいじ\]DocumentSyntax schemaとpayload自身\[じしん\]のsource宣言\[せんげん\]で検査\[けんさ\]する。payloadの単一\[たんいつ\]View owner\/headと局所\[きょくしょ\]Viewは外\[そと\]tokenのものと一致\[いっち\]し、意味\[いみ\]nodeのSpanとOriginの位置\[いち\]は同\[おな\]じtoken head内\[ない\]、または明示\[めいじ\]SourceMapの全経路\[ぜんけいろ\]でその範囲内\[はんいない\]へ戻\[もど\]らなければならない。別\[べつ\]tokenのpayloadや、Viewだけを合\[あ\]わせて意味位置\[いみいち\]を別\[べつ\]source・別範囲\[べつはんい\]へ移\[うつ\]した値\[あたい\]は拒否\[きょひ\]する。元\[もと\]テキストを再\[さい\]parseして意味値\[いみち\]を推測\[すいそく\]し直\[なお\]さない。外側構文\[そとがわこうぶん\]のOrigin IDとtoken\-local View IDは保持\[ほじ\]し、literal内\[ない\]のOrigin参照\[さんしょう\]だけを結合後\[けつごうご\]のarenaへ再配置\[さいはいち\]する。ForeignClosureのowner\-local Originと環境\[かんきょう\]digestにはこの再配置\[さいはいち\]を適用\[てきよう\]しない。source admissionは操作内\[そうさない\]で共有\[きょうゆう\]し、payload宣言\[せんげん\]の欠落\[けつらく\]をambient source storeから補\[おぎな\]わない。

単一\[たんいつ\]source内\[ない\]で完結\[かんけつ\]するreader向\[む\]けには、追加\[ついか\]の交換型\[こうかんがた\] `SentencePayload` を用意\[ようい\]する。field順\[じゅん\]は `value: DocValue`、`origins: List<Origin>`、`view: DocView` とし、`view.head` のSpanが元\[もと\]sourceの完全\[かんぜん\]なsnapshot参照\[さんしょう\]を保持\[ほじ\]する。本文\[ほんぶん\]byte列\[れつ\]はpayloadへ重複格納\[ちょうふくかくのう\]せず、呼出\[よびだ\]し元\[もと\]が明示\[めいじ\]するowner sourceから解決\[かいけつ\]する。受信\[じゅしん\]codecはその一\[ひと\]つのsourceだけをscopeとし、同\[おな\]じURIやsource名\[めい\]だけが一致\[いっち\]する別\[べつ\]revision・別\[べつ\]digest、ambientにしかないsourceで参照\[さんしょう\]を補\[おぎな\]わない。

この型\[かた\]はSentence root、Text・Concat・Ruby・Annoのみのnode、単一\[たんいつ\]DocView、単一\[たんいつ\]source、空\[から\]のembeds・sourceMapsに限定\[げんてい\]する。各意味\[かくいみ\]nodeはSpanとOriginを持\[も\]ち、位置\[いち\]はhead内\[ない\]に含\[ふく\]まれる。Originの合成\[ごうせい\]は既存\[きそん\]の非循環\[ひじゅんかん\]・参照検査\[さんしょうけんさ\]に従\[したが\]い、Direct・生成呼出\[せいせいよびだ\]し位置\[いち\]・Synthetic anchorの実在\[じつざい\]Spanも同\[おな\]じhead内\[ない\]を要求\[ようきゅう\]する。型\[かた\]・source閉包\[へいほう\]・位置\[いち\]の検査\[けんさ\]と資源計上\[しげんけいじょう\]はnative\/portableで共通\[きょうつう\]に行\[おこな\]い、停止理由\[ていしりゆう\]を変更\[へんこう\]しない。対応\[たいおう\]するRustの型付\[かたつ\]き意味値\[いみち\]は既存\[きそん\]のDocumentSyntaxであり、codec受信時\[じゅしんじ\]に明示\[めいじ\]owner sourceを加\[くわ\]えて構築\[こうちく\]する。

この追加型\[ついかがた\]は、別生成\[べつせいせい\]sourceから明示\[めいじ\]SourceMapで戻\[もど\]す従来\[じゅうらい\]のDocumentSyntax payloadを置\[お\]き換\[か\]えない。providerは署名\[しょめい\]で返却型\[へんきゃくがた\]を明示\[めいじ\]し、lowerでは型\[かた\]ごとに検査\[けんさ\]する。SentencePayload単独\[たんどく\]の受信検査\[じゅしんけんさ\]は指定\[してい\]source内\[ない\]のliteralデータを検査\[けんさ\]するものであり、特定\[とくてい\]tokenへの所属\[しょぞく\]を証明\[しょうめい\]しない。lowerが実際\[じっさい\]のowner bundleのsource宣言\[せんげん\]から対象\[たいしょう\]を選\[えら\]び、外\[そと\]tokenのheadと局所\[きょくしょ\]Viewの完全一致\[かんぜんいっち\]を追加\[ついか\]で要求\[ようきゅう\]する。元本文\[もとほんぶん\]の省略\[しょうりゃく\]、再\[さい\]parse、架空\[かくう\]のsource位置\[いち\]、provider版\[ばん\]の暗黙\[あんもく\]な切替\[きりか\]えによって重複\[ちょうふく\]を減\[へ\]らしてはならない。

標準文法\[ひょうじゅんぶんぽう\]は `doc.reader/sentence-v2` を明示\[めいじ\]して選択\[せんたく\]し、`nepl3.doc.reader` の `sentenceReferenced` 操作\[そうさ\]を呼\[よ\]ぶ。包絡\[ほうらく\]はReadRequest\/ReadReplyのままで、ProviderSignatureのvalue出力\[しゅつりょく\]をSentencePayloadとする。従来\[じゅうらい\]の `sentence` 操作\[そうさ\]およびDocumentSyntaxの交換型\[こうかんがた\]とは識別\[しきべつ\]を分\[わ\]け、Profileの解決済\[かいけつず\]み署名\[しょめい\]・文法\[ぶんぽう\]source・provider identityを更新\[こうしん\]する。lowerは受信\[じゅしん\]recordの型\[かた\]を選\[えら\]んでから対応\[たいおう\]するcodecを一度\[いちど\]だけ呼\[よ\]び、SentencePayloadの検査失敗\[けんさしっぱい\]を旧\[きゅう\]codecで再試行\[さいしこう\]しない。

意味値\[いみち\]からliteralを出力\[しゅつりょく\]するprinterはText\/Ruby\/Annoだけの表現\[ひょうげん\]ならescapeを行\[おこな\]ってliteral化\[か\]できる。他\[ほか\]のInlineがあればprefix構文\[こうぶん\]を出力\[しゅつりょく\]する。内容\[ないよう\]を削除\[さくじょ\]して無理\[むり\]にliteralにしない。どちらでも再\[さい\]parse\/lower後\[ご\]の意味\[いみ\]が一致\[いっち\]することを保証\[ほしょう\]する。

<a name="n-706172616c6c656c"></a>

<a name="4-parallelの単位"></a>

## 4\. parallelの単位\[たんい\]

Sentenceは著者\[ちょしゃ\]が明示\[めいじ\]した対応単位\[たいおうたんい\]であり、句読点\[くとうてん\]を自動検出\[じどうけんしゅつ\]した自然言語学的\[しぜんげんごがくてき\]な文\[ぶん\]ではない。literal中\[ちゅう\]に句点\[くてん\]が二\[ふた\]つあっても一\[ひと\]つのSentence。二\[ふた\]つのSentenceに対応\[たいおう\]させたい場合\[ばあい\]は著者\[ちょしゃ\]が別\[べつ\]ノードにする。

Parallelはvariantのlanguageがcase\-insensitiveで重複\[じゅうふく\]しないこと、2variant以上\[いじょう\]であることを検査\[けんさ\]する。空\[くう\]Sentenceは翻訳\[ほんやく\]が空\[から\]であるという明示値\[めいじち\]として許\[ゆる\]すが、missingと推測\[すいそく\]して他言語\[たげんご\]から補\[おぎな\]わない。並\[なら\]び順\[じゅん\]はソース順\[じゅん\]。各\[かく\]variantにCorrespondsTo relationを与\[あた\]え、名前束縛\[なまえそくばく\]の同一性\[どういつせい\]にはしない。

paragraphのネスト、節番号\[せつばんごう\]、HTMLの行折返\[ぎょうおりかえ\]しはこの対応\[たいおう\]を変更\[へんこう\]しない。単一言語表示\[たんいつげんごひょうじ\]はRenderOptionsの指定\[してい\]languageに一致\[いっち\]するvariantのみを選\[えら\]ぶ。なければMissingVariantを返\[かえ\]すか、明示\[めいじ\]されたFallbackLanguage順\[じゅん\]で選\[えら\]ぶ。暗黙\[あんもく\]の先頭選択\[せんとうせんたく\]はしない。

<a name="n-6c6162656c73"></a>

<a name="5-ラベルと参照"></a>

## 5\. ラベルと参照\[さんしょう\]

Section\.idとInline Anchor\.idはarticleのDocLabel空間\[くうかん\]へ定義\[ていぎ\]をexportする。ネストしたparagraph\/sectionも同\[おな\]じarticle内\[ない\]で集\[あつ\]める。重複\[じゅうふく\]はエラー。Reference\.targetはarticle内\[ない\]から解決\[かいけつ\]し、前方参照\[ぜんぽうさんしょう\]を認\[みと\]める。別\[べつ\]articleやforeign言語\[げんご\]へ暗黙\[あんもく\]にscopeを広\[ひろ\]げない。

source上\[じょう\]の名前\[なまえ\]の選択範囲\[せんたくはんい\]と、定義全体\[ていぎぜんたい\]の範囲\[はんい\]を分\[わ\]ける。Referenceは表示\[ひょうじ\]labelを持\[も\]ち、参照先\[さんしょうさき\]の見出\[みだ\]し文字列\[もじれつ\]を自動\[じどう\]コピーしない。HTML idは `n-` \+ UTF\-8 byteの小文字\[こもじ\]hexとし、任意\[にんい\]の名前\[なまえ\]から衝突\[しょうとつ\]なく生成\[せいせい\]する。

`DocNode.locations` は名前\[なまえ\]operandの位置\[いち\]を `DocFieldLocation` として保持\[ほじ\]する。fieldはDoc所有\[しょゆう\]の閉\[と\]じた種別\[しゅべつ\]SectionId \/ AnchorId \/ ReferenceTargetで、対応\[たいおう\]するDocKindだけに指定\[してい\]でき、同\[おな\]じfieldの重複\[ちょうふく\]を許\[ゆる\]さない。lowerは元\[もと\]の型付\[かたつ\]きchild\/tokenからSpanとOriginを取得\[しゅとく\]し、意味\[いみ\]Stringの綴\[つづ\]りを本文\[ほんぶん\]から検索\[けんさく\]して位置\[いち\]を作\[つく\]らない。source\-less constructorでは空\[から\]のlocationsまたはNoneの位置\[いち\]を保持\[ほじ\]する。存在\[そんざい\]するSpan\/Originは宣言済\[せんげんず\]みsource\/Origin arenaを参照\[さんしょう\]し、Spanは指定\[してい\]された定義全体\[ていぎぜんたい\]cover内\[ない\]へ明示\[めいじ\]SourceMapを通\[つう\]じて包含\[ほうがん\]される必要\[ひつよう\]がある。Originも指定\[してい\]した場合\[ばあい\]は、その明示\[めいじ\]causeが選択\[せんたく\]Spanを支\[ささ\]えることを検査\[けんさ\]する。構造検査\[こうぞうけんさ\]と初回\[しょかい\]NDF decodeに同\[おな\]じ条件\[じょうけん\]を適用\[てきよう\]し、元\[もと\]tokenの局所\[きょくしょ\]Viewは変更\[へんこう\]しない。

意味\[いみ\]nodeの共有\[きょうゆう\]と表示上\[ひょうじじょう\]のanchorの一意性\[いちいせい\]は別\[べつ\]である。同\[おな\]じSection\/Anchor nodeへArticle rootから複数\[ふくすう\]の表示経路\[ひょうじけいろ\]がある場合\[ばあい\]はDuplicateOccurrenceとする。直接\[ちょくせつ\]同\[おな\]じ子\[こ\]を二度\[にど\]参照\[さんしょう\]する場合\[ばあい\]と、共有\[きょうゆう\]parentを介\[かい\]する場合\[ばあい\]の双方\[そうほう\]を含\[ふく\]む。labelを含\[ふく\]まないDAG共有\[きょうゆう\]は許容\[きょよう\]する。暗黙\[あんもく\]のID複製\[ふくせい\]・改名\[かいめい\]では回避\[かいひ\]しない。経路数\[けいろすう\]を2で飽和\[ほうわ\]させる予算付\[よさんつ\]き検査\[けんさ\]を行\[おこな\]い、同\[おな\]じsource selectionを持\[も\]つ二\[ふた\]つの出現\[しゅつげん\]は、owner node・子\[こ\]の順序\[じゅんじょ\]index・target nodeを並\[なら\]べた `LabelOccurrencePaths` で識別\[しきべつ\]する。この型付\[かたつ\]き経路\[けいろ\]を通常\[つうじょう\]Diagnosticの `LabelDiagnosticArguments` に含\[ふく\]め、架空\[かくう\]のSpanを追加\[ついか\]しない。foreign guestの構造\[こうぞう\]はこのArticleの表示経路\[ひょうじけいろ\]として展開\[てんかい\]せず、そのlabelを暗黙\[あんもく\]に定義\[ていぎ\]・参照\[さんしょう\]へ取\[と\]り込\[こ\]まない。

<a name="n-656d62656464696e67"></a>

<a name="6-埋め込み"></a>

## 6\. 埋\[う\]め込\[こ\]み

InlineMath \/ DisplayMath \/ CircuitFigure \/ Codeは、スロット種別\[しゅべつ\]とForeignClosureを保持\[ほじ\]する。ForeignClosureはForeignSyntaxに選択済\[せんたくず\]みowner環境\[かんきょう\]、元\[もと\]Origin表\[ひょう\]、source\/map宣言閉包\[せんげんへいほう\]を加\[くわ\]えた共通型\[きょうつうがた\]である。環境\[かんきょう\]digestに含\[ふく\]まれる元\[もと\]Origin IDを保存\[ほぞん\]し、guest自身\[じしん\]のOrigin表\[ひょう\]と混同\[こんどう\]しない。MathやCircuitの型\[かた\]をdoc\-coreへimportしない。

suiteがMathの構造\[こうぞう\]・bindingをcheckし、[17章\[しょう\]](<17\-math\-html\.md>)の生成\[せいせい\]policyに従\[したが\]ってhostによるKaTeX生成\[せいせい\]、または独立\[どくりつ\]math\-mathmlの検査済\[けんさず\]みfragmentを準備\[じゅんび\]する。閲覧時\[えつらんじ\]にKaTeXを再実行\[さいじっこう\]しない。CircuitFigureはcheck\/elaborateとdiagramを使\[つか\]う。Codeは元\[もと\]のguest sourceとviewの表示\[ひょうじ\]であり、guestのlower・意味\[いみ\]check・compile・evaluateを実行\[じっこう\]しない。guestがDoc自身\[じしん\]の場合\[ばあい\]もDoc\:DocGuest\.syntaxはForeignClosure中\[ちゅう\]のForeignSyntaxを保持\[ほじ\]し、Doc\/Articleの意味値\[いみち\]への変換\[へんかん\]を表示\[ひょうじ\]の前提\[ぜんてい\]にしない。bundleのschema・参照\[さんしょう\]・source範囲\[はんい\]の安全性検査\[あんぜんせいけんさ\]は省略\[しょうりゃく\]しない。

標準\[ひょうじゅん\]Doc文法\[ぶんぽう\]の `DocGuest.syntax` は `foreign Doc Article` を読\[よ\]む。表層\[ひょうそう\]の `Doc article ...` は変\[か\]わらないが、同\[どう\]aliasへの入\[い\]れ子\[こ\]でも独立\[どくりつ\]guest bundleと選択\[せんたく\]された環境\[かんきょう\]を保持\[ほじ\]し、終了後\[しゅうりょうご\]はhostの読取\[よみとり\]contextへ復帰\[ふっき\]する。Profileには標準\[ひょうじゅん\]alias `Doc` の登録\[とうろく\]が必要\[ひつよう\]である。hostを別\[べつ\]aliasへ登録\[とうろく\]してもforeign先\[さき\]をそのaliasへ暗黙\[あんもく\]に置換\[ちかん\]しない。同\[おな\]じpackageを `Doc` として明示登録\[めいじとうろく\]するか、別名\[べつめい\]を指定\[してい\]する変更済\[へんこうず\]み文法\[ぶんぽう\]\/packageを明示選択\[めいじせんたく\]する。Code用\[よう\]Doc guestが意味的\[いみてき\]に不正\[ふせい\]な注釈\[ちゅうしゃく\]やlabelを含\[ふく\]んでも、構文\[こうぶん\]が成立\[せいりつ\]している限\[かぎ\]り表示\[ひょうじ\]のためにguestのDoc lowerを呼\[よ\]んで拒否\[きょひ\]しない。

Docのcheckは通常構造\[つうじょうこうぞう\]・注釈\[ちゅうしゃく\]・labelを検査\[けんさ\]し、foreign slotにはRequirementを返\[かえ\]す。suiteが要求\[ようきゅう\]を解決\[かいけつ\]した `PreparedArticle` だけをHTML backendへ渡\[わた\]す。未解決\[みかいけつ\]slotの空表示\[からひょうじ\]や文字列化\[もじれつか\]による偽成功\[ぎせいこう\]は禁止\[きんし\]。

<a name="n-68746d6c"></a>

<a name="7-html-backend"></a>

## 7\. HTML backend

Articleはarticle要素\[ようそ\]、titleはh1。Sectionはsection要素\[ようそ\]と適切\[てきせつ\]な見出\[みだ\]し。深\[ふか\]さ6超\[ちょう\]はrole\=headingとaria\-levelを持\[も\]つ要素\[ようそ\]を使\[つか\]う。Paragraphを機械的\[きかいてき\]に入\[い\]れ子\[こ\]のpへ変換\[へんかん\]しない。Paragraphはdiv構造\[こうぞう\]、連続\[れんぞく\]したSentence\/Parallelのrunをpへまとめ、子\[こ\]Blockはrunを閉\[と\]じてから出\[だ\]す。

Sentence間\[かん\]へ空白\[くうはく\]を勝手\[かって\]に挿入\[そうにゅう\]しない。必要\[ひつよう\]な空白\[くうはく\]はTextに含\[ふく\]める。white\-space\:pre\-wrapで著者\[ちょしゃ\]の空白\[くうはく\]と明示改行\[めいじかいぎょう\]を尊重\[そんちょう\]する。

Ruby\/Annoはネスト可能\[かのう\]なtyped span構造\[こうぞう\]として描画\[びょうが\]し、base、上側\[うえがわ\]のreading、下側\[したがわ\]のnotesを分離\[ぶんり\]する。reference backendはinline\-gridを基本\[きほん\]とし、必要\[ひつよう\]なbaseline指定\[してい\]に未対応\[みたいおう\]のbrowserでは20章\[しょう\]のinline\-table配置\[はいち\]へ切\[き\]り替\[か\]える。同\[おな\]じspan構造\[こうぞう\]を維持\[いじ\]するため、CSSの選択\[せんたく\]によってsemantic HTML正規形\[せいきけい\]は変\[か\]わらない。単純\[たんじゅん\]Rubyには別\[べつ\]backendでHTML ruby\/rtを使用\[しよう\]してもよい。CSSは配布\[はいふ\]asset、リモートfont\/CDN\/JSは不要\[ふよう\]。

Parallelは一\[ひと\]つのalignment wrapperにlanguageごとのsentence spanを入\[い\]れる。横並\[よこなら\]び・縦並\[たてなら\]び・単一言語\[たんいつげんご\]はRenderOptionsとして変\[か\]える。DOM上\[じょう\]に対応関係\[たいおうかんけい\]IDを保存\[ほぞん\]する。

全\[ぜん\]Text\/属性\[ぞくせい\]をmarkup serializerでescapeする。RawHtml variantはDocに存在\[そんざい\]しない。外部\[がいぶ\]HTMLを表示\[ひょうじ\]するextensionは別\[べつ\]の明示的\[めいじてき\]なtrust契約\[けいやく\]を要求\[ようきゅう\]する。token化\[か\]されたHTMLをそのままinnerHTMLへ渡\[わた\]してはならない。

<a name="n-7075626c6963"></a>

<a name="8-公開操作"></a>

## 8\. 公開操作\[こうかいそうさ\]

lower\(Parsed\, Profile\) \-\> DocumentSyntax \+ diagnostics。check\(DocumentSyntax\, LabelEnvironment\) \-\> CheckedArticle \+ foreign Requirements。prepare\(CheckedArticle\, ResolvedEmbeds\) \-\> PreparedArticle。render\(PreparedArticle\, RenderOptions\) \-\> HtmlArtifact。plain\_text\(PlainTextRequest\) \-\> PlainTextReply。print\(PrintRequest\) \-\> PrintReply。

AnnotationPolicyはBaseOnly、WithReadings、WithAllNotesを明示\[めいじ\]する。テキスト抽出時\[ちゅうしゅつじ\]に隠\[かく\]れた翻訳選択\[ほんやくせんたく\]を行\[おこな\]わない。各\[かく\]constructorはRust APIとportable record constructorの双方\[そうほう\]から呼\[よ\]べる。

<a name="n-706c61696e74657874"></a>

<a name="81-plain_text-の実行契約"></a>

### 8\.1\. plain\_text の実行契約\[じっこうけいやく\]

実操作\[じっそうさ\]descriptorは `nepl3.doc@1` の `plainText: PlainTextRequest -> PlainTextReply`（pure）である。要求\[ようきゅう\]は宣言\[せんげん\]source閉包\[へいほう\]を持\[も\]つDocumentSyntax、対象\[たいしょう\]SentenceRef、AnnotationPolicy、明示\[めいじ\]ResolvedInlineText列\[れつ\]を所有\[しょゆう\]する。SentenceRefはそのdocument内\[ない\]のSentenceを指\[さ\]す必要\[ひつよう\]があり、別\[べつ\]categoryはExpectedSentenceとなる。schemaとDocumentSyntax構造\[こうぞう\]の不正\[ふせい\]は入力境界\[にゅうりょくきょうかい\]の型付\[かたつ\]きエラーである。受理済\[じゅりず\]み要求\[ようきゅう\]の名前付\[なまえつ\]き失敗\[しっぱい\]はPlainTextFailureで返\[かえ\]し、停止\[ていし\]は準備段階\[じゅんびだんかい\]も含\[ふく\]めStoppedと元\[もと\]StopReasonを返\[かえ\]す。成功時\[せいこうじ\]だけComplete\.textを返\[かえ\]す。Reportは同一操作\[どういつそうさ\]のUsageを持\[も\]ち、その位置参照\[いちさんしょう\]は元要求\[もとようきゅう\]documentの宣言\[せんげん\]source閉包\[へいほう\]に限\[かぎ\]る。独立返信\[どくりつへんしん\]codecにも元要求\[もとようきゅう\]documentを明示\[めいじ\]して渡\[わた\]し、ambient sourceを補完\[ほかん\]しない。非\[ひ\]StoppedとtraceOverflowの組合\[くみあわ\]せを拒否\[きょひ\]する。

BaseOnlyはRubyとAnnoのbaseだけを投影\[とうえい\]する。WithReadingsはRubyを `base[reading]` とし、Annoはbaseだけを投影\[とうえい\]する。WithAllNotesはRubyも同\[おな\]じとし、Annoを `base{note1/note2}` とする。入\[い\]れ子\[こ\]にも同\[おな\]じpolicyを適用\[てきよう\]する。TextとInlineCodeの文字\[もじ\]は変更\[へんこう\]せず、BreakはLFを一\[ひと\]つ出力\[しゅつりょく\]する。装飾\[そうしょく\]は子\[こ\]inline、Anchor\/Reference\/Linkは明示\[めいじ\]label、InlineImageは明示\[めいじ\]alt Sentenceを投影\[とうえい\]する。Sentence\/Concatの子間\[こかん\]に空白\[くうはく\]を追加\[ついか\]しない。この出力\[しゅつりょく\]は再\[さい\]parse用\[よう\]sentence literalではなく、角括弧\[かくかっこ\]・波括弧\[なみかっこ\]・slashをescapeし直\[なお\]さない。任意\[にんい\]separatorのoptionsは設\[もう\]けない。

InlineMathは暗黙\[あんもく\]にlower・意味\[いみ\]check・評価\[ひょうか\]しない。必要\[ひつよう\]な表示\[ひょうじ\]textはhostが `ResolvedInlineText{documentDigest,embed,guestDigest,text}` として提供\[ていきょう\]する。documentDigestは `SHA-256("NEPL3.Doc.PlainText.Document.v1\0" || canonical-NDF/1-CBOR(DocumentSyntax))`、guestDigestは `SHA-256("NEPL3.Doc.PlainText.Guest.v1\0" || canonical-NDF/1-CBOR(ForeignClosure))` とする。引用内\[いんようない\]の `\0` は一\[ひと\]つのゼロbyte、他\[ほか\]はASCII byteである。値\[あたい\]はそれぞれDocの正式\[せいしき\]DocumentSyntax codecとfoundationの正式\[せいしき\]ForeignClosure codecが返\[かえ\]すschema検査済\[けんさず\]み値\[あたい\]であり、型\[かた\]を参照\[さんしょう\]するSchemaRefのdigestもcanonical CBORへ含\[ふく\]める。guest側\[がわ\]はowner環境\[かんきょう\]・元\[もと\]Origin列\[れつ\]・source\/map閉包\[へいほう\]も含\[ふく\]み、局所\[きょくしょ\]EmbedRefだけの一致\[いっち\]で別\[べつ\]guestや古\[ふる\]い環境\[かんきょう\]のtextを再利用\[さいりよう\]しない。

全提供\[ぜんていきょう\]entryについて32byte digest、対象\[たいしょう\]InlineMath、重複\[ちょうふく\]EmbedRef、現在\[げんざい\]document\/guest両\[りょう\]identityとの一致\[いっち\]を検査\[けんさ\]する。policyがそのentryを表示\[ひょうじ\]しない場合\[ばあい\]も不正\[ふせい\]entryを黙殺\[もくさつ\]しない。一方\[いっぽう\]、未提供\[みていきょう\]textによるUnresolvedEmbedは選択\[せんたく\]policyで実際\[じっさい\]に投影\[とうえい\]するInlineMathだけに適用\[てきよう\]し、BaseOnlyで隠\[かく\]れるreading\/notesのtextを要求\[ようきゅう\]しない。hostのtextは明示\[めいじ\]データであり、guest意味\[いみ\]checkが成功\[せいこう\]した証明\[しょうめい\]ではない。

identity取得\[しゅとく\]のprepareは独立\[どくりつ\]した操作\[そうさ\]であり、後\[のち\]の実行予算\[じっこうよさん\]を支払\[しはら\]い済\[ず\]みとするproofではない。公開実行入口\[こうかいじっこういりぐち\]plain\_textは毎回同一\[まいかいどういつ\]Budget\/SourceAdmissionでdocument検査\[けんさ\]、canonical値\[あたい\]の生成\[せいせい\]とhash、entry検査\[けんさ\]、出力\[しゅつりょく\]を合成\[ごうせい\]する。sourceは共有\[きょうゆう\]admissionで同\[どう\]snapshotを一度\[いちど\]だけ計上\[けいじょう\]し、出力\[しゅつりょく\]byte、Work、Allocation、深\[ふか\]さを計上\[けいじょう\]する。停止時\[ていしじ\]は元要求\[もとようきゅう\]を変更\[へんこう\]せず、不完全\[ふかんぜん\]なTextをCompleteへ昇格\[しょうかく\]させない。

<a name="n-7072696e74"></a>

<a name="82-print-の実行契約"></a>

### 8\.2\. print の実行契約\[じっこうけいやく\]

hostがguest sourceを先\[さき\]に生成\[せいせい\]する場合\[ばあい\]も、Doc内\[ない\]の埋\[う\]め込\[こ\]み位置\[いち\]をDepthへ引\[ひ\]き継\[つ\]ぐ。`print::guest_depths` は検査済\[けんさず\]みDoc DAGからEmbedRefごとの最大\[さいだい\]owner深\[ふか\]さを求\[もと\]める。rootを1とし、共有\[きょうゆう\]guestは最初\[さいしょ\]の出現\[しゅつげん\]ではなく最\[もっと\]も深\[ふか\]い表示経路\[ひょうじけいろ\]を採用\[さいよう\]する。これはO\(nodes \+ edges \+ embeds\)の準備\[じゅんび\]であり、guestの意味検査\[いみけんさ\]やdigest照合\[しょうごう\]を代替\[だいたい\]しない。hostは現在\[げんざい\]の深\[ふか\]さへそのoffsetを加\[くわ\]え、同\[おな\]じBudgetで選択済\[せんたくず\]みguest printerを呼\[よ\]ぶ。生成結果\[せいせいけっか\]は既存\[きそん\]のdocument\/guest digest付\[つ\]きPrintedGuestとして通常\[つうじょう\]printへ渡\[わた\]す。

開発\[かいはつ\]hostのDoc\/Math source adapterは、明示選択\[めいじせんたく\]したMath ExprとDoc Sentenceを相互\[そうご\]にlower\/printし、評価\[ひょうか\]を実行\[じっこう\]しない。同期再帰\[どうきさいき\]adapterのhost上限\[じょうげん\]は合計\[ごうけい\]Depth 64とし、呼出元\[よびだしもと\]のより小\[ちい\]さい上限\[じょうげん\]を維持\[いじ\]する。上限超過\[じょうげんちょうか\]・cancelを元\[もと\]のStoppedとして返\[かえ\]し、別\[べつ\]Budgetや原文\[げんぶん\]コピーで成功\[せいこう\]へ切\[き\]り替\[か\]えない。このhost上限\[じょうげん\]はDoc\/Mathの意味型\[いみがた\]の制限\[せいげん\]ではない。Circuitなど未選択\[みせんたく\]のguestをMathとして解釈\[かいしゃく\]せず、選択不一致\[せんたくふいっち\]または未解決\[みかいけつ\]として拒否\[きょひ\]する。

実\[じつ\]descriptorは `nepl3.doc@1` の `print: PrintRequest -> PrintReply`（pure）。PrintRequestはDocumentSyntax、PrintMode（Prefix \/ Compact）、明示\[めいじ\]GuestBinding列\[れつ\]、PrintedGuest列\[れつ\]を所有\[しょゆう\]する。Completeだけが `SourceArtifact{text,entry}` を返\[かえ\]す。entryは再\[さい\]parseする具体的\[ぐたいてき\]なDoc表層\[ひょうそう\]categoryであり、hostがtext保存時\[ほぞんじ\]のSourceId・revision・URIとSourceSnapshotを発行\[はっこう\]する。printerが架空\[かくう\]のsnapshotや元\[もと\]source位置\[いち\]を作\[つく\]ることはない。再\[さい\]parse\/lowerとの一致\[いっち\]はsource\/Originを除\[のぞ\]いた意味正規形\[いみせいきけい\]について定\[さだ\]め、未正規化\[みせいきか\]のconstructor値\[あたい\]を変更\[へんこう\]して入力\[にゅうりょく\]へ書\[か\]き戻\[もど\]さない。

Prefixは全\[ぜん\]constructorを正式\[せいしき\]なheadと引数順\[ひきすうじゅん\]で出力\[しゅつりょく\]する。CompactはSentence内\[ない\]がText \/ Concat \/ Ruby \/ Annoだけで表\[あらわ\]せる場合\[ばあい\]にsentence literalへし、それ以外\[いがい\]はprefixを保持\[ほじ\]する。Textは引用符\[いんようふ\]・backslash・CR・LF・tabをescapeし、literalでは注釈\[ちゅうしゃく\]delimiterもescapeする。escapeの生成文字\[せいせいもじ\]を再\[ふたた\]び注釈\[ちゅうしゃく\]と解釈\[かいしゃく\]しない。明示\[めいじ\]BreakはTextの改行\[かいぎょう\]と異\[こと\]なる意味\[いみ\]nodeなので、Breakを含\[ふく\]むSentenceをliteralへ変換\[へんかん\]しない。装飾\[そうしょく\]・link・画像\[がぞう\]・Mathを省略\[しょうりゃく\]してliteral化\[か\]することもない。

以下\[いか\]のentryと4言語\[げんご\]wrapperは現在\[げんざい\]の標準\[ひょうじゅん\]Doc profileの契約\[けいやく\]であり、NEPL3全体\[ぜんたい\]の言語集合\[げんごしゅうごう\]を制限\[せいげん\]しない。補助\[ほじょ\]fragmentを含\[ふく\]む表層\[ひょうそう\]entryは20種類\[しゅるい\]である。guestの具体\[ぐたい\]entryはMathGuest \/ CircuitGuest \/ Guestの3種類\[しゅるい\]、wrapperはMath \/ Circuit \/ Grammar \/ Docの4種類\[しゅるい\]。独立\[どくりつ\]rootでは `DocKind.Guest{language,syntax}` とEmbedKind\.Guestを保持\[ほじ\]し、MathGuest\/CircuitGuestのrootはそのlanguageとの一致\[いっち\]を検査\[けんさ\]する。親\[おや\]の埋\[う\]め込\[こ\]みoperandへ取\[と\]り込\[こ\]む場合\[ばあい\]はwrapper nodeを別\[べつ\]の意味子\[いみし\]として残\[のこ\]さず、元\[もと\]ForeignClosure・View・Originを保持\[ほじ\]する。GrammarGuestとDocGuestはform kindであり、独立\[どくりつ\]した文法\[ぶんぽう\]category名\[めい\]ではない。

NameとLangはreaderと共通\[きょうつう\]の `nepl3_core::lexical` の規則\[きそく\]で印字可能性\[いんじかのうせい\]を検査\[けんさ\]する。NameはUnicode 16 XIDと先頭\[せんとう\]underscore、Langは既存\[きそん\]readerのRFC 5646節\[せつ\]2\.1 ABNFでありregistry上\[じょう\]の登録\[とうろく\]や重複\[ちょうふく\]variant\/singletonの追加制約\[ついかせいやく\]を意味\[いみ\]しない。source\-less意味値\[いみち\]の定義域\[ていぎいき\]をこの表層規則\[ひょうそうきそく\]へ狭\[せば\]めず、表\[あらわ\]せない値\[あたい\]をUnprintableName \/ UnprintableLanguageとして返\[かえ\]す。名前\[なまえ\]の正規化\[せいきか\]・置換\[ちかん\]・本文\[ほんぶん\]からの位置推測\[いちすいそく\]は行\[おこな\]わない。

GuestBindingは `{schema,category,language}`。標準\[ひょうじゅん\]wrapperに対応\[たいおう\]するcategoryはMath→Expr、Circuit→Design、Grammar→Root、Doc→Articleとする。schemaからaliasを推測\[すいそく\]しない。同\[おな\]じschemaまたは同\[おな\]じlanguage（表層\[ひょうそう\]alias）への複数\[ふくすう\]binding、カテゴリ不整合\[ふせいごう\]、使用\[しよう\]するguestのbinding欠落\[けつらく\]を型付\[かたつ\]き失敗\[しっぱい\]とする。InlineMath\/DisplayMath\/CircuitFigureと独立\[どくりつ\]guest rootではslot\/languageの制約\[せいやく\]も検査\[けんさ\]する。提供\[ていきょう\]bindingは使用\[しよう\]の有無\[うむ\]によらず検査\[けんさ\]する。

PrintedGuestは `{documentDigest,embed,guestDigest,text}`。documentDigestは `SHA-256("NEPL3.Doc.Print.Document.v1\0" || canonical-NDF/1-CBOR(DocumentSyntax))`、guestDigestは `SHA-256("NEPL3.Doc.Print.Guest.v1\0" || canonical-NDF/1-CBOR(ForeignClosure))` とする。`\0` はゼロbyte、他\[ほか\]のdomain文字\[もじ\]はASCIIで、正式\[せいしき\]Doc\/foundation codecが返\[かえ\]すschema検査済\[けんさず\]み値\[あたい\]のSchemaRef digest、owner環境\[かんきょう\]、元\[もと\]Origin表\[ひょう\]、source\/map閉包\[へいほう\]を含\[ふく\]める。全提供\[ぜんていきょう\]entryの32byte digest、EmbedRefの存在\[そんざい\]、重複\[ちょうふく\]、両\[りょう\]identityを検査\[けんさ\]し、必要\[ひつよう\]な印字\[いんじ\]がなければUnresolvedGuestとする。

このdigestはtextがguest意味\[いみ\]と一致\[いっち\]することを証明\[しょうめい\]しない。通常\[つうじょう\]printの意味\[いみ\]roundtrip保証\[ほしょう\]は、hostが同\[おな\]じguestを扱\[あつか\]う実\[じつ\]printerの正\[ただ\]しい出力\[しゅつりょく\]を供給\[きょうきゅう\]することを前提\[ぜんてい\]とする。raw受信\[じゅしん\]やDoc coreはその意味\[いみ\]proofを発行\[はっこう\]せず、guestのlower・意味\[いみ\]check・評価\[ひょうか\]も呼\[よ\]ばない。原文取得\[げんぶんしゅとく\]helper `original_guest_source` はroot coverに対応\[たいおう\]する保持\[ほじ\]byteを返\[かえ\]すだけであり、coverがなければNoneとなる。構造検査済\[こうぞうけんさず\]みForeignClosureにも構文\[こうぶん\]と原文\[げんぶん\]の一致\[いっち\]proofはないため、通常\[つうじょう\]printはこのhelperを自動\[じどう\]fallbackとして使\[つか\]わない。host側\[がわ\]の統合試験\[とうごうしけん\]では実\[じつ\]guest parserとchecked tree、generic engine printerを通\[とお\]し、Doc→Docの意味不正\[いみふせい\]なCodeもguest構文\[こうぶん\]のまま保持\[ほじ\]する。

公開\[こうかい\]identity取得\[しゅとく\]はhost要求\[ようきゅう\]の準備\[じゅんび\]データであり、実行予算\[じっこうよさん\]を支払\[しはら\]い済\[ず\]みにするproofではない。公開\[こうかい\]printは同一\[どういつ\]Budget\/SourceAdmissionでDocumentSyntax再検査\[さいけんさ\]、canonical値\[あたい\]の生成\[せいせい\]・digest、全\[ぜん\]entry検査\[けんさ\]と出力\[しゅつりょく\]を合成\[ごうせい\]する。sourceの重複計上\[ちょうふくけいじょう\]を避\[さ\]け、反復処理\[はんぷくしょり\]で共有\[きょうゆう\]DAGの各表示経路\[かくひょうじけいろ\]を出力\[しゅつりょく\]し、そのWork・出力\[しゅつりょく\]byte・Allocation・Depthを計上\[けいじょう\]する。停止時\[ていしじ\]は元要求\[もとようきゅう\]を変更\[へんこう\]せず、準備中\[じゅんびちゅう\]も元\[もと\]StopReasonのStoppedとUsageを返\[かえ\]し、途中\[とちゅう\]textをCompleteへ昇格\[しょうかく\]させない。構造\[こうぞう\]\/schema不正\[ふせい\]は型付\[かたつ\]き入力境界\[にゅうりょくきょうかい\]エラー、受理後\[じゅりご\]の不適合\[ふてきごう\]はPrintFailureとなる。返信\[へんしん\]Reportの位置\[いち\]は明示要求\[めいじようきゅう\]documentの宣言\[せんげん\]source閉包\[へいほう\]だけを参照\[さんしょう\]でき、独立\[どくりつ\]reply codecにもそのdocumentを渡\[わた\]す。ambient補完\[ほかん\]と非\[ひ\]StoppedのtraceOverflowを拒否\[きょひ\]する。
