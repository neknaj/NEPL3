<!-- Generated from doc/spec/26&#45;syntax&#45;source&#45;archive.nepld; renderer nepl3-tools.markdown-annotated/4; source SHA-256 ff7851e02b15e042cb2451fe11e3323b7e2dfdfd68888f4d146d325239f5fa05; alias input SHA-256 e399d582096965f1f95d9888c458e63e4d9c1c77c938bdf077eb281ce35ec7b9. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="direct-foreign-syntax-archive-v1"></a>

# Direct\-foreign syntax archive v1

[正本（NEPL3d）](<26-syntax-source-archive.nepld>)

The opt\-in `nepl3.syntax-archive` revision 1 root `SyntaxArchive` has a `sources` field of `nepl3.source-archive` revision 1 `SourceSets` and a `skeleton: Bytes` field\. Its separate schema leaves the exact SourceSets descriptor identity unchanged\. Both interface contracts are checked against Rust descriptors and registered together with foundation revision 1 before boundary use\.

Skeleton bytes encode a structurally checked foundation SyntaxBundle with empty source lists in the root bundle and every directly traversed ForeignSyntax bundle\. They are not advertised as a semantically valid standalone syntax value\. Each bundle consumes one explicit source set in deterministic child\-first Finish order\. Its source declarations are restored before node\/origin\/span\/source\-map resolution\. Nonempty skeleton declaration lists\, missing sets and extra sets are rejected\. An empty membership set grants no declarations\, including when other sets or the admission ledger contain matching sources\.

Encoding validates the original native graph\. Decoding validates the outer archive schema\, the source\-set boundary and the skeleton\'s structural schema\; then reconstructs through the shared typed bundle decoder\. Existing canonical node\/reachability checks remain\, and final native validate\_with\_sources is required before publication\. Budget and admission are shared across all steps\.

Opaque token payloads remain unchanged\, even if they contain nested source text\. This stage covers direct ForeignSyntax fields only\; complete typed token\/view ownership adapters are unfinished\. It is not enabled on default Doc conversion\. Existing v1 semantic identity uses the reconstructed native value and legacy expanded encoding\. Neither skeleton bytes nor archive bytes may replace it\.

<a name="n-64657363726970746f72"></a>

<a name="replaced-field-descriptor-boundary"></a>

## Replaced\-field descriptor boundary

Empty skeleton source lists do not establish their element types\. Before encode or decode\, SyntaxArchive requires the selected Foundation descriptor to declare SyntaxBundle field 0 as `sources: List(SourceContent)`\. GroupArchive also requires ForeignProvenance field 1 to have that exact contract\. Each SourceContent named reference uses package `nepl3.foundation`\, revision 1\. These checks apply even to empty inputs\; an archive deliberately requires its complete replacement contract\. The selected SchemaRef must match the finalized registry\. Descriptor lookups and comparisons are charged to the current Budget and respect sticky cancellation\. These checks supplement full original\-native validation\, checked skeleton and source\-table boundaries\, and reconstructed\-native validation\; they do not change v1 descriptors\, wire bytes\, or component identities\.
