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

Article以外のBody、Block、Flow、Sentence、Inline、Variant、Row、ListItemも明示 `DocRoot` として単独のfragmentを構成できる。Alignment、ListStyle、Check、LinkTarget、Asset、OptionalRow、OptionalSentence、OptionalTextは表層の固定arity補助constructorである。単独lower時には対応するarena wrapperをrootとする。親constructorのoperandである場合はAlignment、ListKind、Option、LinkTarget、AssetRefという型付き値へ取り込み、補助wrapperを意味的な子として残さない。元operandのView、Origin、source byte宣言はこの取り込みで破棄しない。Optionやlistの暗黙文法は導入せず、signatureにある `none` / `some`、`cons` / `nil` を読む。

arenaの構造検査はroot/childカテゴリ、index範囲、循環、到達性、注釈内容、表の列数等を検査する。共有DAGを許すが全経路の最大Depthを検査し、ForeignClosureの内側深さもその所有nodeまでの深さに合成する。この構造proofはlabel解決、guest意味check、PreparedArticleを意味しない。tokenごとのViewは `DocView.head` に束縛し、ViewRefとrelationのID空間をtoken間で混ぜない。Text正規化後も元ViewとOriginを保存する。sourceを持つTextとsource-less Textを結合する場合、後者を明示Synthetic OriginとしてCompositeに含め、既知spanを全体のspanに偽装しない。

### 1.2 文書入力に必要な要素

DG01のTableは列ごとのDefault/Left/Center/Right、任意header、順序付きRowを持つ。全Rowのcell数は列数と一致する。cellはSentenceであり空Sentenceも明示値として許す。ゼロ列のTableは構造上の空表として表現可能で、内容のある列を補わない。

DG02のListはUnorderedまたはOrdered(start:Nat)を明示し、ListItemは任意のchecked状態とBodyを持つ。Bodyを介した入れ子を保持する。Orderedのstartは公開arenaのU64で表せる範囲をlower時に検査し、範囲外を丸めない。

DG03のLinkはPage(page,fragment)、Relative(path,fragment)、External(uri)を区別し、表示labelを保持する。DG06のSection/Anchor/Referenceのarticle内IDとは区別し、ページ名、外部URI、相対pathを同じ名前空間で自動解決しない。公開URIへの写像とリンク解決はcheck/prepare側の明示入力であり、構造値だけからファイル読出し権限や安全なHTML属性を得ない。

DG04のInlineCodeとRawCodeは意味解析しないTextを保持する。RawCodeのlanguageHintは任意の表示情報で、parserや評価器の自動実行要求ではない。DG05のImage/InlineImageはAssetRef(id,任意digest)とaltを持ち、block画像は任意captionを持つ。表層のdigestはOptionalTextの64桁hexを32byteのDigestへlowerし、無効な桁数・文字を拒否する。assetの読出し、digest照合、表示準備は別の解決操作で行う。RawHtmlは追加しない。

この要素選択はmain `b5295cef655aa59affffd6644f2902268d071953` の文書入力監査（inventory SHA-256 `daf94085913930f05c1655d2adbef4f56651864f9a97ac4d261400449c98499e`）に基づく。61 Markdown、52表/1283cell、39list/233item、1134 inline code、12 code block、249link、1imageを含む。監査は実装中差分やrustdocの意味監査の完了を示さず、実装済み操作は `implementation-status.json` と実行済み受入で区別する。

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

意味値からliteralを出力するprinterはText/Ruby/Annoだけの表現ならescapeを行ってliteral化できる。他のInlineがあればprefix構文を出力する。内容を削除して無理にliteralにしない。どちらでも再parse/lower後の意味が一致することを保証する。

## 4. parallelの単位

Sentenceは著者が明示した対応単位であり、句読点を自動検出した自然言語学的な文ではない。literal中に句点が二つあっても一つのSentence。二つのSentenceに対応させたい場合は著者が別ノードにする。

Parallelはvariantのlanguageがcase-insensitiveで重複しないこと、2variant以上であることを検査する。空Sentenceは翻訳が空であるという明示値として許すが、missingと推測して他言語から補わない。並び順はソース順。各variantにCorrespondsTo relationを与え、名前束縛の同一性にはしない。

paragraphのネスト、節番号、HTMLの行折返しはこの対応を変更しない。単一言語表示はRenderOptionsの指定languageに一致するvariantのみを選ぶ。なければMissingVariantを返すか、明示されたFallbackLanguage順で選ぶ。暗黙の先頭選択はしない。

## 5. ラベルと参照

Section.idとInline Anchor.idはarticleのDocLabel空間へ定義をexportする。ネストしたparagraph/sectionも同じarticle内で集める。重複はエラー。Reference.targetはarticle内から解決し、前方参照を認める。別articleやforeign言語へ暗黙にscopeを広げない。

source上の名前の選択範囲と、定義全体の範囲を分ける。Referenceは表示labelを持ち、参照先の見出し文字列を自動コピーしない。HTML idは `n-` + UTF-8 byteの小文字hexとし、任意の名前から衝突なく生成する。

## 6. 埋め込み

InlineMath / DisplayMath / CircuitFigure / Codeは、スロット種別とForeignClosureを保持する。ForeignClosureはForeignSyntaxに選択済みowner環境、元Origin表、source/map宣言閉包を加えた共通型である。環境digestに含まれる元Origin IDを保存し、guest自身のOrigin表と混同しない。MathやCircuitの型をdoc-coreへimportしない。

suiteがMathの構造・bindingをcheckし、math-mathmlからsafeなMathML subtreeを作る。CircuitFigureはcheck/elaborateとdiagramを使う。Codeは元のguest sourceとviewの表示であり、guestのlower・意味check・compile・evaluateを実行しない。guestがDoc自身の場合もDoc:DocGuest.syntaxはForeignClosure中のForeignSyntaxを保持し、Doc/Articleの意味値への変換を表示の前提にしない。bundleのschema・参照・source範囲の安全性検査は省略しない。

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
plain_text(Sentence, AnnotationPolicy) -> Text。
print(DocumentSyntax, Prefix|Compact) -> SourceArtifact。

AnnotationPolicyはBaseOnly、WithReadings、WithAllNotesを明示する。テキスト抽出時に隠れた翻訳選択を行わない。各constructorはRust APIとportable record constructorの双方から呼べる。
