# SVG画像の試作

同じDoc原稿からSVGの参照形式と埋め込み形式を生成する。対象は単一文書である。

```sh
nepl3-tools doc-html svg --css inline --svg external examples/document/svg/example.nepld examples/document/svg/assets.json new-external
nepl3-tools doc-html svg --css inline --svg embedded examples/document/svg/example.nepld examples/document/svg/assets.json new-embedded
```

参照形式は出力ディレクトリ全体を保持する。埋め込み形式はdocument.html内にSVGを保持する。CSSはexternalとinlineを独立に選択する。フォントは既存の外部参照が残り、オフラインではシステムフォントを使用する。

画像入力はmanifestに明記し、SVGのbyte列を変更せずに検証する。静的profileはsvg・g・path・defsと、defs内のpathを指す非再帰のuseを扱う。transform等の属性は専用の制限付き文法で検査する。text・style・script・外部資源を含むSVGは拒否し、TikZの文字はpath glyphとして受け取る。任意のSVGを扱うprofileではない。

triangle.svgはtriangle.texからlatexとdvisvgmで生成した。図の内部に白い背景を含むため、暗い文書背景でも黒線を判読できる。

数式を含む原稿では、同じSVG操作へ `--math-renderer mathml-only` を指定できる。省略時は `katex-preferred` となり、対応するKaTeX adapterが未接続の間は理由をmanifestへ記録してMathMLを用いる。Codeの内容は評価せず、保持したソースを表示する。

この試作は既存のpages操作やportable asset準備操作の完成を意味しない。既存の `prepare_local`・SVG-only・SVG+Code準備APIの契約は維持し、SVG+Mathは別の準備経路で扱う。実ブラウザでの表示確認は構造・生成試験と区別する。

## MarkdownへのSVG出力

[Doc正本](markdown.nepld)と[登録ファイル](markdown.pages.json)から、[閲覧用Markdown](markdown.md)と参照先SVGを生成する。

```sh
mkdir -p .tmp
nepl3-tools doc-markdown footnotes-pages examples/document/svg/markdown.pages.json .tmp/svg-markdown
```

出力先は未作成のディレクトリを指定する。captionのAnnoは脚注、altはBaseOnlyのテキストとなる。SVGは登録routeに元のbytesで出力し、manifestへ依存関係とSHA-256を記録する。

生成例を更新する場合は、出力検証後のmarkdown.mdとassets/triangle.svgをこのディレクトリへコピーする。Markdownを手編集しない。GitHub上での画像・脚注表示の確認は、ローカル生成試験とは別に行う。
