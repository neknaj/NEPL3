# Group source archive v1

The opt-in `nepl3.group-archive` revision 1 root `GroupArchive` contains
`sources: nepl3.source-archive/1 SourceSets` and `skeleton: Bytes`. This is a
separate descriptor identity; source-set and syntax-archive identities do not
change. JSON/Rust schema parity and dependency closure are checked by tools.

The skeleton is structurally checked foundation ForeignClosureGroup data with
empty source fields for each provenance and directly traversed syntax bundle.
One local source set is consumed for every provenance in table order, followed
by each entry's syntax bundles in child-first Finish order. Empty sets still
consume one position. Owner/environment/entry order and indices are preserved;
only the shared source table and local source memberships are canonicalized.
Equal content does not merge owners, IDs or revisions.

Each provenance's declarations are restored before its origins/maps are decoded.
Each bundle's declarations are restored before its node geometry, origins/maps
and views are decoded. Local stores use metered insertion. Missing/extra sets,
nonempty skeleton source fields, invalid identities and ownership references
reject. Encoding validates the original native group; decoding validates the
fully reconstructed group before publishing it. Neither a different component
nor the operation admission ledger supplies a missing local declaration.

The transport shares typed source-field mechanics with SyntaxArchive. It does
not rewrite opaque payloads or replace the expanded native v1 component identity.
Legacy group decoding rejects the archive envelope. Standalone closure extraction
and existing component digests are evaluated on the reconstructed native group.
Default Doc integration, general Sentence payload adapters and real-data 4B Work
recovery remain separate unfinished work.

## Replaced-field descriptor boundary

Empty skeleton source lists do not establish their element types. Before encode
or decode, SyntaxArchive requires the selected Foundation descriptor to declare
SyntaxBundle field 0 as `sources: List(SourceContent)`. GroupArchive also requires
ForeignProvenance field 1 to have that exact contract. Each SourceContent named
reference uses package `nepl3.foundation`, revision 1. These checks apply even to
empty inputs; an archive deliberately requires its complete replacement contract.
The selected SchemaRef must match the finalized registry. Descriptor lookups and
comparisons are charged to the current Budget and respect sticky cancellation.
These checks supplement full original-native validation, checked skeleton and
source-table boundaries, and reconstructed-native validation; they do not change
v1 descriptors, wire bytes, or component identities.
