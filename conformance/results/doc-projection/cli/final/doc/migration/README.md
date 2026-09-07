# Doc移行候補

このdirectoryは正式文書の変換候補です。現在の正本は引き続き `doc/spec/*.md` です。
候補を別の原稿として手で保守せず、原本から再生成し、内容・構造を比較します。

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
