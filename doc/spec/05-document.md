# 05. Doc言語

## 方針

本文の階層、文単位の翻訳対応、inlineの注釈を独立した構造として扱う。sentence literalとprefix constructorを同じ意味モデルへlowerする。source表記は別に保持する。

## 1. 主要な型

Article(language, title:Sentence, body:Body)。BodyはBlockの列。BlockはParagraph、Section、DisplayMath、CircuitFigure、Code。
ParagraphはFlowの列。FlowはSentence、ParallelとBlockの各variant。したがってParagraphを再帰的に含められる。
SentenceはInlineの列。Parallelは2個以上のVariant(language, sentence)の列。VariantへParagraphやParallelは入れられない。
InlineはText、Concat、Ruby(base,reading)、Anno(base,notes)、InlineMath、Anchor(id,label)、Reference(target,label)、Emphasis、Strong、Break。

全signatureはdoc-signaturesを参照。bare quoted文字列をSentence/Flowの位置で読むとSentenceLiteralになる。`text` の子は通常Textであり、その内部の `[` や `{` を注釈と解釈しない。

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

InlineMath / DisplayMath / CircuitFigure / Codeは、スロット種別とForeignSyntaxを保持する。MathやCircuitの型をdoc-coreへimportしない。

suiteがMathの構造・bindingをcheckし、math-mathmlからsafeなMathML subtreeを作る。CircuitFigureはcheck/elaborateとdiagramを使う。Codeは元のguest sourceとviewの表示であり、guestのlower・意味check・compile・evaluateを実行しない。guestがDoc自身の場合もDoc:DocGuest.syntaxはForeignSyntaxであり、Doc/Articleの意味値への変換を表示の前提にしない。bundleのschema・参照・source範囲の安全性検査は省略しない。

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
