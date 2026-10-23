<!-- Generated from doc/spec/32&#45;group&#45;identity&#45;without&#45;transport.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page group&#45;identity&#45;without&#45;transport; source SHA-256 7fc059dac85612a8737eef10cc4d1cc97fd48c479d59401f74d76da4bc3d620c; alias input SHA-256 a4fcd4308863d5492612ba3b3a463702e1d05e5412795a778fe053284e51cca8; page input SHA-256 cf6074dc38b42ec09e13d99ecb5ba9aaaf54208a7baefce0e45fae2c76a4f02e. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="group-identity-without-transport"></a>

# Group identity without transport

[正本（NEPL3d）](<32-group-identity-without-transport.nepld>)

FoundationGroupCommitmentCodec is an optional capability separate from the archive codec\. It returns a complete group commitment and ordered existing v2 entry commitments after depth\-aware native and codec\-registry validation\. It produces no GroupArchive transport and has no default\-success implementation\.

The complete group digest uses domain `NEPL3.Foreign.Group.v2\0` over exactly a three\-element NDF List\:

1. The checked ForeignClosureGroup skeleton\, with typed local source lists replaced by empty lists\, retaining all other fields and table order\.
1. A provenance\-indexed List of membership\-list sequences\.
1. An entry\-indexed List of child\-first syntax membership\-list sequences\.

Every membership list contains the ordered SourceContent v2 commitments defined by the compact entry contract\. Empty sets and component boundaries are preserved\. Source commitments include the full Foundation SchemaRef\, SourceRef\, URI and text\. Unused local declarations remain included\. Native\-invalid unreferenced provenance or environment rows are not admitted\. Complete group identity intentionally binds table order and owner\/provenance references\; per\-entry v2 identities retain their existing valid\-remapping invariance\.

The operation preserves native occurrence depths\/current outer depth\, independent codec registry validation\, replaced\-source\-field descriptor checks\, skeleton structural validation\, individual SourceContent checks and one cumulative admission ledger\. Missing local declarations and conflicting snapshots reject\. It charges actual Work\, allocations\, retained commitments and digest output\. It does not charge or allocate nonexistent serialized skeleton\/GroupArchive buffers\. Existing archive APIs\, transport bytes\, identity domains and resource charges remain unchanged\.

Returned digests are data\, not native validation proofs\. A consuming Doc\/PageSet profile must bind its remaining fields separately and explicitly select new identity domains\. This capability alone establishes no rendering or historical\-corpus success\.

Identity\-only document consumption is specified in [Doc commitment profile](<33\-doc\-commitment\-profile\.md>)\.

Identity\-only capture does not build the archive union or SourceRef membership storage\. It orders each local declaration set once\, explicitly admits each source\, and retains the same exact SourceSnapshot\/digest cache and ordered commitment memberships\. Archive capture keeps its existing union\/membership behavior\. Removing unused transport bookkeeping does not alter either identity preimage or skip source equality\, descriptor\, native validation or admission checks\.

Identity\-only source digest lookup may use a next\-index hint local to one canonically ordered source list\. Every hit still compares the full identity and checks exact snapshot equality\. Gaps fall back to bounded binary search\; the hint is reset for each local list\. Admission\, duplicate rejection\, miss\-path schema validation and digest framing remain mandatory\. Archive lookup is unchanged\.

On pointer\-atomic targets\, identity\-only capture may retain one owned local source sequence and its canonical index permutation\. Exact immutable\-storage prefix matches permit sorting only newly appended declarations and merging the two orders\. The merge rejects equal SourceId\/revision across the boundary\, regardless of digest\. Shrinking or unrelated sequences use the ordinary complete ordering path\. Independent snapshots never require a text comparison merely to reuse an order\. Returned references always point to the current local input\; every source still undergoes admission and exact digest\-cache validation\. Retained snapshots\, indices\, comparisons and output references are budgeted\, and failed preparation leaves prior ordering metadata intact\. Other targets keep ordinary ordering\.
