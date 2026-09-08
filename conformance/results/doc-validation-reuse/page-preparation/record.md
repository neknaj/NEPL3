# Independent review: Doc page preparation reuse

Reviewed before abf9b4b608adeccb7a9bfb087afd276e7577c07d and after
8b6088e0ff818fae5d8176bc2690dbba42ed43f5. No blocking defect found in this
bounded change. No production, canonical documentation, or Git state was edited.

## Source and contract review

The seven-file diff preserves registration checking, all-document structural and
boundary validation, PageSet hash, each document's label/hash/requirements, then
link resolution. The new accessor only borrows a child from the internally
generated PageSet NDF. Its shape checks alone are not an external schema proof;
its sole caller has just constructed and validated that value. Domain bytes and
canonical child values are unchanged. No hash-of-digests substitution occurs.

CheckedPages retains the exact immutable PageSet borrow and a private digest
vector in registration order. There is no public raw constructor or decoding
path that mints this proof. HTML resolves its actual request first and uses those
digests while retaining the proof. The existing raw plan and rendered-output
receivers still reexecute and compare. Registry, codec, admission and Budget
remain those of the same operation. The constant-time public digest getter uses
checked u64-to-usize conversion and bounds checking. The managed tests exercise
out-of-range and u64::MAX on native and wasm32.

The private requirements function receives CheckedLabels and retains the original
arena order, foreign owner order and foreign closure digest checks. It does not
evaluate Code. New vector pushes precharge Work and Allocation. Changed stops
and lower usage are expected; universal memory improvement is not claimed
(inspect now keeps its label proof alive while encoding). The five added
specification sentences and their Ruby-annotated manuscript counterparts match
these constraints and preserve the surrounding source bytes.

## Independent execution

Each snapshot was extracted with git archive and every one of its 3430 tracked
files checked against the exact Git bytes. Only html/tests/local.rs has an
appended independently written test module, saved as probe.rs. Separate target
directories bind builds to each snapshot. Native and wasm32-wasip2 runs both pass:
before 29 tests, after 30 tests, including four independent tests. After's actual
tools parser/lower/pages tests also pass 2 native and 2 WASI. Commands, complete
logs, tool versions, test binary hashes and per-file compressed manifests are
preserved. These are focused tests, not a rerun of the full workspace gates.

The independent typed fixture has two cross-linked pages, Unicode labels,
special HTML characters, CRLF, and source-less/source-derived variants. Expected
PageSet and Document digests are independently assembled from the specified
domain plus actual canonical CBOR bytes. HTML, all element-origin mappings,
request/reply packet digests, semantic failures and foreign requirements produce
14 identical output lines across before/after and native/WASI. First receivers
start with an empty store and fresh admission. Altered options, route and text
reject stale rendered values; a schema-valid link-list omission cannot turn a
raw plan into a proof.

Seven independently selected source/label/registration cases check whole-set
boundary precedence before earlier-page duplicate labels, missing source,
legitimate identical cross-document source sharing, conflicting same revision,
duplicate page registration, and absent page destination. A valid common guest
graph deliberately lacking Article meaning remains a Code requirement; fresh
CBOR preserves its plan. Missing guest source and changed owner identity reject.
These typed fixture positions are legal structural data, not a claim that the
fixture text was parsed into the nodes. Actual parsed sources are covered by the
separate managed tools tests.

Six resources (SourceBytes, Work, Allocation, Nodes, Depth, Output) each use nine
caps derived from a fresh successful operation: eight stopped cases and one
success, totaling 48 stops and 6 successes per run, plus pre-cancellation. Retry
retains sticky stop and all input values remain equal. Diagnostic/Event limits
and all possible stop windows are not claimed.

For the sourced small fixture, native Work falls 70,921 to 44,699 and Allocation
232,419 to 99,151; Output falls 160 to 96 (SET plus two Document digests rather
than duplicated Document digests). WASI uses different allocation units, with
the same Work and Output. This comparison's SourceBytes is zero because the
independent expected-NDF setup already admitted sources; the stop sweep uses
fresh admissions and tests SourceBytes separately.

## Preserved review infrastructure failures

Initial before/after invocations shared a Cargo target directory. The native
after run silently reused before core artifacts (29 tests, unchanged old Work),
so those logs are retained as shared-target-* and are not after evidence. The
definitive reruns use target-before and target-after and show 29 versus 30 tests
and the expected usage change. One local comparison command also omitted UTF-8
and failed with cp932 decoding before comparing any outputs; it was rerun with
explicit UTF-8. Neither is a production failure. initial-before-native retains
the earlier three-probe run; the final runs include four probes.

## Separate large Reader phase observation

The recovery-addition source is exactly 69,199 bytes, SHA256
162fe9ec7faf8c671ef3769fc27342209577db38084d0201220b7871c430bab3.
The small phase workspace is a separate full archive of 8b6088e. Baseline adds
only a tools-side Usage print after render_pages returns. Detailed observation
adds fixed atomic storage in scratch core and phase markers, enabled after
parse/lower; it never mutates Budget. Actual command, input, before/after
instrumentation, binary hashes and full logs are saved. Both builds return exit
1 and Stopped(WorkLimit), with all eight Usage fields exactly equal:
Source 72,485; Work 99,999,972; Depth 14; Nodes 3,815,426;
Allocation 203,614,047; Output 64; Diagnostics 0; Events 0.

The stop is in resolve's Document digest, after the generated NDF child accessor:

| Phase | Work at entry | Work at next boundary |
|---|---:|---:|
| set_to_value | 54 | 9,977,358 |
| PageSet digest | 9,977,358 | 54,525,452 |
| labels (structure plus declarations) | 54,525,452 | 60,502,279 |
| child accessor and Document digest | 60,502,279 | 99,999,972 (stop) |

Requirements, link resolution, HTML building, anchor verification and shell
serialization are not reached. The final Document digest is incomplete; Output
64 includes precharged digest output and does not mean two successful hashes.
Reusing the prior structural proof can remove part of the 5,976,827 labels Work,
but that interval also includes semantic label work. This measurement does not
establish that such a future change will make the complete operation fit.
The original root run lacks Usage, so equality is between the minimal baseline
and detailed scratch runs; its original WorkLimit and source bytes are retained.
No caps were raised, and neither this longer document nor a future proof change
is reported complete. The root's shorter 62,074-byte success is separate evidence
and was not independently rerun in this review.

Replay: setup.py creates fresh before/after directories; run.py MODE TARGET adds
the probe and executes tests. run-tools.py TARGET executes the two real-source
tests. phase-instrument.py is applied only after the saved baseline tools delta
to a separate archive; phase-run.py records both phase executions. verify.py
checks existing snapshots, outputs and phase equality. The evidence manifest
excludes extracted workspaces and build directories; compressed file manifests
bind all dependencies to Git and binary-hashes.json binds actual test artifacts.
