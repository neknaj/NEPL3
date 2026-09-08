# Doc移行候補

このdirectoryは正式文書をDocへ移行するための候補を置きます。現在の正本は引き続き元のMarkdown文書です。

`authored/` は [執筆指針](../authoring.md) に従って人が読む文・振り仮名・注釈・構造を判断して書き直した原稿です。元文書のcommitとdigestを固定して対応を確認し、執筆者とは別の担当が本文・コード・表・参照を独立レビューします。通常の本文にはsentence literalを使い、コードや参照などを含む文には明示的なsentence構築を使います。原本が更新された場合は差分を原稿にも反映し、その差分を再レビューします。

原稿の作成・内容レビューと、実処理系によるHTML生成・リンク/旧anchor互換・正式な正本切替えは別の工程です。[移行条件](../spec/16-doc-migration.md)が満たされるまではMarkdownを削除しません。切替え後はDocを単一の正本とし、必要なMarkdown入口は生成projectionとして維持します。

## 構文表の生成候補

`generated/*-signatures.nepld` は `design/forms.json` から生成する4言語の構文表です。
`python tools/generate/signatures.py --write` で更新し、引数なし実行で差分を検査します。
通常本文の機械的な置換ではなく、規範データのカテゴリ・form・field順・読取カテゴリ・arity・葉の規則をDocのsectionとtableへ投影します。コードはInlineCodeで保持します。
これらは手書きで保守せず、元のMarkdown構文表との正本切替えや旧anchor互換の完了とは区別します。

## 生成変換器の検証候補

以下の `00-contract.nepld` と確認用索引は、限定した変換器と処理系の検証に使う生成物です。これらは原本から再生成して内容・構造を比較し、手書きで保守しません。`authored/` の執筆原稿とは用途と更新方法が異なります。

最初の候補は [対象と設計上の決定](00-contract.nepld)。
`python tools/migration/contract.py --write` で生成し、同コマンドの引数なし実行で
原本との対応を検査します。この変換器が扱うのは該当ページの見出し・段落・flat list・
inline codeだけで、未対応のMarkdownを黙ってTextへ変えません。

生成後は通常のDoc parser、lower、prepare、HTML backendへ入力します。
人による意味同等性レビュー、旧anchor互換、Markdown互換projectionの正式採用、関連する
機械参照の切替えは未完了です。これらを満たすまで元Markdownを削除せず、
T21やPages公開の完了とも扱いません。

確認用の索引と候補を同じページ集合として生成できます。

```sh
python tools/migration/contract.py
python -m unittest discover -s tools/migration -p test_contract.py
mkdir -p dist
cargo run --locked -p nepl3-tools -- doc-html pages doc/migration/pages.json dist/doc-migration
```

出力先は未作成のdirectoryを指定します。索引から候補への参照、実際のHTML配置、
各ページのCSSを同じ生成処理で検査します。この索引は移行確認用の新規Doc原稿であり、
既存Markdown文書の第二の正本ではありません。

## Markdown互換表示の確認

```sh
mkdir -p dist
cargo run --locked -p nepl3-tools -- doc-markdown doc/migration/00-contract.nepld dist/00-contract.view.md
```

既存のファイルには上書きしません。Doc構造を再度検査し、原稿path・digestとrenderer版を
記したMarkdownを生成します。現在の対応は、この候補に必要な同階層の節、段落、単純な
unordered list、Text、InlineCodeです。空白や記号が別のMarkdown構造にならないよう検査し、
注釈・parallel・リンク・表・隣接code・入れ子の節などは省略せず拒否します。
正式仕様の原本と、Docから生成したMarkdownを独立したMarkdown parserへ通し、本文と
block/codeのevent列を比較します。これは限定した互換表示で、DocのIDやsource mappingを
Markdownから復元する一般的なroundtripではありません。
