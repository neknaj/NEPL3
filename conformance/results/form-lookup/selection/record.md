# Independent form-selection reuse review

Candidate 3f23aec87da9bef9e7ac02639d4bb08dafed2aab; base 6f84a51b626de595fb73635ed4b116055edfb9e4. No blocking finding.

Two production files only: engine/src/parse/session.rs and select.rs. Source sets were taken directly from each Git commit; 216 original paths per version are hashed. Review-only workspaces reduce members to four foundation crates and a probe crate. Cargo manifests/locks are the only workspace projections; retained external dependency versions and checksums are unchanged, and unused external packages omitted by Cargo are listed in verification.json. All other frozen source bytes remain unchanged. Production/worktree/Git were not edited.

## Static boundary

`token_selected` evaluates `form` only for category reads (`frame.read == None`). The returned borrowed kind and owned selection/arity stay local to this single call against the same immutable package, token and source. Static forms still precede provider dispatch. A missing static form dispatches only when `skip_head` is false and the selected profile has a head provider. Shape(None) resumes with `skip_head`, repeats the one necessary local form lookup and then selects a leaf or recovers. The old skip-head path also performed one lookup; this change does not cache across callbacks or trust an external selection. Builtin and ListOf frames do not enter `form` and still use their original kind/spelling handling. Removing private `category` leaves no unexamined public API or wire change.

## Executed checks

- Candidate full engine: 61 native and 61 wasm32-wasip2 tests, all passed (4 unit + 14 head + 21 package + 22 parse). Existing managed coverage includes Foreign ownership, continuations/retry, native host failures, schema rejections and text reservations.
- Independent public ParseSession probe: 2 native + 2 WASI, same 2 tests on the base native, all passed. The probe reuses actual package/transport setup from the existing test helpers but adds original inputs, table sizes, call traces, syntax equality and resource assertions. No production-body instrumentation.
- 15 combinations of five inputs and 0/8/64 extra preceding forms: static Let, builtin name spelling `let`, nested Let, ordinary leaf, dynamic choose with selector and child context. Owned read/resume, synchronous native host, host None fallback, sealed completion and actual CBOR head transport are compared. Static heads never call the shape provider; a leaf calls it exactly once before explicit None fallback. Native host None on child0 is retried once through owned Await (both calls recorded), with the same final tree.
- 18 combinations without a registered head provider, with/without matching leaves; 3 explicit unknown-head Shape(None) modes; 9 ListOf combinations (cons/nil, keyword spelling in builtin position, invalid list head). Unknown remains Recovered; no invented leaf arity and no repeated shape dispatch. List cons/nil/bogus never issue shape requests. Child expressions retain their own provider scope.
- 24 fixed Work caps (0 through 30000) across static/leaf/dynamic inputs preserve typed WorkLimit outcomes or complete within allowance. The successful tree is separately validated; therefore some helper errors at the tight caps are from this explicit post-parse check and are not misreported as a parser stop. Public Stopped outcomes and cancellation are checked by charging zero Work afterward: original stop remains sticky and Usage does not change. The original cancellation helper also tests invalid fresh-budget retry and consumed-continuation rejection.
- Existing invalid identity, profile digest, reply kind, diagnostic location and projected fields are exercised at actual head-resume boundaries before retry, rather than treating successful provider output alone as evidence.

## Cost and semantic comparison

All 46 recorded scenarios have byte-equal Debug representations of the full resulting ParseTree (including syntax/source/token/recovery/context data), cursor and provider call trace between base and candidate. All non-Work native Usage fields are equal. Candidate WASI has identical semantic records and non-allocation Usage; native/WASI allocation size differences are retained.

Representative measured Work:

| Input/profile | Base | Candidate |
| --- | ---: | ---: |
| 64 preceding forms, `let n x trailing`, head provider | 8364 | 7844 |
| 64 preceding forms, `x trailing`, head provider | 5072 | 5072 |
| 64 preceding forms, nested Let, head provider | 11656 | 10616 |
| 64 preceding forms, `choose alt @let z x trailing` | 24332 | 23012 |
| 64 preceding forms, leaf with no head provider | 3992 | 3596 |
| 64 preceding forms, List cons/cons/nil | 15034 | 14514 |

The static Let saving is exactly `(64 + 1) * (category length 4 + head length 3 + 1) = 520`; two static Lets save 1040. Leaf Shape(None) sees no saving because dispatch and resumed fallback still require distinct calls, as before. List-case saving is only the enclosing static Let lookup; no list scanning was introduced. No regression was observed in this bounded matrix. This is not an independent full-Doc corpus speedup or completion claim.

## Evidence limitations and scratch corrections

Initial probe compile mistakes (tuple returns/field names) are saved in initial-compile.log. The first portable fixture attempted its dynamic-only tree mutation check on a static tree and returned `dynamic selection`; initial-fixture.log is retained. That helper is now invoked only for the dynamic example; actual CBOR head transport still runs for static-leaf None and unknown cases. These are reviewer fixture issues, not production findings. An intermediate WASI log is not final evidence: an accidental overlapping invocation shared its log during compilation. Final WASI was rerun alone after both earlier processes ended; its final source and log are hashed. No production caps, golden expectations or functionality were changed to pass tests. Final probe also checks sticky stops and compares full trees rather than only node kinds.

Artifacts: source-manifest.json, before-manifest.json, delta.diff, probe/, five final command/result JSONs and logs, comparison.json, verification.json and versions.log. Build products and full repository copies are excluded from the compact manifest.
