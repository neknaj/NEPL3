# Independent source lookup, report source index and map node index review

Scope: the three original files fixed in files.json at C:/projects/NEPL3-source-lookup-indexes, plus root's exact one-line cancellation fix to get_revision_with_budget. No production or Git edits were made. SourceAdmission index WIP was deliberately excluded: scratch uses the original captured source.rs and predecessor source/edit.rs. root-fixed-get-revision.rs is copied from root's actual function; a script asserted the sole function change was budget.poll before the empty-index path. fix.json preserves both full-live and extracted-function hashes. This review does not certify later SourceAdmission work.

Finding: original public get_revision_with_budget returned Ok(None) from an empty store even after Budget.cancel. Independent before-native.log shows the unchanged expectation Err(Cancelled) failing with Ok(false) (Option mapped to is_some for a compact assertion). Nonempty lookup stopped on its first charge. Root added budget.poll at entry; the same probe passed afterward. This finding is corrected within the reviewed function.

Index review:
- SourceStore lookup orders by source ID and exact revision, charges name comparison before binary search decisions, and returns the complete stored snapshot for caller digest/URI checking. It does not treat a revision match as full identity equality.
- diagnostic Sources.new checks store conflicts through that lookup and validates digest plus URI. Its added-only sorted borrowed index checks any equal key before inserting; exact duplicate re-declarations remain legal as previously. Because all equal keys are validated, choosing any duplicate in the binary search does not hide a conflicting later duplicate. Added lookup remains linear and all spans still require the exact SnapshotId and valid scalar boundary.
- snapshot_dag sorts only a private node-index vector; graph node IDs retain first-seen assignment, and ordering uses the complete SnapshotId including source, revision and digest. Duplicate edges remain in the graph and corresponding incoming counts are decremented consistently. Cyclic coarse graphs still use the existing exact pointwise fallback, permitting disjoint self-snapshot mappings while rejecting real cycles. Depth and Nodes checks remain in graph traversal.
- New reference/index allocations and index shifts are charged before pushes/inserts. Root's existing store and report inputs are borrowed; failed construction publishes no partial index or success proof.

Independent runtime:
- Public API probes:6 native and6 WASI pass.
- Existing core suite from committed source-copy-accounting predecessor:70 native and70 WASI pass against the fixed three-file implementation plus root cancellation fix. This is a core-only scratch workspace, not all repository crates.
- Exact revisions and Unicode IDs under unsorted insertion, absent revisions, zero Work lookup.
- Report identical duplicate sources accepted; conflicting digest or URI in either store/added or added/added rejected. A nonempty Event resolves the exact span from added sources; missing revision rejects. Work caps0..240 and Allocation0..24 include stopped and successful paths.
- Map real cycle, duplicate edge, same SourceId with another revision, disjoint self-snapshot edge allowed, true self-snapshot cycle rejected, chain depth2 rejected. Work0..185 and Allocation0..152 sweeps.
- All512 directed graphs on3 vertices, in both forward and reverse declaration order, agree with an independent Boolean transitive-closure cycle oracle. Vertices include same SourceId at distinct revisions.

Setup limitation preserved: the first attempt to run every core test accidentally paired the fixed old SourceAdmission with a concurrently-added admission-index performance test. all-native.log and all-wasi.log retain that single setup-scope failure. It is not an implementation finding for these three files. Only contracts.rs differed from the committed predecessor tests; restoring that original test file produced fixed-native.log and fixed-wasi.log with76 total passes each (core70 + independent6). No expectations in the six independent tests were weakened to obtain success.

Artifacts include original bytes, the root fix extraction, before/after logs, exact fixed workspace source hashes, and probe source. No bootstrap performance, full Doc completion, physical target, or complete current branch acceptance is claimed.
