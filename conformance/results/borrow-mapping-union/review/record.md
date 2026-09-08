# Borrowed mapping union independent review

Reviewed commit `2b06d1c8077a65c0af45c250f914b07690bb59d3` against `0b4f7e32e5480cb08ab6a2cfb121b8e33fa196d7`. All 2,566 tracked files from each commit were frozen via Git archive into review-only directories. The three-file production delta is preserved in delta.diff. No production files or Git state were changed.

## Conclusion

No blocking finding in this bounded change. The union proof borrows both complete slices; it does not infer validity from membership in either slice. Geometry and Exact text checks, cycle analysis, reused source closure validation, and reverse-path containment all iterate the same ordered chained union. The private already-accepted empty-artifact shortcut predates this change. The new public proof remains tied to immutable borrowed mapping slices; it is not a portable execution authority.

Reader artifact validation uses this proof before application. Source combination and declaration validation remain in the provider boundary. Invalid replies do not consume the pending slot; successful append operations precharge their storage before publishing collected artifacts. The two-slice change does not modify either mapping table, source bytes, span coordinates, outcome, or rollback checkpoints.

## Executed checks

- Frozen core and reader complete native and wasm32-wasip2 suites passed. Full raw logs and command/exit records are retained. This includes the managed 512-graph oracle in both orders and every split, geometry-invalid entries in either part, missing reused source closure, unrelated reverse paths, long SourceIds under a 4 KiB allocation cap, reader native/NDF transform outcome and retry tests. Exact aggregate counts are in results.json.
- Three independently authored provider tests passed native and WASI. They use a checked ReaderPlan, a real ReaderSession, two actual provider suspensions and resumes. Unchanged existing registry/context helper scaffolding is copied from the frozen runtime test; new inputs and assertions are independent.rs.
- The first provider accepts input[1,2) -> aux[0,1); the second returns aux -> next with Capture(next). Success requires both mapping parts. Cross-part cycle, unequal Exact bytes, missing next declaration, and an unrelated reverse path are rejected with the observed typed errors recorded in logs. The original Await remains equal and accepts a subsequent valid reply. Final value, source/map order and capture are checked.
- A failed Choice branch that accepted a generated source and map rolls both back. A subsequent reply referring to the former source fails MissingSnapshot; retry succeeds with empty source/map collections.
- Work, AllocationUnits and SourceBytes exhaustion and explicit cancellation at the pending boundary retain their typed stop reasons and sticky Budget state. The exposed pending continuation remains unchanged. A formal Stopped result, when returned, contains only the prior accepted prefix, never the new map/source. No resource cap was raised.

## Allocation comparison

The exact same independent source and existing scaffolding ran against both frozen commits. On native, the clean second resume with approximately 10 KiB source names requested 53,769 allocator bytes before and 23,083 after (30,686 fewer); logical AllocationUnits were 82,804 -> 51,702 and Work 310,803 -> 280,779. Both versions produce the same successful data and reject the same independent negatives. The observation counts requested alloc/realloc sizes only during resume; it is not peak/live memory, a whole-document performance claim, or a physical OOM guarantee. Source construction and reply preparation are outside the measured interval. Mode 3 has prior source-admission cache effects and is not used for the clean comparison. WASI results confirm semantics but allocation sizes differ by target layout.

## Boundaries and limitations

The new independent cross-part tests exercise the native typed provider boundary on both targets, not a newly built serialized ReadReply transport. Existing managed actual native/NDF Transform tests were separately executed successfully. No new wire schema or codec changed. Full workspace, browser and baremetal execution are outside this review; native/WASI core+reader evidence does not imply all consumers or document workloads complete. The inherited pointwise cycle algorithm and pre-existing source/identity comparison accounting were not redesigned by this delta. No throughput claim is drawn from concurrent test timings.

Source hash manifests, compiler/runtime versions, Cargo registry dependency equivalence, exact commands, raw output and independent probe source are retained. Finalization rechecks every frozen source file and confirms the before/after probe bytes are identical. Early exploratory tool output was truncated; final managed runs were repeated unchanged solely to capture complete evidence logs.
