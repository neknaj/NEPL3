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

The independent LanguagePackage, literal/prefix lowering and printing,
annotation adapter and Doc/Math migration remain part of
the same ongoing recovery. This stage is not a completed language or removal of
lexical comments. Do not merge it as a completed migration.
