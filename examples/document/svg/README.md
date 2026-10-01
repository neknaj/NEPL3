# SVG画像の試作

同じDoc原稿からSVGの参照形式と埋め込み形式を生成する。対象は単一文書である。

```sh
nepl3-tools doc-html svg --css inline --svg external examples/document/svg/example.nepld examples/document/svg/assets.json new-external
nepl3-tools doc-html svg --css inline --svg embedded examples/document/svg/example.nepld examples/document/svg/assets.json new-embedded
```

参照形式は出力ディレクトリ全体を保持する。埋め込み形式はdocument.html内にSVGを保持する。CSSはexternalとinlineを独立に選択する。フォントは既存の外部参照が残り、オフラインではシステムフォントを使用する。

画像入力はmanifestに明記し、SVGのbyte列を変更せずに検証する。初期profileはsvg・g・pathに限定する。text・use・transform・style・script・外部資源を含むSVGは拒否する。文字を含む一般のTikZ図の対応は今後の拡張である。

triangle.svgはtriangle.texからlatexとdvisvgmで生成した。図の内部に白い背景を含むため、暗い文書背景でも黒線を判読できる。

この試作は既存のpages操作やportable asset準備操作の完成を意味しない。doc-html exportのlocal-only契約は維持する。
