# Source-set archive v1

This opt-in boundary uses package `nepl3.source-archive`, revision 1, root
`SourceSets`. Its structural contract is `interfaces/source-archive.json` and
`nepl3_core::schema::archive::descriptor`. It requires the registered foundation
revision 1 schema for `SourceContent` and `SourceRef`. The caller supplies both
exact schema references and a finalized registry. Unknown versions or schema
identities are rejected by the checked boundary.

The first field is a canonical source table, ordered by complete source identity.
A source ID and revision appear at most once, including entries with different
content or URI. Actual content digests and locators are checked on import.
The second field is an ordered list of local source sets. Each set contains
ordered, unique `SourceRef` memberships. Set order is meaningful and is preserved.
An empty set declares no source. A missing membership cannot be recovered from
another set or the operation's admission ledger. Every table entry must be used
by at least one set; unused entries are rejected. The encoder requires canonical
input rather than silently sorting membership lists.

Decode constructs checked snapshots once from the table. It resolves each set
by metered native cloning, sharing immutable text on pointer-atomic targets and
charging actual owned copies on other targets. It does not expand a separate
NDF Text for every membership. Failed resolution does not mark partial membership
as used. Limits and cancellation remain sticky; no partial archive result is
returned. The admission ledger retains its normal monotonic failure semantics.

This format archives source sets only. It does not rewrite SyntaxBundle,
ForeignClosureGroup, token payloads, view bundles or arbitrary TypedValue records.
Receiving graph adapters must validate syntax, origins and mappings against each
local set. Legacy source/syntax decoders do not accept SourceSets records. Existing
v1 semantic identity domains and default Doc conversion remain unchanged.

Full syntax transport, typed nested payload coverage and real-data Work recovery
remain separate unfinished stages. A smaller source-set archive is not evidence
that the default 4-billion-Work Doc conversion succeeds.
