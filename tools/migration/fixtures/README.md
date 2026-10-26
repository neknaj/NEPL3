# Historical converter input

`00-contract.md` is the unchanged Git blob from
`a163764e93e2809f8013d7bbe16721082f979143:doc/spec/00-contract.md`.
Its SHA-256 is `4ca8d780d32696bf102774c346ffdf18d01452085f93a2daae9e49acf16b003f`.

This fixture keeps the restricted Markdown converter and its independent
Markdown-event comparison reproducible after chapter zero moves to a Doc source.
It is historical test input, not a second source of truth for the specification.
Do not update it to match newly generated Markdown. The maintained specification
is registered in `doc/canonical.json`.

## Documentation index original

`documentation-index.md` is the unchanged `doc/README.md` at
`113e87da0449430b4c5422d77593116d91a72f0f`, SHA-256
`5120d92cff9839cec6afdd608d284f479759c4ffe67ea9ea2c3e8e0fc4fec3a6`.
It is a fixed pre-cutover regression input, not a maintained second source.
The native typed-model comparison checks prose/table structure and references.
The two directory links `../examples/` and `../tasks/` are explicitly
mapped to the corresponding public GitHub `tree/main` URLs; this preserves
public-main navigation, not checkout/release-relative or offline navigation.
Do not regenerate this historical input from the candidate Doc or projection.
The explicit source link `spec/13-reproducibility.nepld` also maps to
`https://github.com/neknaj/NEPL3/blob/main/doc/spec/13-reproducibility.nepld`,
not to its rendered page. Canonical PageSet logical sources are projection paths;
physical source paths are not implicit aliases. This third mapping offers public
main source-file viewing, not raw-byte HTTP delivery, version-fixed or offline access.
