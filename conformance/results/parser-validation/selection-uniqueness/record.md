# Independent review: completed-tree selection uniqueness

Candidate b41cb039765cb97484a5e77121d552cb76592a70; base 2681cc0e3ba2143be900463db6be9e03abe9cffe. No additional blocking finding. Production/Git were not modified by this reviewer.

The changed implementation was read directly. Every node must first be reachable; selection count must then equal the local bundle's node count. Therefore consuming the already all-true local bitmap rejects a repeated in-range NodeRef and, with equal counts, establishes coverage without another allocation. The bitmap is created per bundle and discarded after that bundle. Its consumed bits are not used by subsequent field selection lookup. Input selections, immutable tree fields, semantic/execution identity checks, and the foreign path lookup are unchanged. Full u64 conversion and bounds validation precede indexing.

## Independent execution

The fixed engine suite passed 61 native and 61 wasm32-wasip2 tests. Two added public-API probes passed native/WASI on the candidate and native on the base. Core/reader/wire/engine production bodies are fixed Git bytes; probes live in a separate scratch crate.

- Public ParseTree::validate was exercised on 1, 3, 9, 33, 129 and 257-node trees in ascending, reverse and permuted selection orders. Source identity uses Japanese characters via explicit Rust Unicode escapes and revision u64::MAX. All successful tree values remained unchanged.
- All 125 triples drawn from node IDs 0/1/2/3/u64::MAX were compared exactly between the base and candidate, and with candidate WASI. The six complete permutations succeed; all remaining cases reject with identical typed outcomes. Missing count, missing context, unreachable nodes, empty malformed syntax, duplicate selection and invalid alias precedence were also checked.
- Error ordering remains meaningful: a parent missing its child's selection can produce Selection before a later duplicate is examined; a duplicate leaf selection is rejected before its forged alias is checked. An invalid earlier alias remains Profile(MissingAlias). The original draft probe incorrectly assumed unconditional Duplicate priority; both versions reproduced that expectation failure. The retained draft logs are test-author mistakes, not regressions.
- Actual ParseSession input `pair :wrap @x ~y trailing` creates host/guest bundles with overlapping local NodeRef 0/1 but distinct concrete ownership and modes. All eight context/host-selection/guest-selection order combinations validate. Duplicating selections, duplicating the context path or assigning a guest node the host alias rejects; the new bitmap does not mix bundle scope.
- Work limits 0/1/10/100/1000 and immediately around each version's measured complete Usage retain WorkLimit and sticky polling; sufficient limits complete. Allocation limits 0/1/10/100/1000 and cancellation before validation retain their typed stops. Preexisting Work remains charged after cancellation. No cap was increased.

## Cost comparison

For all 18 valid tree/order combinations, Work savings equal exactly n*(n-1)/2 and all other native Usage fields are unchanged. At 33 nodes: 14023 to 13495 Work. At 257 nodes: 196359 to 163463 Work; allocation stays 104264, Nodes 772 and Depth 129. Work is equal in candidate native/WASI (their target-specific allocation charges differ). The real two-bundle foreign fixture changes from 1922 to 1920 Work with unchanged native allocation. No cost regression was observed for these inputs; this only removes this one quadratic uniqueness scan, not every repeated field/context lookup in tree validation.

## Evidence scope

The compact manifest contains source hashes, exact probe files, commands, tool versions and raw logs. Full scratch source copies and build products are excluded. Original external Cargo dependency versions/checksums were checked; scratch workspace membership is limited to the four foundation crates and the probe. A compile-only scratch StopReason-to-Box<Error> conversion was corrected before execution and its original log is preserved.

No independent full Doc corpus benchmark, first-CBOR replay, engine-wide asymptotic claim or canonical migration acceptance is included. Existing engine managed tests retain their original portable coverage. Root's separate Doc benchmarks are not counted as independent review evidence here.
