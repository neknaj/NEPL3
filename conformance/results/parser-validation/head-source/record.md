# Independent review: completed-tree head source locality

Candidate 6f84a51b626de595fb73635ed4b116055edfb9e4; base e7124f9c8ad5a50d0ff5331b954e96f0f054016d. No additional blocking finding. Reviewer changes are confined to scratch probes and evidence; production/Git were not edited.

## Contract and code

The implementation compares the first SourceSnapshot's complete SnapshotId with the token head's SnapshotId, including SourceId, revision and digest. It never uses SourceId equality alone. A miss charges the remaining table length before the same first-match linear search. Successful fallback pays the same total Work as before; the lookup allocates no new storage. The selected source is still used for checked slicing and the spelling/payload checks retain their prior order. URI/content/admission consistency remains enforced by the preceding SyntaxBundle validation.

Empty or missing source closures in an externally supplied ParseTree fail in the earlier syntax/origin validation. Thus the private helper's split_first empty case does not introduce an observed public path that bypasses the initial Budget poll. Each foreign bundle still supplies its own source table; the optimization does not add an authority cache or union source closures.

## Executed evidence

- Full fixed engine suite: 61 native and 61 wasm32-wasip2 tests passed. Independent probes: two native/two WASI on the candidate and the identical two native on the base.
- Public ParseTree::validate on 33-node trees with 1/2/8/32 declared source snapshots and the correct one first/middle/last. Every added source intentionally has the same Japanese SourceId (ASCII Rust Unicode escapes preserve it), a different revision and different bytes; the real source uses revision u64::MAX. All source orders remain valid and the tree is unchanged.
- Six invalid public inputs preserve the same typed error across base/candidate: empty source table; different digest without the referenced source; wrong revision without the referenced source; same identity with conflicting URI; duplicate identical snapshot; conflicting content under the same SourceId/revision. These are actual public syntax/identity failures, not private helper injections.
- The actual ParseSession source `pair :wrap @x ~y trailing` supplies host and guest bundles. Four independent first/later placements of unrelated same-ID revisions retain successful ownership-aware validation. Removing the guest source table fails even though the host still owns that same full input source. No host-to-guest source substitution occurs.
- Work caps at 0/1/100 and immediately around each measured success, Allocation caps 0/1/100, and cancellation preserve typed sticky stops. A separate fixed 17-unit-step Work sweep compares the old/new fallback outcomes. Changing one all-table precharge into first/rest precharges can change the already-charged Work when the second charge stops; comparison.json records these differences rather than claiming byte-identical stopped Usage. Total successful fallback Work and the failure kind are unchanged, and neither route refunds costs.

## Measured cost

For the 33-node fixture, 16 forms and the final leaf perform 17 textual-head lookups. With a 32-entry table and an 11-byte SourceId, successful first hits save exactly 17*31*(11+41) = 27404 Work: 55207 to 27803. Correct-source-middle and correct-source-last stay 55206 in both revisions. At all table sizes, other native Usage fields remain identical; Work also matches candidate WASI. The tiny order-dependent baseline differences come from unchanged source admission/index work and are not attributed to this optimization.

The actual foreign fixture is unchanged at 2473 Work when both real sources are later; when both are first, Work changes from 2472 to 2256. Native allocation remains 2429 in all four placements. These measurements support a first-source locality optimization, not universal speedup or complete Doc acceptance.

## Reproducibility and limits

Source manifests bind 216 foundation/Cargo/toolchain files per revision. All production bodies are verified unchanged; the independent test source is byte-identical across revisions. Scratch Cargo workspace membership is reduced to the four foundation crates and probe, and external dependency versions/checksums are verified against the original lock. Exact source delta hashes, probe sources, raw logs, commands, versions and comparisons form the compact archive; full source copies and targets are excluded.

No independent full-Doc corpus, first-CBOR replay, no_std ARM build or complete cross-target product acceptance is claimed here. The base's separately reviewed parent-lookup change is not re-reviewed. Existing engine managed portable tests remain exercised; the new source-order probes call the public native validator and an actual parser-produced foreign tree.
