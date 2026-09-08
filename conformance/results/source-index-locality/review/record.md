# Source index locality hint independent review

Candidate `58d622c5d0909475dc8be24bbd3e89c80627ae06`, formal base `79f35ca22848c1e2643108540010a0b458d52bed`. Both complete 2,771-file trees were frozen from Git. The rejected tail-only experiment bea17fd is not the comparison base and is not included. No production or Git change was made during this review.

## Conclusion

No additional correctness blocking finding within the executed scope. Search hints remain private ordering metadata; they are not source admission, content equality, closure authority or validation proof. The implementation checks the complete source/revision key at the hinted position and the necessary neighbor, with UTF-8 name/revision comparison work precharged. It then either returns the proven exact position/gap or searches the remaining bounded interval. Greater/less branches exclude only already-compared endpoints. Duplicate content/URI/digest equality remains in existing callers. Failed or stopped insertions do not update the hint. Successful unbudgeted, owned-budgeted and borrowed-budgeted insertions/duplicates all update it. Atomic edit preparation does not consult a prior hint; the final commit assigns None together with the new sorted index.

## Executed coverage

Full core managed suite: 83 native and83 wasm32-wasip2 tests successful. Independent six-test probe: native/WASI both successful. Identical probe source also runs successfully against the formal base. Raw logs, command records, compiler/runtime versions and source SHA manifests are retained.

- All permutations of five source keys, including one name at revision0 and u64::MAX, prefix names, and Japanese UTF-8 names:240 owned/ref cases. Every insertion preserves public insertion order and get_ref/get_revision/resolve/latest results. Independently reconstructed equal snapshots deduplicate; same key with changed URI or changed content/digest rejects atomically.
- 2,640 Work/Allocation fault attempts per target:182 native/150 WASI typed sticky stops, all store contents and lookups preserved. Empty/nonempty cancellation for owned/ref, plus a real9,000-byte Japanese SourceId (ASCII Rust escapes) verifies complete comparison charging.
- Seven mixed sequences use unbudgeted/owned/ref APIs, move hints via successive duplicates, and insert/query missing keys at both ends and interior gaps. Atomic source edits retain old revision, publish the next revision and correct changed bytes, and preserve every lookup.
- Two stores with different prior hints undergo the same edit and have identical subsequent measured search Usage; old hints do not survive commit. Five rejected/stopped operations, including failed edit, preserve subsequent search behavior relative to untouched controls.

## Performance measurements and limits

The following are logical Work on512-entry native inputs, not wall-clock benchmarks. Source construction is outside the measured insertion Budget. No cap was increased.

| Input | Formal base | Candidate |
|---|---:|---:|
| Owned ascending insert | 61,115 | 8,687 |
| Owned descending insert | 200,465 | 139,503 |
| Owned permutation `(i*197)%512` | 126,282 | 139,168 |
| Ascending insert below pre-existing z bound | 62,531 | 14,319 |
| Ascending duplicate sequence | 70,331 | 18,073 |
| Alternating first/last duplicate | 83,200 | 91,904 |

Borrowed inserts add512 Work to each corresponding ordinary insertion row in both versions. Native insertion AllocationUnits remain8,192, WASI4,096; duplicate-only loops allocate0. At16,000 Work the base stops at169 accepted entries/15,997 Work while the candidate accepts512 at8,687. The candidate improves locality-based inputs but increases the measured permutation by10.20% and alternating duplicate sequence by10.46%. Do not advertise a universal improvement or unchanged stop positions for arbitrary inputs. Root reports actual corpus improvements; that corpus was not independently rerun here and remains separate evidence.

## Probe correction and unverified scope

Initial independent edit setup used the full snapshot digest instead of the digest of the replaced byte range. Both old and new implementations correctly rejected it with ExpectedDigest. The original probe/logs are preserved in probe-draft-error/. The probe was corrected to Digest::of(b"b") for span1..2 and Digest::of(b"a") for span0..1; the same corrected probe was rerun on both versions. This was a reviewer fixture error, not a production regression. Final source trees remain unchanged.

No full workspace/browser/baremetal or document completion is inferred from these core tests. No new source-byte allowance or schema/wire identity is introduced. Cache/proof design is outside this slice. Public SourceStore debug output and its private in-memory size can reflect the new field; no wire layout uses it. The logical cost intentionally depends on prior successful insertion/duplicate history, and deterministic identical call sequences were compared rather than treating differently constructed stores as equal-cost operations.
