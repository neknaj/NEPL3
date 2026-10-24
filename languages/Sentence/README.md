<!-- Generated from languages/Sentence/README.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page sentence&#45;surface; source SHA-256 30a2b186fd267a13f73c15c8ed0d8cef0c1a3df5ba1c06e25575e694bfce69c4; alias input SHA-256 3a6c1347ddcb9a4e7adbd684187f9127be495852d1e31db3ffb374c8065ce01b; page input SHA-256 741250fca47aaad691b3df7fa2f8b5508cce636f2c1bca01d16a17820a42a7c4. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="sentence"></a>

# Sentence

[正本（NEPL3d）](<README.nepld>)

`syntax.neplg` defines the independent structured\-sentence surface\, with root `Sentence` and category `Inline`\. The production Grammar compiler builds its LanguagePackage\; the development host registers the Name and Sentence literal reader operations explicitly\. Neither Doc nor Math is a required schema\.

```text
sentence
  cons ruby text "漢" text "かん"
  cons anno text "語" cons text "note" nil
  nil
```

The corresponding literal is `"[漢/かん]{語/note}"`\. Literal token structure and the prefix parse tree remain distinct\. The semantic equivalence of the two is part of the lower\/check\/print acceptance\, not proven by parsing alone\.

The surface accepts whitespace separation\. It does not skip `#` comments\. Annotation belongs to the separate target\-wrapper contract\; document structure belongs to Doc\. See [spec 23](<\.\.\/\.\.\/doc\/spec\/23\-sentence\-annotation\.md>)\.

LanguagePackage compilation and native\/owned prefix parsing are implemented\. The standard prefix printer emits all ten forms\, keeping constructor boundaries and escaping BuiltinText\. Foreign\-inline output needs its selected surface adapter\. Standard prefix lowering returns a checked meaning arena and a mapping back to the retained syntax bundle\. The presentation entry handles literal payloads and prefix forms with their source locations and views\. Foreign adapters and Doc body migration remain in progress\; this is not a completed document migration\.
