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

## Explicit compact component identity v2

The opt-in `syntax::group::compact::encode` returns the unchanged GroupArchive
transport and a parallel vector of v2 entry digests. Existing encode, decode,
FoundationValueCodec, and v1 identity APIs do not implicitly select this path.
The operation uses one supplied registry, SourceAdmission and Budget. It validates
the original native group, the replaced-field descriptor contract, the complete
empty-source skeleton, each full SourceContent commitment input, and the final
archive envelope. A failure returns no successful transport/digest pair.

Let `NDF(v)` mean canonical NDF/1 bytes and `H(d,v)` mean SHA-256 of the literal
UTF-8 domain `d` followed by NDF(v). Each domain below ends in one zero byte.

- Source commitment: H(`NEPL3.Foreign.SourceContent.v2\0`, full SourceContent).
  The record retains the exact Foundation SchemaRef, full SourceRef, URI and text.
- A local membership is a List of Bytes32 source commitments, ordered by the
  existing canonical source ID/revision/digest ordering. All declarations remain,
  including unused ones. Empty local sets occupy a position.
- Provenance: H(`NEPL3.Foreign.Provenance.v2\0`, List(skeleton provenance,
  List(local membership))). There is exactly one local set per provenance.
- Syntax: H(`NEPL3.Foreign.Syntax.v2\0`, List(skeleton ForeignSyntax,
  List(local memberships))). Direct foreign bundles contribute in the existing
  child-first Finish order, followed by their parent bundle. Memberships never
  cross an entry boundary. Opaque token payloads are preserved and not traversed.
- Environment: H(`NEPL3.Foreign.Environment.v2\0`, the encoded environment field).
  The owner table index and provenance table reference are excluded here.
- Entry: H(`NEPL3.Foreign.Entry.v2\0`, List(Bytes32 provenance commitment,
  Bytes32 environment commitment, Bytes32 syntax commitment)). References select
  the appropriate components but their group table indices are not committed.

These two-element component envelopes are private hash preimages, not valid
stand-alone native syntax or additional transport descriptors. Canonical NDF
framing and separate domains preserve component and local membership boundaries.
Components used only by other valid entries do not affect a selected entry identity.

Source commitments are memoized only inside this operation. Sorted full SnapshotId
lookup is metered; a hit requires exact admitted immutable snapshot equality,
including URI and text. Independently allocated equal snapshots pay for equality.
All lookup, insertion shift, component bookkeeping, copies, retained storage,
encoded digest input, digest output and transport output consume the same Budget.
A sticky stop remains a failure on retry. No proof or cache crosses operations.

This Foundation API does not complete default Doc integration, foreign Sentence
payload adaptation, or the historical real-data 4B Work acceptance milestone.
