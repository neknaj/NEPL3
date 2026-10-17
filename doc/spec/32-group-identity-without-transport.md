# Group identity without transport

FoundationGroupCommitmentCodec is an optional capability separate from the archive
codec. It returns a complete group commitment and ordered existing v2 entry
commitments after depth-aware native and codec-registry validation. It produces no
GroupArchive transport and has no default-success implementation.

The complete group digest uses domain `NEPL3.Foreign.Group.v2\0` over exactly a
three-element NDF List:
1. The checked ForeignClosureGroup skeleton, with typed local source lists replaced
   by empty lists, retaining all other fields and table order.
2. A provenance-indexed List of membership-list sequences.
3. An entry-indexed List of child-first syntax membership-list sequences.

Every membership list contains the ordered SourceContent v2 commitments defined
by the compact entry contract. Empty sets and component boundaries are preserved.
Source commitments include the full Foundation SchemaRef, SourceRef, URI and text.
Unused local declarations remain included. Native-invalid unreferenced provenance
or environment rows are not admitted. Complete group identity intentionally binds
table order and owner/provenance references; per-entry v2 identities retain their
existing valid-remapping invariance.

The operation preserves native occurrence depths/current outer depth, independent
codec registry validation, replaced-source-field descriptor checks, skeleton
structural validation, individual SourceContent checks and one cumulative admission
ledger. Missing local declarations and conflicting snapshots reject. It charges
actual Work, allocations, retained commitments and digest output. It does not charge
or allocate nonexistent serialized skeleton/GroupArchive buffers. Existing archive
APIs, transport bytes, identity domains and resource charges remain unchanged.

Returned digests are data, not native validation proofs. A consuming Doc/PageSet
profile must bind its remaining fields separately and explicitly select new identity
domains. This capability alone establishes no rendering or historical-corpus success.

Identity-only document consumption is specified in [Doc commitment profile](33-doc-commitment-profile.md).

Identity-only capture does not build the archive union or SourceRef membership
storage. It orders each local declaration set once, explicitly admits each source,
and retains the same exact SourceSnapshot/digest cache and ordered commitment
memberships. Archive capture keeps its existing union/membership behavior. Removing
unused transport bookkeeping does not alter either identity preimage or skip
source equality, descriptor, native validation or admission checks.

Identity-only source digest lookup may use a next-index hint local to one
canonically ordered source list. Every hit still compares the full identity and
checks exact snapshot equality. Gaps fall back to bounded binary search; the hint
is reset for each local list. Admission, duplicate rejection, miss-path schema
validation and digest framing remain mandatory. Archive lookup is unchanged.
