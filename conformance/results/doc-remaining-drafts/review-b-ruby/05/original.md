# 05. Doc言語

## 方針

本文の階層、文単位の翻訳対応、inlineの注釈を独立した構造として扱う。sentence literalとprefix constructorを同じ意味モデルへlowerする。source表記は別に保持する。

## 1. 主要な型

Article(language, title:Sentence, body:Body)。BodyはBlockの列。BlockはParagraph、Section、DisplayMath、CircuitFigure、Code、Table、List、RawCode、Image。
ParagraphはFlowの列。FlowはSentence、ParallelとBlockの各variant。したがってParagraphを再帰的に含められる。
SentenceはInlineの列。Parallelは2個以上のVariant(language, sentence)の列。VariantへParagraphやParallelは入れられない。
InlineはText、Concat、Ruby(base,reading)、Anno(base,notes)、InlineMath、Anchor(id,label)、Reference(target,label)、Emphasis、Strong、Break、Link、InlineCode、InlineImage。

全signatureはdoc-signaturesを参照。bare quoted文字列をSentence/Flowの位置で読むとSentenceLiteralになる。`text` の子は通常Textであり、その内部の `[` や `{` を注釈と解釈しない。

### 1.1 意味モデルと公開arena

`interfaces/model.json` の `Doc:*` recordと `Doc/*` unionはconstructorの論理的な意味展開であり、独立した再帰wire layoutではない。Doc操作で送受信する値の正本は `interfaces/doc.json` の `DocumentSyntax` / `DocValue`。`DocValue.nodes` の各 `DocNode.kind` がconstructorを表し、子はカテゴリ別のindex参照を使う。Rust enumの並びをwire tagへ転用せず、schemaの明示variant名とfield列を対応させる。

Article以外のBody、Block、Flow、Sentence、Inline、Variant、Row、ListItem、MathGuest、CircuitGuest、Guestも明示 `DocRoot` として単独のfragmentを構成できる。Alignment、ListStyle、Check、LinkTarget、Asset、OptionalRow、OptionalSentence、OptionalTextは表層の固定arity補助constructorである。単独lower時には対応するarena wrapperをrootとする。親constructorのoperandである場合はAlignment、ListKind、Option、LinkTarget、AssetRefという型付き値へ取り込み、補助wrapperを意味的な子として残さない。元operandのView、Origin、source byte宣言はこの取り込みで破棄しない。Optionやlistの暗黙文法は導入せず、signatureにある `none` / `some`、`cons` / `nil` を読む。

arenaの構造検査はroot/childカテゴリ、index範囲、循環、到達性、注釈内容、表の列数等を検査する。共有DAGを許すが全経路の最大Depthを検査し、ForeignClosureの内側深さもその所有nodeまでの深さに合成する。この構造proofはlabel解決、guest意味check、PreparedArticleを意味しない。tokenごとのViewは `DocView.head` に束縛し、ViewRefとrelationのID空間をtoken間で混ぜない。Text正規化後も元ViewとOriginを保存する。sourceを持つTextとsource-less Textを結合する場合、後者を明示Synthetic OriginとしてCompositeに含め、既知spanを全体のspanに偽装しない。

### 1.2 文書入力に必要な要素

DG01のTableは列ごとのDefault/Left/Center/Right、任意header、順序付きRowを持つ。全Rowのcell数は列数と一致する。cellはSentenceであり空Sentenceも明示値として許す。ゼロ列のTableは構造上の空表として表現可能で、内容のある列を補わない。

DG02のListはUnorderedまたはOrdered(start:Nat)を明示し、ListItemは任意のchecked状態とBodyを持つ。Bodyを介した入れ子を保持する。Orderedのstartは公開arenaのU64で表せる範囲をlower時に検査し、範囲外を丸めない。

DG03のLinkはPage(page,fragment)、Relative(path,fragment)、External(uri)を区別し、表示labelを保持する。DG06のSection/Anchor/Referenceのarticle内IDとは区別し、ページ名、外部URI、相対pathを同じ名前空間で自動解決しない。公開URIへの写像とリンク解決はcheck/prepare側の明示入力であり、構造値だけからファイル読出し権限や安全なHTML属性を得ない。

DG04のInlineCodeとRawCodeは意味解析しないTextを保持する。RawCodeのlanguageHintは任意の表示情報で、parserや評価器の自動実行要求ではない。DG05のImage/InlineImageはAssetRef(id,任意digest)とaltを持ち、block画像は任意captionを持つ。表層のdigestはOptionalTextの64桁hexを32byteのDigestへlowerし、無効な桁数・文字を拒否する。assetの読出し、digest照合、表示準備は別の解決操作で行う。RawHtmlは追加しない。

この要素選択はmain `b5295cef655aa59affffd6644f2902268d071953` の文書入力監査（inventory SHA-256 `daf94085913930f05c1655d2adbef4f56651864f9a97ac4d261400449c98499e`）に基づく。61 Markdown、52表/1283cell、39list/233item、1134 inline code、12 code block、249link、1imageを含む。監査は実装中差分やrustdocの意味監査の完了を示さず、実装済み操作は `implementation-status.json` と実行済み受入で区別する。

### 1.3 文書準備の要求発見

`prepare::inspect` はArticleの構造・source閉包・article内labelを検査し、`DocPreparationPlan` を返す。これは必要な外部入力の列挙であり、完全なPreparedArticleではない。全Parallel variantを対象とし、表示言語の選択やguest操作を実行しない。

planは `documentDigest` と順序付き `requirements` を持つ。Linkは意味node indexと元LinkTarget、Assetは意味node indexと元AssetRef、ForeignはEmbedRef・EmbedKind・guestDigestを持つ。Link/Assetはarena順に一度ずつ列挙し、その後owner embed表順にForeignを列挙する。同じ資源を参照する別nodeは別の要求であり、共有nodeの表示出現ごとには増やさない。Codeも構文のままForeignとして保持し、意味エラーを含む例の表示を可能にする。

documentDigestは `SHA-256("NEPL3.Doc.Prepare.Document.v1\0" || canonical-NDF/1-CBOR(DocumentSyntax))`、guestDigestは `SHA-256("NEPL3.Doc.Prepare.Guest.v1\0" || canonical-NDF/1-CBOR(ForeignClosure))`。domainの `\0` はゼロbyte、digestは32byte。owner環境・source・Originを含め、型名やURIだけをidentityにしない。Doc nodeとOrigin IDは保持し、共通codecのsource表順・guest NodeRef正準化に従う。

portable planの送受信には対象DocumentSyntaxを明示し、同じ検査と要求列挙を再実行して全fieldを比較する。schema-validでも古い文書、欠落/追加/重複/順序違い、異なるnode/guest/digestを拒否する。source admissionとBudgetを操作内で共有し、停止は元StopReasonを保持する。native helperのPreparationErrorは構造/label/boundary/停止を区別するが、完全なcheck/prepareのReport操作包絡として広告しない。

この発見段階はURIの安全性、page/fragmentの存在、asset byte列・MIME・digestの適合、guest生成物や表示言語の準備を証明しない。後続prepareで明示資源・解決結果を同じdocument/guest identityへ束縛し、必要条件がすべて満たされてからHTML backendへ渡す。

## 2. sentence literalの完全な規則

説明用EBNF:

```text
SentenceLiteral = '"' InlineSequence '"'
InlineSequence  = InlineItem*
InlineItem      = TextRun | Escape | Ruby | Anno
Ruby            = '[' NonemptySequence '/' NonemptySequence ']'
Anno            = '{' NonemptySequence ('/' NonemptySequence)+ '}'
```

InlineSequenceの終端集合は呼出し位置で決まる。topでは未escapeの `"`、Rubyのbaseでは `/`、readingでは `]`、Annoの各fieldでは `/` または `}`。入れ子のRuby/Annoは一つのInlineItemとして読むため、内側の `/` を外側の区切りにしない。

未escapeの `[` と `{` は常に注釈開始。対応しない `]` / `}` はエラー。topの `/` は通常文字。注釈field内のliteral `/` は `\/` と書く。Rubyはtop-level separatorがちょうど一つ。Annoは一つ以上。

escapeは通常Textのものに加え `\[`、`\]`、`\{`、`\}`、`\/`。escapeの出力を改めて注釈開始として解析しない。直接のCR/LFは禁止、`\n` は内容の改行。EOF・改行・終了引用符に達した時点で閉じていない注釈は、その開始位置をrelatedに持つUnclosedAnnotation。

Nonemptyはlower後に可視内容が存在すること。空Textや空Concatだけのbase/reading/noteはEmptyAnnotationPart。Annoのnotesは非空。Sentence自体は空を許す。

ここで可視内容はfontや描画幅に依存させず、保持する内容で判定する。Text（および明示inline code）は非空なら内容を持ち、空白だけのTextやescapeで生成した改行も含める。明示Break、InlineMath、inline画像は表示要素として内容を持つ。Concatは子のいずれか、装飾・Anchor・Reference・Linkはlabel/子、Ruby/Annoはbaseの内容を用いる。空要素を隠れた空白へ補うことはない。literal、prefix、portable constructorは同じ判定を用いる。

例: `"これは{[文書/ぶんしょ]/document}を記述する。"` はText + Anno(Ruby(...),[Text]) + Text。

### 2.1. 文書構造の明示改行

前置構文の `break`（Doc/Inline、arity 0）はDoc.Breakを構築する。同じSentence内の明示的な改行であり、新しいSentence・Paragraph・Parallel variantを作らない。例えば `sentence cons text "a" cons break cons text "b" nil` と書く。HTMLは`br`、plain_textはLFを出力し、source printerは`break`を保持する。

著者が指定する文書の改行にはbreakを用いる。Text中の`\n`は文字データのLFとして保持する既存escapeであり、Break nodeを生成しない。literalにはBreak constructorを追加せず、明示改行を含むSentenceは前置構築を使う。TextのLF、明示Break、Paragraph境界、表示時の自動折返しを相互に推測変換しない。

## 3. prefix経路との等価性

```text
sentence
  cons text "これは"
  cons anno
    ruby text "文書" text "ぶんしょ"
    cons text "document" nil
  cons text "を記述する。"
  nil
```

上例と前節のliteralは、Origin/sourceを除いた意味正規形が等しい。正規化はConcatの平坦化、空Textの除去、同じinline列内の隣接Textの結合だけ。Ruby/Annoの境界、noteの順、明示Break、Math等の埋め込みは保存する。

受理済みSentenceLiteralとprefix constructorの混在lowerでは、literal payloadを明示DocumentSyntax schemaとpayload自身のsource宣言で検査する。payloadの単一View owner/headと局所Viewは外tokenのものと一致し、意味nodeのSpanとOriginの位置は同じtoken head内、または明示SourceMapの全経路でその範囲内へ戻らなければならない。別tokenのpayloadや、Viewだけを合わせて意味位置を別source・別範囲へ移した値は拒否する。元テキストを再parseして意味値を推測し直さない。外側構文のOrigin IDとtoken-local View IDは保持し、literal内のOrigin参照だけを結合後のarenaへ再配置する。ForeignClosureのowner-local Originと環境digestにはこの再配置を適用しない。source admissionは操作内で共有し、payload宣言の欠落をambient source storeから補わない。

意味値からliteralを出力するprinterはText/Ruby/Annoだけの表現ならescapeを行ってliteral化できる。他のInlineがあればprefix構文を出力する。内容を削除して無理にliteralにしない。どちらでも再parse/lower後の意味が一致することを保証する。

## 4. parallelの単位

Sentenceは著者が明示した対応単位であり、句読点を自動検出した自然言語学的な文ではない。literal中に句点が二つあっても一つのSentence。二つのSentenceに対応させたい場合は著者が別ノードにする。

Parallelはvariantのlanguageがcase-insensitiveで重複しないこと、2variant以上であることを検査する。空Sentenceは翻訳が空であるという明示値として許すが、missingと推測して他言語から補わない。並び順はソース順。各variantにCorrespondsTo relationを与え、名前束縛の同一性にはしない。

paragraphのネスト、節番号、HTMLの行折返しはこの対応を変更しない。単一言語表示はRenderOptionsの指定languageに一致するvariantのみを選ぶ。なければMissingVariantを返すか、明示されたFallbackLanguage順で選ぶ。暗黙の先頭選択はしない。

## 5. ラベルと参照

Section.idとInline Anchor.idはarticleのDocLabel空間へ定義をexportする。ネストしたparagraph/sectionも同じarticle内で集める。重複はエラー。Reference.targetはarticle内から解決し、前方参照を認める。別articleやforeign言語へ暗黙にscopeを広げない。

source上の名前の選択範囲と、定義全体の範囲を分ける。Referenceは表示labelを持ち、参照先の見出し文字列を自動コピーしない。HTML idは `n-` + UTF-8 byteの小文字hexとし、任意の名前から衝突なく生成する。

`DocNode.locations` は名前operandの位置を `DocFieldLocation` として保持する。fieldはDoc所有の閉じた種別SectionId / AnchorId / ReferenceTargetで、対応するDocKindだけに指定でき、同じfieldの重複を許さない。lowerは元の型付きchild/tokenからSpanとOriginを取得し、意味Stringの綴りを本文から検索して位置を作らない。source-less constructorでは空のlocationsまたはNoneの位置を保持する。存在するSpan/Originは宣言済みsource/Origin arenaを参照し、Spanは指定された定義全体cover内へ明示SourceMapを通じて包含される必要がある。Originも指定した場合は、その明示causeが選択Spanを支えることを検査する。構造検査と初回NDF decodeに同じ条件を適用し、元tokenの局所Viewは変更しない。

意味nodeの共有と表示上のanchorの一意性は別である。同じSection/Anchor nodeへArticle rootから複数の表示経路がある場合はDuplicateOccurrenceとする。直接同じ子を二度参照する場合と、共有parentを介する場合の双方を含む。labelを含まないDAG共有は許容する。暗黙のID複製・改名では回避しない。経路数を2で飽和させる予算付き検査を行い、同じsource selectionを持つ二つの出現は、owner node・子の順序index・target nodeを並べた `LabelOccurrencePaths` で識別する。この型付き経路を通常Diagnosticの `LabelDiagnosticArguments` に含め、架空のSpanを追加しない。foreign guestの構造はこのArticleの表示経路として展開せず、そのlabelを暗黙に定義・参照へ取り込まない。

## 6. 埋め込み

InlineMath / DisplayMath / CircuitFigure / Codeは、スロット種別とForeignClosureを保持する。ForeignClosureはForeignSyntaxに選択済みowner環境、元Origin表、source/map宣言閉包を加えた共通型である。環境digestに含まれる元Origin IDを保存し、guest自身のOrigin表と混同しない。MathやCircuitの型をdoc-coreへimportしない。

suiteがMathの構造・bindingをcheckし、[17章](17-math-html.md)の生成policyに従ってhostによるKaTeX生成、または独立math-mathmlの検査済みfragmentを準備する。閲覧時にKaTeXを再実行しない。CircuitFigureはcheck/elaborateとdiagramを使う。Codeは元のguest sourceとviewの表示であり、guestのlower・意味check・compile・evaluateを実行しない。guestがDoc自身の場合もDoc:DocGuest.syntaxはForeignClosure中のForeignSyntaxを保持し、Doc/Articleの意味値への変換を表示の前提にしない。bundleのschema・参照・source範囲の安全性検査は省略しない。

標準Doc文法の `DocGuest.syntax` は `foreign Doc Article` を読む。表層の `Doc article ...` は変わらないが、同aliasへの入れ子でも独立guest bundleと選択された環境を保持し、終了後はhostの読取contextへ復帰する。Profileには標準alias `Doc` の登録が必要である。hostを別aliasへ登録してもforeign先をそのaliasへ暗黙に置換しない。同じpackageを `Doc` として明示登録するか、別名を指定する変更済み文法/packageを明示選択する。Code用Doc guestが意味的に不正な注釈やlabelを含んでも、構文が成立している限り表示のためにguestのDoc lowerを呼んで拒否しない。

Docのcheckは通常構造・注釈・labelを検査し、foreign slotにはRequirementを返す。suiteが要求を解決した `PreparedArticle` だけをHTML backendへ渡す。未解決slotの空表示や文字列化による偽成功は禁止。

## 7. HTML backend

Articleはarticle要素、titleはh1。Sectionはsection要素と適切な見出し。深さ6超はrole=headingとaria-levelを持つ要素を使う。Paragraphを機械的に入れ子のpへ変換しない。Paragraphはdiv構造、連続したSentence/Parallelのrunをpへまとめ、子Blockはrunを閉じてから出す。

Sentence間へ空白を勝手に挿入しない。必要な空白はTextに含める。white-space:pre-wrapで著者の空白と明示改行を尊重する。

Ruby/Annoはネスト可能なinline-gridのtyped span構造として描画し、base、上側のreading、下側のnotesを別セルへ置く。単純RubyにはHTML ruby/rtを使用してもよいが、semantic HTML正規形のreference backendはinline-gridへ統一する。CSSは配布asset、リモートfont/CDN/JSは不要。

Parallelは一つのalignment wrapperにlanguageごとのsentence spanを入れる。横並び・縦並び・単一言語はRenderOptionsとして変える。DOM上に対応関係IDを保存する。

全Text/属性をmarkup serializerでescapeする。RawHtml variantはDocに存在しない。外部HTMLを表示するextensionは別の明示的なtrust契約を要求する。token化されたHTMLをそのままinnerHTMLへ渡してはならない。

## 8. 公開操作

lower(Parsed, Profile) -> DocumentSyntax + diagnostics。
check(DocumentSyntax, LabelEnvironment) -> CheckedArticle + foreign Requirements。
prepare(CheckedArticle, ResolvedEmbeds) -> PreparedArticle。
render(PreparedArticle, RenderOptions) -> HtmlArtifact。
plain_text(PlainTextRequest) -> PlainTextReply。
print(PrintRequest) -> PrintReply。

AnnotationPolicyはBaseOnly、WithReadings、WithAllNotesを明示する。テキスト抽出時に隠れた翻訳選択を行わない。各constructorはRust APIとportable record constructorの双方から呼べる。

### 8.1. plain_text の実行契約

実操作descriptorは `nepl3.doc@1` の `plainText: PlainTextRequest -> PlainTextReply`（pure）である。要求は宣言source閉包を持つDocumentSyntax、対象SentenceRef、AnnotationPolicy、明示ResolvedInlineText列を所有する。SentenceRefはそのdocument内のSentenceを指す必要があり、別categoryはExpectedSentenceとなる。schemaとDocumentSyntax構造の不正は入力境界の型付きエラーである。受理済み要求の名前付き失敗はPlainTextFailureで返し、停止は準備段階も含めStoppedと元StopReasonを返す。成功時だけComplete.textを返す。Reportは同一操作のUsageを持ち、その位置参照は元要求documentの宣言source閉包に限る。独立返信codecにも元要求documentを明示して渡し、ambient sourceを補完しない。非StoppedとtraceOverflowの組合せを拒否する。

BaseOnlyはRubyとAnnoのbaseだけを投影する。WithReadingsはRubyを `base[reading]` とし、Annoはbaseだけを投影する。WithAllNotesはRubyも同じとし、Annoを `base{note1/note2}` とする。入れ子にも同じpolicyを適用する。TextとInlineCodeの文字は変更せず、BreakはLFを一つ出力する。装飾は子inline、Anchor/Reference/Linkは明示label、InlineImageは明示alt Sentenceを投影する。Sentence/Concatの子間に空白を追加しない。この出力は再parse用sentence literalではなく、角括弧・波括弧・slashをescapeし直さない。任意separatorのoptionsは設けない。

InlineMathは暗黙にlower・意味check・評価しない。必要な表示textはhostが `ResolvedInlineText{documentDigest,embed,guestDigest,text}` として提供する。documentDigestは `SHA-256("NEPL3.Doc.PlainText.Document.v1\0" || canonical-NDF/1-CBOR(DocumentSyntax))`、guestDigestは `SHA-256("NEPL3.Doc.PlainText.Guest.v1\0" || canonical-NDF/1-CBOR(ForeignClosure))` とする。引用内の `\0` は一つのゼロbyte、他はASCII byteである。値はそれぞれDocの正式DocumentSyntax codecとfoundationの正式ForeignClosure codecが返すschema検査済み値であり、型を参照するSchemaRefのdigestもcanonical CBORへ含める。guest側はowner環境・元Origin列・source/map閉包も含み、局所EmbedRefだけの一致で別guestや古い環境のtextを再利用しない。

全提供entryについて32byte digest、対象InlineMath、重複EmbedRef、現在document/guest両identityとの一致を検査する。policyがそのentryを表示しない場合も不正entryを黙殺しない。一方、未提供textによるUnresolvedEmbedは選択policyで実際に投影するInlineMathだけに適用し、BaseOnlyで隠れるreading/notesのtextを要求しない。hostのtextは明示データであり、guest意味checkが成功した証明ではない。

identity取得のprepareは独立した操作であり、後の実行予算を支払い済みとするproofではない。公開実行入口plain_textは毎回同一Budget/SourceAdmissionでdocument検査、canonical値生成とhash、entry検査、出力を合成する。sourceは共有admissionで同snapshotを一度だけ計上し、出力byte、Work、Allocation、深さを計上する。停止時は元要求を変更せず、不完全なTextをCompleteへ昇格させない。

### 8.2. print の実行契約

実descriptorは `nepl3.doc@1` の `print: PrintRequest -> PrintReply`（pure）。PrintRequestはDocumentSyntax、PrintMode（Prefix / Compact）、明示GuestBinding列、PrintedGuest列を所有する。Completeだけが `SourceArtifact{text,entry}` を返す。entryは再parseする具体的なDoc表層categoryであり、hostがtext保存時のSourceId・revision・URIとSourceSnapshotを発行する。printerが架空のsnapshotや元source位置を作ることはない。再parse/lowerとの一致はsource/Originを除いた意味正規形について定め、未正規化のconstructor値を変更して入力へ書き戻さない。

Prefixは全constructorを正式なheadと引数順で出力する。CompactはSentence内がText / Concat / Ruby / Annoだけで表せる場合にsentence literalへし、それ以外はprefixを保持する。Textは引用符・backslash・CR・LF・tabをescapeし、literalでは注釈delimiterもescapeする。escapeの生成文字を再び注釈と解釈しない。明示BreakはTextの改行と異なる意味nodeなので、Breakを含むSentenceをliteralへ変換しない。装飾・link・画像・Mathを省略してliteral化することもない。

補助fragmentを含む表層entryは20種類である。guestの具体entryはMathGuest / CircuitGuest / Guestの3種類、wrapperはMath / Circuit / Grammar / Docの4種類。独立rootでは `DocKind.Guest{language,syntax}` とEmbedKind.Guestを保持し、MathGuest/CircuitGuestのrootはそのlanguageとの一致を検査する。親の埋め込みoperandへ取り込む場合はwrapper nodeを別の意味子として残さず、元ForeignClosure・View・Originを保持する。GrammarGuestとDocGuestはform kindであり、独立した文法category名ではない。

NameとLangはreaderと共通の `nepl3_core::lexical` の規則で印字可能性を検査する。NameはUnicode 16 XIDと先頭underscore、Langは既存readerのRFC 5646節2.1 ABNFでありregistry上の登録や重複variant/singletonの追加制約を意味しない。source-less意味値の定義域をこの表層規則へ狭めず、表せない値をUnprintableName / UnprintableLanguageとして返す。名前の正規化・置換・本文からの位置推測は行わない。

GuestBindingは `{schema,category,language}`。標準wrapperに対応するcategoryはMath→Expr、Circuit→Design、Grammar→Root、Doc→Articleとする。schemaからaliasを推測しない。同じschemaまたは同じlanguage（表層alias）への複数binding、カテゴリ不整合、使用するguestのbinding欠落を型付き失敗とする。InlineMath/DisplayMath/CircuitFigureと独立guest rootではslot/languageの制約も検査する。提供bindingは使用の有無によらず検査する。

PrintedGuestは `{documentDigest,embed,guestDigest,text}`。documentDigestは `SHA-256("NEPL3.Doc.Print.Document.v1\0" || canonical-NDF/1-CBOR(DocumentSyntax))`、guestDigestは `SHA-256("NEPL3.Doc.Print.Guest.v1\0" || canonical-NDF/1-CBOR(ForeignClosure))` とする。`\0` はゼロbyte、他のdomain文字はASCIIで、正式Doc/foundation codecが返すschema検査済み値のSchemaRef digest、owner環境、元Origin表、source/map閉包を含める。全提供entryの32byte digest、EmbedRefの存在、重複、両identityを検査し、必要な印字がなければUnresolvedGuestとする。

このdigestはtextがguest意味と一致することを証明しない。通常printの意味roundtrip保証は、hostが同じguestを扱う実printerの正しい出力を供給することを前提とする。raw受信やDoc coreはその意味proofを発行せず、guestのlower・意味check・評価も呼ばない。原文取得helper `original_guest_source` はroot coverに対応する保持byteを返すだけであり、coverがなければNoneとなる。構造検査済みForeignClosureにも構文と原文の一致proofはないため、通常printはこのhelperを自動fallbackとして使わない。host側の統合試験では実guest parserとchecked tree、generic engine printerを通し、Doc→Docの意味不正なCodeもguest構文のまま保持する。

公開identity取得はhost要求の準備データであり、実行予算を支払い済みにするproofではない。公開printは同一Budget/SourceAdmissionでDocumentSyntax再検査、canonical値生成・digest、全entry検査と出力を合成する。sourceの重複計上を避け、反復処理で共有DAGの各表示経路を出力し、そのWork・出力byte・Allocation・Depthを計上する。停止時は元要求を変更せず、準備中も元StopReasonのStoppedとUsageを返し、途中textをCompleteへ昇格させない。構造/schema不正は型付き入力境界エラー、受理後の不適合はPrintFailureとなる。返信Reportの位置は明示要求documentの宣言source閉包だけを参照でき、独立reply codecにもそのdocumentを渡す。ambient補完と非StoppedのtraceOverflowを拒否する。
