# Doc文書の執筆指針

この指針は、NEPL3の例・仕様・解説をDocで執筆する際の表記選択と確認手順を定める。構文と意味は[Doc仕様](spec/05-document.md)、正式文書の正本切替えは[移行条件](spec/16-doc-migration.md)を参照する。

## Sentence literalを標準にする場面

Sentence literalは、Text・Ruby・Annoからなる文章を引用符内に記述する表記である。通常の説明文、見出し、表のセル、対応文のvariantでは、この表記を基本とする。Ruby・Annoの入れ子も表現できる。

```text
"これは{[文書/ぶんしょ]/document}の[例/れい]です。"
```

一つのliteralは一つのSentenceを表す。本文では、著者が対応させる文ごとにliteralを記述する。見出しや表のセルでは、短い語句も一つのSentenceとして扱える。Sentenceの境界は著者が指定し、literal内の句点は本文の一部として扱う。

literalはソース上の一行に記述する。引用符は `\"`、バックスラッシュは `\\`、文字としての注釈区切りは `\[` などでescapeする。`\n`はTextのデータとしてLFを保持するescapeである。

文書構造として指定する改行には、前置構築の[`break`](#明示的な改行はbreak)を使う。画面幅に応じた折返しは表示側が処理する。長い文も著者が指定した対応単位を保持し、編集上の都合に応じて前置構築を選択する。

## 明示的なsentence構築を選ぶ場面

前置構築は、`sentence`の子としてInlineを列挙する表記である。強調の`strong`・`em`、参照の`ref`・`anchor`・`link`、Inline位置の`code`、画像、Math、明示的な改行の`break`を含むSentenceに使用する。

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

複雑な注釈の各部分を編集・比較する場合や、constructor API・構文を解説する場合にも前置構築を使用できる。通常の本文は、含まれるInlineと編集目的に応じて表記を選択する。

プログラムから文書を生成する通常の経路では、型付きconstructorで意味構造を構築し、既存の検査・printerへ渡す。ソース文字列の出力はprinterが担当する。

`text "..."` の引用部分は通常のText値として保持される。Ruby・Annoは、それぞれのconstructorで構築する。Sentence literalのcategoryはSentenceであり、`sentence`の子に使用できるcategoryはInlineである。

## 明示的な改行はbreak

`break`は引数を持たないInline constructorであり、同じSentence内の改行位置を指定する。HTMLでは`br`、plain text抽出ではLFとして出力される。Sentenceの対応単位は保持される。段落の境界はparagraphで表す。空の段落や複数のbreakによる余白調整は禁止する。

```text
sentence
  cons text "ここで"
  cons break
  cons ruby text "改行" text "かいぎょう"
  cons text "します。"
  nil
```

既存のText内LFとRawCodeの改行は、元の内容として保持する。これらと`break`の自動相互変換は禁止する。Compact printerは、breakを含むSentenceを前置構築で出力する。[改行の例](../examples/document/line-break.nepld)では、日英の対応文それぞれにbreakを配置している。

## sentenceとparallelの使い分け

`parallel`は、意味が対応する各言語のSentenceをまとめる構造である。本文では文ごとに作成し、各`variant`へ対応する言語のSentenceを配置する。複数の文からなる段落は、文ごとのparallelを並べて表す。各言語の語順と語数は、その言語で自然に意味を表現するために選択する。

```text
parallel
  cons variant ja "これは{[文書/ぶんしょ]/document}の[例/れい]です。"
  cons variant en "This is an example of a document."
  nil
```

表記はvariantごとに選択する。各言語で必要なInlineに応じて、literalと前置構築を組み合わせられる。強調や参照を付ける場合は、対応言語の意味と参照先も確認する。空Sentenceは、空の翻訳を意図する場合に限って使用する。

[線型結合の例](../examples/document/linear-combination.nepld)では、最初の対応文をliteral、続く対応文を明示的なsentence構築で記述する。両表記とも、文単位の意味対応を保持する教材として使用する。

## RubyとAnnoの対象

Rubyは本文に読みを付与する構造である。日本語の振り仮名は[GlossのRuby指針](https://github.com/neknaj/gloss#ruby)に従い、漢字部分に付ける。送り仮名・助詞・カタカナ・数字・記号は通常のTextとして配置する。読みの単位は、文脈に合う語または漢字部分とする。

語句全体の訳語・意味説明はAnnoに置く。例えば `{[原点/げんてん]を[通/とお]る[直線/ちょくせん]/a line through the origin}` とする。前置構築ではAnnoのbaseをConcatで組み、その子をRubyとTextに分ける。カタカナ語は `{ベクトル/vector}` のように意味注釈を付けられる。

漢字部分への振り仮名という規則は、日本語文書の執筆に適用する。Rubyのparserとschemaは、中国語のピンインや他言語の転写を含む一般的な音韻注釈を扱う。

## 構造と確認

話題のまとまりはsection、連続する本文はparagraph、列挙はlist、同じ項目を比較する情報はtableで表す。参照には安定したsection/anchor IDを使う。装飾・表・埋め込みは、読者が内容を理解するために必要な箇所へ配置する。

表記変更後は、本文・読み・注釈・対応文・強調・参照の保持を確認する。literalと前置構築の等価性は意味構造で判定する。ソース位置は各表記のsourceに属し、変更後の位置を個別に確認する。

構造audit、正式なparser、lower、HTML生成は、それぞれの検証結果を記録する。資源停止や未対応機能が残る例には、その制約を明記する。実行可能性の説明は、実際に成功した処理範囲に限定する。

RustのDoc APIで既存部分を組み替える場合は、正式にlowerした`DocumentSyntax`から親子参照を辿って対象を選び、`fragment(DocRoot, registry, budget, admission)`で抽出する。抽出は元文書全体を検証し、到達可能なnodeとembedを再配置する。共有参照とForeignClosureを保持し、Source・Origin・View・source mapは元identityのまま保持する。このため、費用と保持するsource集合は元文書全体に依存する。

抽出後に型付きconstructorでListItemなどを構築し、既存printerへ渡す。sourceの局所編集には、元モデルが示すSpanと元byte列のdigestを持つ`TextEdit`を`SourceStore::apply`へ渡す。変更後の位置は新しいsnapshotの再parseで確定し、参照解決やprepareも改めて行う。実行例は[Doc printer統合試験](../tools/tests/doc/print.rs)の`paragraph_edit_uses_model_span_and_preserves_surrounding_source`にある。
