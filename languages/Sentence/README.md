# Sentence

`syntax.neplg` defines the independent structured-sentence surface, with root
`Sentence` and category `Inline`. The production Grammar compiler builds its
LanguagePackage; the development host registers the Name and Sentence literal
reader operations explicitly. Neither Doc nor Math is a required schema.

```text
sentence
  cons ruby text "漢" text "かん"
  cons anno text "語" cons text "note" nil
  nil
```

The corresponding literal is `"[漢/かん]{語/note}"`. Literal token structure and
the prefix parse tree remain distinct. The semantic equivalence of the two is
part of the lower/check/print acceptance, not proven by parsing alone.

The surface accepts whitespace separation. It does not skip `#` comments.
Annotation belongs to the separate target-wrapper contract; document structure
belongs to Doc. See [spec 23](../../doc/spec/23-sentence-annotation.md).

LanguagePackage compilation and native/owned prefix parsing are implemented.
Prefix lowering, general printing and Doc body migration remain in progress.
