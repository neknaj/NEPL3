`interfaces/doc.json` の順序付きrecordを交換契約とする。
`PageRegistration` は安定した `id`、入力の論理 `source` path、出力artifact内の
`route` を持つ。`PageDocument` はregistrationと完全な `DocumentSyntax` を持ち、
`PageSet` は文書の順序付き非空list `pages` と、非Doc fileの順序付きlist `files` を持つ。
`PageFile` はregistrationと `content: Bytes` を持ち、元byte列をそのまま交換する。
文書とfileの登録順は別のindex名前空間とし、`PageDestination` の
`Page { index }` / `File { index }` で区別する。空のDocをfileの代わりに登録しない。