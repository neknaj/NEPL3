<!-- Generated from doc/spec/20&#45;doc&#45;html.nepld; renderer nepl3-tools.markdown-annotated/2; source SHA-256 5d0380d855e620f89140b34e01d0a0f26c20833f4874a655c5fe033523fb861d; alias input SHA-256 a89d4b749408c5f6e9ccc8ed5749c8203bf1a66d788d81725b0d44b7af3c82d5; document digest 238e57cfee192f84fe65ba698a4c909eb0d488ca84db9f3223151aad34e14d49. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="20-docのhtml変換"></a>

# 20\. DocのHTML変換\[へんかん\]

`nepl3-doc-html` はDoc coreとmarkupへ依存\[いぞん\]する純粋\[じゅんすい\]backendであり、ファイル・DOM・guest意味処理\[いみしょり\]を実行\[じっこう\]しない。言語中立\[げんごちゅうりつ\]の値\[あたい\]は `interfaces/doc-html.json` に定義\[ていぎ\]する。本章\[ほんしょう\]のlocal入口\[いりぐち\]は、外部要求\[がいぶようきゅう\]が一\[ひと\]つもないArticleをHTML fragmentへ変換\[へんかん\]する制約\[せいやく\]を持\[も\]つ。document shellと固定\[こてい\]CSSの配布\[はいふ\]は、後述\[こうじゅつ\]の開発\[かいはつ\]hostが担当\[たんとう\]する。一般\[いっぱん\]page\/asset\/foreign解決\[かいけつ\]と完全\[かんぜん\]なReport操作\[そうさ\]は、このlocal入口\[いりぐち\]の責務\[せきむ\]に含\[ふく\]めず、別\[べつ\]の統合経路\[とうごうけいろ\]で扱\[あつか\]う。

`LocalHtmlRequest` はDocumentSyntaxとRenderOptionsを所有\[しょゆう\]する。request codecはDocの構造\[こうぞう\]・source閉包\[へいほう\]を再検査\[さいけんさ\]し、optionsをraw値\[ち\]として復元\[ふくげん\]する。実行準備\[じっこうじゅんび\] `prepare_local` は、Docの要求発見\[ようきゅうはっけん\]とlabel検査\[けんさ\]を実行\[じっこう\]する。外部要求\[がいぶようきゅう\]があれば完全\[かんぜん\]なplanをNeedsResolutionとして返\[かえ\]し、空\[から\]fragmentへ変\[か\]えない。この拒否\[きょひ\]は、外部\[がいぶ\]URIの合法性\[ごうほうせい\]やMath式\[しき\]の意味\[いみ\]エラーを判定\[はんてい\]したものではない。要求\[ようきゅう\]がなければoptionsとlocal backend制約\[せいやく\]を検査\[けんさ\]し、対象\[たいしょう\]document\/optionsを借用\[しゃくよう\]する非公開\[ひこうかい\]constructorのPreparedLocalArticleを発行\[はっこう\]する。

ParallelModeは、Rows、Columns、Single\(language\,fallbacks\)のいずれかとする。Singleの言語\[げんご\]タグは共通\[きょうつう\]Lang規則\[きそく\]に従\[したが\]い、候補\[こうほ\]の重複\[ちょうふく\]はASCII case\-insensitiveで拒否\[きょひ\]する。各\[かく\]Parallelは、指定\[してい\]languageから明示\[めいじ\]fallback順\[じゅん\]に一致\[いっち\]するvariantを選\[えら\]び、存在\[そんざい\]しなければMissingVariantとする。空\[から\]Sentenceは、存在\[そんざい\]するvariantとして保持\[ほじ\]する。Rows\/Columnsは、元順\[もとじゅん\]の全\[ぜん\]variantを表示\[ひょうじ\]する。各表示出現\[かくひょうじしゅつげん\]のdata\-nepl\-groupは生成\[せいせい\]elementの番号\[ばんごう\]から決定\[けってい\]し、同\[おな\]じDoc nodeを複数表示\[ふくすうひょうじ\]しても出現\[しゅつげん\]を区別\[くべつ\]する。

Articleはclass nepl\-docと言語\[げんご\]を持\[も\]つarticle、titleはh1とする。SectionはUTF\-8 byteの小文字\[こもじ\]hexにn\-を付\[つ\]けたidと見出\[みだ\]しを持\[も\]ち、sectionの入\[い\]れ子\[こ\]ごとにlevelを増\[ふ\]やす。6を超\[こ\]える見出\[みだ\]しは、role\=heading\/aria\-levelのdivとする。Anchor\/Referenceも同\[おな\]じid写像\[しゃぞう\]を使用\[しよう\]する。Paragraphはdivとし、連続\[れんぞく\]Sentence\/Parallelをpへまとめ、子\[こ\]Blockは独立\[どくりつ\]div内\[ない\]へ出\[だ\]し、前後\[ぜんご\]のpを閉\[と\]じる。子\[こ\]の元順\[もとじゅん\]と空白\[くうはく\]を保持\[ほじ\]する。

Sentence\/Concatはspan、Emphasis\/Strongはem\/strong、明示\[めいじ\]Breakはbrとする。Ruby\/Annoは型付\[かたつ\]きspanと固定\[こてい\]CSSを使\[つか\]い、base\/reading\/全\[ぜん\]noteを別要素\[べつようそ\]として保持\[ほじ\]する。Tableはheaderのthead、順序付\[じゅんじょつ\]きtbody\/trとth\/td、列\[れつ\]alignmentの固定\[こてい\]classを使\[つか\]う。空表\[くうひょう\]は、空\[から\]tableのままにする。Listはul\/ol、itemはliとBody、明示\[めいじ\]checkboxは状態\[じょうたい\]を持\[も\]つ表示\[ひょうじ\]spanとする。HTMLのol\.start範囲\[はんい\]を超\[こ\]えるU64はListStartとして拒否\[きょひ\]し、丸\[まる\]めない。RawCodeはfigureの任意\[にんい\]captionにlanguageHintを保持\[ほじ\]し、pre\/codeには元\[もと\]Textを置\[お\]く。hintをparser実行要求\[じっこうようきゅう\]へ変\[か\]えない。

renderはHTML arenaを反復構築\[はんぷくこうちく\]し、全生成\[ぜんせいせい\]node・属性\[ぞくせい\]・文字列\[もじれつ\]・処理待\[しょりま\]ち・深\[ふか\]さをBudgetへ計上\[けいじょう\]する。全結果\[ぜんけっか\]を共通\[きょうつう\]markup validatorへ通\[とお\]す。本文\[ほんぶん\]\/属性\[ぞくせい\]のXML不適合\[ふてきごう\]やHTML内容不適合\[ないようふてきごう\]は、型付\[かたつ\]きMarkup失敗\[しっぱい\]とする。これらを削除\[さくじょ\]して成功\[せいこう\]にしない。出力\[しゅつりょく\]fragmentの深\[ふか\]さは256までとし、超過\[ちょうか\]はOutputDepthとして元\[もと\]Doc nodeを示\[しめ\]す。これはbrowser出力\[しゅつりょく\]profileの限界\[げんかい\]であり、入力意味\[にゅうりょくいみ\]モデルを平坦化\[へいたんか\]する規則\[きそく\]ではない。文書\[ぶんしょ\]shellは残\[のこ\]りの閲覧側深度\[えつらんがわしんど\]を含\[ふく\]めて検査\[けんさ\]し、実\[じつ\]browserで再確認\[さいかくにん\]する。

`RenderedFragment` はdocumentDigest、RenderOptions、HtmlRequest、ElementOrigin\(element\,node\)列\[れつ\]を保持\[ほじ\]する。elementは出力\[しゅつりょく\]arena、nodeは明示\[めいじ\]Doc arenaの番号\[ばんごう\]で、生成\[せいせい\]したText\/elementごとに一\[ひと\]つの原因\[げんいん\]を出力順\[しゅつりょくじゅん\]で保持\[ほじ\]する。元\[もと\]DocのSpan\/Origin\/source表\[ひょう\]を参照\[さんしょう\]し、架空\[かくう\]の生成\[せいせい\]source位置\[いち\]を作\[つく\]らない。固定\[こてい\]stylesheetはcrateのassets\/doc\.cssであり、HtmlPolicy自己申告\[じこしんこく\]やfragment単体\[たんたい\]を、CSS配布済\[はいふず\]みの証明\[しょうめい\]としない。

portable rendered値\[ち\]は、prepared document\/optionsを明示\[めいじ\]して受\[う\]け取\[と\]る。schema・markup・32byte digestを検査\[けんさ\]した後\[あと\]、同\[おな\]じbackendで再生成\[さいせいせい\]し、正式値\[せいしきち\]のcanonical CBORに `NEPL3.Doc.Html.Fragment.v1` とゼロbyteを前置\[ぜんち\]したdigestを比較\[ひかく\]する。改変\[かいへん\]markup、原因対応\[げんいんたいおう\]の欠落\[けつらく\]・変更\[へんこう\]、別\[べつ\]document\/optionsの結果\[けっか\]を、raw受信\[じゅしん\]したproofとして採用\[さいよう\]しない。これは現在\[げんざい\]の純粋\[じゅんすい\]local変換\[へんかん\]の再実行比較\[さいじっこうひかく\]であり、将来\[しょうらい\]の外部\[がいぶ\]rendererの授権\[じゅけん\]や完全\[かんぜん\]なArtifact検査\[けんさ\]を代行\[だいこう\]しない。全体\[ぜんたい\]の停止\[ていし\]・資源\[しげん\]Usageは、同\[おな\]じBudgetで単調\[たんちょう\]に保持\[ほじ\]する。

local入口\[いりぐち\]と要求発見\[ようきゅうはっけん\]だけで、T23やT21を完成\[かんせい\]にはしない。ページの外部\[がいぶ\]リンク・画像\[がぞう\]・foreign表現\[ひょうげん\]を含\[ふく\]む正式要求\[せいしきようきゅう\]を満\[み\]たしてから、16章\[しょう\]\/18章\[しょう\]に従\[したが\]って\.nepld正本\[せいほん\]へ切\[き\]り替\[か\]える。

<a name="n-6c6f63616c5f6578706f7274"></a>

<a name="開発hostによるローカル文書の書き出し"></a>

## 開発\[かいはつ\]hostによるローカル文書\[ぶんしょ\]の書\[か\]き出\[だ\]し

`nepl3-tools doc-html export <input.nepld> <new-directory>` は、共通\[きょうつう\]の標準\[ひょうじゅん\]Doc source処理\[しょり\]からlocal HTMLを生成\[せいせい\]し、`document.html`、`assets/doc.css`、`manifest.json` を新\[あたら\]しいディレクトリへ書\[か\]く。既存\[きそん\]の出力\[しゅつりょく\]は上書\[うわが\]きしない。入力\[にゅうりょく\]はUTF\-8、読取\[よみと\]り上限\[じょうげん\]は10\,000\,000 bytesとし、上限\[じょうげん\]を超\[こ\]えた内容\[ないよう\]を解析\[かいせき\]しない。生成\[せいせい\]を完了\[かんりょう\]してから出力\[しゅつりょく\]ディレクトリを作\[つく\]る。I\/O途中\[とちゅう\]の失敗\[しっぱい\]では不完全\[ふかんぜん\]なディレクトリが残\[のこ\]り得\[う\]るため、manifestは最後\[さいご\]に書\[か\]き、失敗\[しっぱい\]を成功\[せいこう\]にしない。

この入口\[いりぐち\]は開発用\[かいはつよう\]hostであり、検査済\[けんさず\]みbootstrap fixtureをGrammar compilerへ渡\[わた\]してpackageを構成\[こうせい\]する。正式\[せいしき\]CLI、生成\[せいせい\]package配布\[はいふ\]、suite、全資源解決\[ぜんしげんかいけつ\]の完成\[かんせい\]とは区別\[くべつ\]する。source読取\[よみと\]り\/検査\[けんさ\]、lower、prepare\/render\/serializeは、それぞれ明示\[めいじ\]した独立\[どくりつ\]Budgetを持\[も\]つ。manifestのUsageはその単位\[たんい\]で記録\[きろく\]し、文書全体\[ぶんしょぜんたい\]が一\[ひと\]つのBudgetで完走\[かんそう\]した記録\[きろく\]へ読\[よ\]み替\[か\]えない。ファイルI\/OとmanifestのJSON整形\[せいけい\]は、hostの責務\[せきむ\]である。

shellは固定\[こてい\]HTMLと検査済\[けんさず\]みfragmentだけから作\[つく\]り、stylesheetはbackendの同\[おな\]じ固定\[こてい\]byte列\[れつ\]を配布\[はいふ\]する。CSPは既定\[きてい\]の取得\[しゅとく\]・script・formを禁止\[きんし\]し、同\[おな\]じoriginのstylesheetだけを許可\[きょか\]する。manifestは入力\[にゅうりょく\]、Profile、Doc schema、生成\[せいせい\]HTML\/CSSのdigestと表示\[ひょうじ\]optionsを保持\[ほじ\]する。現在\[げんざい\]の形式\[けいしき\]はlocal export用\[よう\]の記録\[きろく\]であり、17章\[しょう\]のKaTeX資源\[しげん\]bundleや汎用\[はんよう\]Artifactの境界検査\[きょうかいけんさ\]を代行\[だいこう\]しない。HTTPの非\[ひ\]root pathとJavaScript無効\[むこう\]の実\[じつ\]browserで、生成物\[せいせいぶつ\]を検査\[けんさ\]する。外部\[がいぶ\]page\/asset\/foreignが必要\[ひつよう\]な文書\[ぶんしょ\]はNeedsResolutionのまま拒否\[きょひ\]し、リンクや埋\[う\]め込\[こ\]みを落\[お\]としたHTMLを書\[か\]き出\[だ\]さない。

<a name="n-626173656c696e65"></a>

<a name="固定cssのbaselineと対応範囲"></a>

## 固定\[こてい\]CSSのbaselineと対応範囲\[たいおうはんい\]

Rubyはreadingを第\[だい\]1行\[ぎょう\]、baseを第\[だい\]2行\[ぎょう\]とし、`baseline-source:last`でbase側\[がわ\]のbaselineを外\[そと\]へ公開\[こうかい\]する。Annoはbaseを第\[だい\]1行\[ぎょう\]、notesを第\[だい\]2行\[ぎょう\]とし、`baseline-source:first`を使\[つか\]う。空\[から\]の先頭行\[せんとうぎょう\]を挟\[はさ\]まない。これにより、入\[い\]れ子\[こ\]のbaseも内側要素\[うちがわようそ\]が公開\[こうかい\]するbaselineで整列\[せいれつ\]する。明示改行\[めいじかいぎょう\]を含\[ふく\]むbaseの場合\[ばあい\]、Rubyは最後\[さいご\]の行\[ぎょう\]、Annoは最初\[さいしょ\]の行\[ぎょう\]が整列基準\[せいれつきじゅん\]になる。高\[たか\]さを仮定\[かてい\]した固定\[こてい\]offsetやJavaScriptで合\[あ\]わせない。

この指定\[してい\]は [CSS Inline Layoutのbaseline\-source](<https\:\/\/drafts\.csswg\.org\/css\-inline\/\#baseline\-source>) と [Gridのbaseline規則\[きそく\]](<https\:\/\/www\.w3\.org\/TR\/css\-grid\-1\/\#grid\-baselines>) に基\[もと\]づく。Inline Layoutは草案\[そうあん\]であり、仕様\[しよう\]の存在\[そんざい\]だけを全\[ぜん\]browserでの実装証拠\[じっそうしょうこ\]にしない。生成文書\[せいせいぶんしょ\]の対応\[たいおう\]browserは、固定\[こてい\]CSSとの実試験\[じつしけん\]で確認\[かくにん\]する。幅\[はば\]が狭\[せま\]い場合\[ばあい\]にもRuby\/Annoの内部\[ないぶ\]を任意位置\[にんいいち\]で分割\[ぶんかつ\]せず、max\-contentの一\[ひと\]つのinline boxとして保持\[ほじ\]する。長\[なが\]いbase\/reading\/notesやpre内\[ない\]codeは横\[よこ\]にはみ出\[だ\]し得\[う\]るため、hostの閲覧\[えつらん\]・印刷\[いんさつ\]profileは、その表示\[ひょうじ\]と移動手段\[いどうしゅだん\]を別途検証\[べっとけんしょう\]する。

`baseline-source:first/last`の両方\[りょうほう\]を利用\[りよう\]できないbrowserでは、同\[おな\]じspan構造\[こうぞう\]にinline\-tableの固定\[こてい\]CSSを適用\[てきよう\]する。readingは上側\[うえがわ\]、notesは下側\[したがわ\]のtable\-captionとし、baseを持\[も\]つ唯一\[ゆいいつ\]の行\[ぎょう\]からbaselineを公開\[こうかい\]する。Rubyのbaseはinline\-blockとして最終行\[さいしゅうぎょう\]のbaselineを、Annoのbaseはtable\-cellとして最初\[さいしょ\]の行\[ぎょう\]のbaselineを公開\[こうかい\]する。DOMの順序\[じゅんじょ\]や注釈内容\[ちゅうしゃくないよう\]を変更\[へんこう\]せず、HTMLの表要素\[ひょうようそ\]やJavaScriptを追加\[ついか\]しない。この代替\[だいたい\]はWebKitで基底文字\[きていもじ\]が本文\[ほんぶん\]より下\[さ\]がる不具合\[ふぐあい\]への対応\[たいおう\]であり、型付\[かたつ\]きmarkup・意味正規形\[いみせいきけい\]は変更\[へんこう\]しない。

代替\[だいたい\]の根拠\[こんきょ\]は [CSS2のinline\-block baseline](<https\:\/\/www\.w3\.org\/TR\/CSS2\/visudet\.html\#leading>) と [tableのbaseline規則\[きそく\]](<https\:\/\/www\.w3\.org\/TR\/CSS2\/tables\.html\#height\-layout>) である。対応\[たいおう\]propertyの有無\[うむ\]だけで合否\[ごうひ\]を決\[き\]めず、実際\[じっさい\]のproduction HTMLとCSSをChromium・Firefox・WebKitで表示\[ひょうじ\]し、本文\[ほんぶん\]とのbaseline、読\[よ\]み・注釈\[ちゅうしゃく\]の上下配置\[じょうげはいち\]、複数行\[ふくすうぎょう\]の基準\[きじゅん\]、入\[い\]れ子\[こ\]と前後行\[ぜんごぎょう\]の高\[たか\]さ予約\[よやく\]を検査\[けんさ\]する。文書内\[ぶんしょない\]のJavaScriptは無効\[むこう\]とする。この静的文書\[せいてきぶんしょ\]の検査\[けんさ\]はWasm・Playground・支援技術\[しえんぎじゅつ\]による実操作\[じつそうさ\]の受入\[うけいれ\]を代行\[だいこう\]しない。
