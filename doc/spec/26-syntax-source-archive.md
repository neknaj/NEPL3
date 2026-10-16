# Direct-foreign syntax archive v1

The opt-in `nepl3.syntax-archive` revision 1 root `SyntaxArchive` has a `sources`
field of `nepl3.source-archive` revision 1 `SourceSets` and a `skeleton: Bytes`
field. Its separate schema leaves the exact SourceSets descriptor identity
unchanged. Both interface contracts are checked against Rust descriptors and
registered together with foundation revision 1 before boundary use.

Skeleton bytes encode a structurally checked foundation SyntaxBundle with empty
source lists in the root bundle and every directly traversed ForeignSyntax
bundle. They are not advertised as a semantically valid standalone syntax value.
Each bundle consumes one explicit source set in deterministic child-first Finish
order. Its source declarations are restored before node/origin/span/source-map
resolution. Nonempty skeleton declaration lists, missing sets and extra sets
are rejected. An empty membership set grants no declarations, including when
other sets or the admission ledger contain matching sources.

Encoding validates the original native graph. Decoding validates the outer
archive schema, the source-set boundary and the skeleton's structural schema;
then reconstructs through the shared typed bundle decoder. Existing canonical
node/reachability checks remain, and final native validate_with_sources is
required before publication. Budget and admission are shared across all steps.

Opaque token payloads remain unchanged, even if they contain nested source text.
This stage covers direct ForeignSyntax fields only; complete typed token/view
ownership adapters are unfinished. It is not enabled on default Doc conversion.
Existing v1 semantic identity uses the reconstructed native value and legacy
expanded encoding. Neither skeleton bytes nor archive bytes may replace it.
