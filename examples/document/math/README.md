# GitHub Markdown向けMathの例

正本は `markdown.nepld`。閲覧用の `markdown.md` は次の操作で生成する。

```sh
cargo run --locked -p nepl3-tools -- doc-markdown footnotes-pages examples/document/math/markdown.pages.json /tmp/nepl3-math-example
```

本文・Docの脚注・display block・table cellの数式を扱う。
Math内部のSentence注釈など、構造TeXで保持できない内容は拒否する。
Mathの評価は行わず、GitHub側のMathJaxによる表示を利用する。
生成物の構造検査は、GitHubでの実描画・アクセシビリティ検証を代替しない。
