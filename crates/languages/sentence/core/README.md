# Independent Sentence core

This `no_std + alloc` crate owns structured sentence content independently of
Doc, syntax annotation, Math and host rendering. The migration contract is
[spec 23](../../../../doc/spec/23-sentence-annotation.md).

The current stage implements typed finite arenas, graph/category/nonempty
annotation checks, and the versioned schema descriptor. A shape proof does not
validate foreign source closures, resolve document names or certify safe HTML.

The independent LanguagePackage, literal/prefix lowering and printing, codecs,
Source/Origin boundary, annotation adapter and Doc/Math migration remain part of
the same ongoing recovery. This stage is not a completed language or removal of
lexical comments. Do not merge it as a completed migration.
