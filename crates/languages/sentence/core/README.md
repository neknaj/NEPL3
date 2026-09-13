# Independent Sentence core

This `no_std + alloc` crate owns structured sentence content independently of
Doc, syntax annotation, Math and host rendering. The migration contract is
[spec 23](../../../../doc/spec/23-sentence-annotation.md).

The current stage implements typed finite arenas, graph/category/nonempty
annotation checks, the versioned schema descriptor and an explicit NDF value
codec. The portable boundary checks both the exact schema and arena invariants,
and delegates foreign source closures to the host-selected foundation codec.
A shape proof alone does not
validate foreign source closures, resolve document names or certify safe HTML.

`SentenceSyntax` separately retains dense node locations, source snapshots,
Origin arenas and owner-indexed token views. Native and portable checks enforce
their declaration closure; this does not prove source/meaning equivalence.

`literal::read` recognizes one quoted sentence and returns its closed syntax;
`literal::print` emits the literal-expressible content subset. Ruby/InlineAnno,
escape spelling and original positions remain distinct. Prefix-only forms
return a typed `NotLiteral` error instead of losing their meaning.

`portable::literal` transfers one literal against an explicit owner snapshot,
without repeating the document's source bytes in every token payload. It checks
root ownership and source containment; generated/mapped syntax uses the general
closed syntax envelope. This codec is not the reader/provider implementation.

The development host now compiles the independent LanguagePackage and parses
literal/prefix sources with the production engine. The standard prefix printer
handles all ten forms; foreign output requires a surface adapter. Standard prefix
lowering preserves a mapping to the retained input syntax. The presentation entry
accepts literal/prefix syntax with checked owner positions and views. Foreign
adapters, full semantic roundtrips and Doc/Math migration remain part of
the same ongoing recovery. This stage is not a completed language or removal of
lexical comments. Do not merge it as a completed migration.

Sentence integration into Doc and docs-only publication precede the full
annotation migration; Doc content does not depend on annotation completion.
