# Independent tokenizer native-host / engine review

Production reviewed and built: commit `4c9080cfb6606de12d1dd1b765e768ec02db8d0c`, archived into `final-source-exact/`. The two UTF-8 document corrections were subsequently read at `abbc9ef34d9a7964cb276b32627bddc58cc2c152`; that commit changes no Rust production or tests. No production, documentation or Git files were edited by the reviewer. Scratch helper edits below are test adapters only.

The bounded change is acceptable after the callback-stop fixes and documentation correction. This does not establish completion of the 13 KB Doc example, later source-cost optimization, or a new portable host protocol.

## Independent findings and disposition

The initial five-file snapshot returned an owned Await/Reserve when a provider or reservation callback returned `ReaderError::Stopped(Cancelled)` without first stopping its Budget. The original two independent inputs reproduce this with a still-live Budget in `before-independent.log`. The exact same stop tests pass on final native and WASI: the outcome is Stopped(Cancelled) and Budget.poll returns Cancelled. The engine adapter also extracts nested Reader/Source stop causes instead of matching only direct ParseError::Stopped. Independent actual parsing of `let x y`, at caller depth 7, checks both nested causes, accepted diagnostic count 1, and caller-depth restoration.

The initial rustdoc overclaimed retry for rejected callbacks. An invalid supplied Text reservation with an empty URI returns Source(Locator). The old owned reserve path returns the same error and a subsequent corrected reserve returns NoPending; the native host path does not add retry support for this case. This is an inherited behavior and a narrowed documented guarantee, not a corrected pending-state retry. The original desired-retry test remains in the evidence and is deliberately filtered from the final run. Its replacement independently checks the existing owned/native error parity and restores caller depth. The original before run has three failures: two stop defects and this overly broad retry expectation. No before WASI result is claimed.

An additional final engine reservation test uses the actual source `let "x" y` and both nested error forms. To make the route independently distinguishable, the provider callback for these actions returns a non-stopping Context error; only the reservation callback returns the tested Cancelled cause. Both targets explicitly log reservation callback entry and finish Stopped(Cancelled), with the prior diagnostic retained. An initial scratch version overescaped the input and exercised the provider branch instead: its source/logs are retained as `rejected-fixture-*` and are not evidence of reservation coverage. The corrected version is `reservation_probe.py` and `reservation-probe/`.

The committed specification/progress additions initially contained actual ASCII question marks in place of Japanese. The reviewer reported this separately. Commit abbc9ef restores UTF-8 prose and explicitly distinguishes None, non-stopping callback errors, invalid provider replies, and invalid reservation values. The corrected bytes and Git delta are included in the evidence.

## Code and contract checks

The synchronous tokenizer path still calls the public ReaderSession::resume, preserving ReaderContinuation equality and ordinary provider/schema/source validation. The outer owned tokenizer path retains full echo, scope, state, usage and limit checks; only internally saved nested continuation state is used after the outer check. The optimization avoids publishing the outer tokenizer continuation on successful synchronous calls; it does not waive checking caller-supplied portable continuations.

Provider callbacks run at the saved absolute depth via with_depth_at_least. Engine dispatch resolves the selected profile's ProviderRequirement before invoking the host. None publishes one owned boundary without an immediate duplicate dispatch. Invalid Some provider replies retain the existing engine parse-error behavior (WrongSecond); tokenizer-level malformed reply recovery is a distinct API contract. The engine keeps an accepted checkpoint before moving the current collector into the tokenizer, so downstream stop/error handling can preserve accepted diagnostics and generated closure. Existing copied production tests exercise generated source/event preservation, fallback, malformed replies, echo checks and allocation-stop boundaries. The changed allocation regression measures successful operation cost before sampling lower limits; it no longer assumes that a formerly expensive operation must still fail at 100000 units.

## Executed evidence

| Fixed-source harness | Native | wasm32-wasip2 |
| --- | ---: | ---: |
| Reader runtime plus independent stop/parity tests | 39 passed, 1 explicitly filtered | 39 passed, 1 explicitly filtered |
| Engine parse plus independent nested-provider stop test | 21 passed | 21 passed |
| Additional route-distinguished nested-reservation test | 1 passed, 20 filtered | 1 passed, 20 filtered |

Each nested test covers both Reader(Stopped) and Source(Stopped), but the table counts Rust test functions. Zero-test doctest executions are not added to totals. These are independently built copied production harnesses plus independent cases, not an independent implementation of the parser. The original full workspace gates are the parent's evidence, not recounted here. Scratch harnesses emit unused-item warnings because production test modules are included in a library; this review does not claim a clean scratch Clippy run.

Exact commands and exit statuses are in final-runs.json and reservation-runs.json. Logs, probe source, fixed source archive/inventory and the six executed test artifacts are hashed in manifest.json. The initial five-file before snapshot, original failure log and original native binary remain preserved. Intermediate after-source/ results are not used as evidence of the final commit. The accidentally empty-diff final-source/ folder is also not the source used by the final build; final-source-exact/ is.
