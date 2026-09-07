# Independent SourceSnapshot sharing review

Reviewed fixed copies of source.rs, source/edit.rs, syntax/copy.rs, reader/runtime/copy.rs and core contracts.rs. Full 95-file dependency/input manifest is files.json; focused hashes are reviewed-files.json. Live bytes still matched every copied file after tests. Production and Git were not modified.

No additional blocking finding in this bounded storage change.

Native: core contracts 19 + edit 5 + wire source 3 + independent shared_review 3 = 30 passed. WASI/Wasmtime: same except native thread test excluded, total 29 passed. Exact commands are cargo test --locked -p nepl3-core --test contracts --test edit and cargo test --locked -p nepl3-wire --test source --test shared_review, with --target-dir ../target and WASI --target wasm32-wasip2 / runner wasmtime. Independent probe includes UTF-8 BOM/CRLF/multibyte, shared clone allocation pointer, retained full-text Work bound and zero SourceBytes, separate live allocations on each fresh CBOR decode with content Eq, once-per-admission SourceBytes and fresh-operation charge, immutable old snapshot after edit, and native Send+Sync/thread ownership. Existing edit test sweeps every Allocation limit and verifies sticky stop, no partial publication, and fresh retry. Wire negative tests cover malformed digest/URI identity, scalar boundaries, zero source budget, and preserved retry admission semantics.

core + reader cargo check succeeded for thumbv6m-none-eabi (no target_has_atomic ptr: String fallback) and wasm32-unknown-unknown. These are compile checks, not device/browser runtime. WASI cfg contains ptr atomics and exercised shared text. Native tests emit two scratch-only unused-helper/import warnings inherited from the wire test setup; production warnings are not inferred.

Static: private immutable text cannot diverge from ID/digest through public mutation. Arc wrapping is charged before allocation; source edit commits admission/store only after all output construction and Arc charges. IDs/URI remain owned and compared. CopyCost delegates to common charge_clone; keeping full text Work is necessary because continuation equality can compare separately decoded allocations. Wire reconstructs full content and validates digest and UTF-8; sharing is neither wire identity nor source authority.

Official reference: https://doc.rust-lang.org/alloc/sync/struct.Arc.html (Arc clones share storage; Send/Sync conditional on T; pointer-atomic target availability), https://doc.rust-lang.org/reference/conditional-compilation.html#target_has_atomic . No dependency or schema changes are introduced here.

Not claimed: full linear-combination document completion, performance resolution under unchanged default caps, engine/full reader runtime suites, physical non-atomic execution, or portable provider authentication. Root-reported full-document WorkLimit remains unresolved and is separate from these storage correctness checks.

## Final specification confirmation

Root corrected the minor target qualification noted in spec-addendum.md. Final doc/spec/02-foundation.md SHA256 d5d1901e524c442113fecbdacc15bbaa5dacb5bb075703518a86e418b8279e14 explicitly distinguishes shared-target identity/URI/slot allocation from non-atomic full-text copying. Both added paragraphs match the implemented cfg branches and retained full-text Work/admission/immutable-edit contracts. No remaining revision request. Root owns the separate full-workspace gate; it was not independently repeated.
