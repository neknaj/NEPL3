# Retained syntax proof review

Fixed commit `2ea3c47d06cf1359769b742e783b4195a04aa454`, base `bc1d5d3`. All 2282 tracked files were extracted with Git archive, with the six changed paths and every SHA recorded in sources.json. Only independent test files were added inside this scratch workspace. No production/Git files were edited.

## Contract and implementation

The new field retains the `ValidatedSyntaxBundle` returned by the existing call in `ParseTree::validate`; it does not introduce a second constructor or change the validation order. The sole mint remains the successful end of ParseTree validation, after profile, source/schema, context, selection, recovery and foreign checks. The stored tree and syntax both borrow the same immutable tree. Private fields prevent external assembly or attachment to a different tree. The getter itself performs no new operation or allocation, so it neither polls nor charges; callers still perform their own operations under current budgets.

The existing SyntaxBundle proof does not bind a borrowed registry or certify environment content digests. The new getter explicitly preserves that limitation. It is not a reusable credential to bypass a new operation's admission. Doc `lower::run` still calls `validate_with_sources` using the caller's registry and decoder admission before lowering. Both prefix and literal paths share that boundary. The tools export/pages/projection closures obtain the proof only from `with_named_input`, whose actual parse is followed by `tree.validate` against the same resolved profile before invoking the closure. All three consumers keep their existing fresh lower budget/codec/admission. Only the redundant intermediate same-operation syntax validation is removed.

Native independent production API testing uses three fixed original inputs: explicit Strong/Text prefix, Japanese literal Ruby+Anno, and ordinary literal prose. Each runs owned and native parser dispatch. The proof addresses its exact tree bundle, getter usage is unchanged, and separately minted syntax proof yields identical Doc value and lower Usage. Both parser paths produce equal Doc values; their parse costs are intentionally different. Every export manifest parse Usage field agrees with the independent native entrypoint's actual accounting, with no copied numeric golden.

Independent negatives cover copied tree with different profile digest, removed source declarations, invalid root; an empty finalized registry at the next lower operation; and fresh Source/Work/Allocation/Depth/Nodes zero limits plus Cancelled through actual lower. Each stop is typed and sticky. Retained proof remains usable afterwards with a fresh legitimate operation, which incurs its own source admission and work. The compilation-negative probe attempts to mutate the raw tree while its retained proof is still used.

The existing four export tests additionally cover deterministic filesystem artifacts/digests, invalid and excessive input, shell depth 250 success and 251/252 rejection, and actual two-page shell generation. Full workspace, full Doc55, original authored00 HTML and CI are root evidence, not rerun or claimed as independent here.

## Probe expectation corrections

The first unlogged preliminary run incorrectly expected the separate narrow Markdown projection to handle Strong and Ruby/Anno, and returned `Unsupported { node: 1 }`. Inspection of the existing projection match and spec21's bounded subset established that refusal is required, so the independent expectation now asserts it. This is not a production defect or a newly broadened implementation.

The next saved run expected a single terminal LF for the ordinary Markdown paragraph but got its existing blank-line delimiter. `probe-expectation-before.log` and `.rs` preserve that failure. The corrected assertion compares exact heading/paragraph content while allowing terminal LF separators, whose multiplicity does not alter that Markdown content. Production was not changed to match either expectation.

## Execution records

Rust/Cargo 1.97.0, Wasmtime44.0.1. `run.py native` and `run.py wasi` save full command, target, exit status, timeout and full output SHA separately. Each subprocess has a 600-second deadline. Production source hashes are verified again when closing the evidence manifest. Native/WASI usage is compared within the corresponding target; allocation byte counts are not asserted equal across pointer widths. The getter is not a portable proof codec, and no raw NDF can mint it without the existing validation boundary.

Final result: independent1 + managed export4 passed on native and wasm32-wasip2/Wasmtime44.0.1. The negative lifetime probe was rejected with E0502 at the attempted mutation while the proof remained live. The first Cargo-check attempt timed out at 300 seconds during concurrent target compilation; it is not a successful lifetime test. A direct rustc attempt initially used the wrong ParseTree module and produced E0425 (saved separately); after correcting only that probe type path, direct rustc produced the expected E0502. These setup failures are separate from production behavior.

No new blocking defect was found within this retained-proof/export-reuse slice. The successful fresh operation and stop checks do not certify unchecked environment hashes, arbitrary future proof consumers, complete Doc migration, or human review.
