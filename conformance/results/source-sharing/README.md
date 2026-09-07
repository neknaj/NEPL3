# Shared immutable source storage: bounded implementation evidence

Base: `3da38ee70cd454ce66187c223be09d54f2cfec8d`. The implementation changes
SourceSnapshot storage, its construction/edit allocation accounting, and core /
reader copy accounting. It does not change the source schema or relax received
continuation comparisons. The focused implementation hashes are in
`independent/reviewed-files.json`.

Root validation:

- `cargo test --locked --workspace`: 430 passed, 1 existing ignored test.
  Executed in a separate worktree containing the base and this six-file change;
  the uncommitted Doc HTML / native host work was excluded from that worktree.
- `cargo test --locked -p nepl3-core -p nepl3-reader -p nepl3-wire -p nepl3-engine`:
  209 passed on native and 209 passed with `--target wasm32-wasip2 -- --test-threads=1`,
  using `CARGO_TARGET_WASM32_WASIP2_RUNNER=wasmtime run` (Wasmtime 44.0.1).
- Clippy with `--all-targets -- -D warnings` passed for those four crates.
- `cargo check --locked -p nepl3-core --no-default-features --target thumbv6m-none-eabi`
  passed; the four foundation crates also compiled for `wasm32-unknown-unknown`.

Independent review and separately executed native / WASI probes are retained in
`independent/`. The probe can be copied to the wire crate's `tests/shared_review.rs`
to reproduce the source / CBOR / immutable edit checks alongside its existing
test support. `files.json` hashes the archived evidence, not a claim that any
required acceptance group is complete.

Archived text is UTF-8 with LF; log trailing whitespace is normalized.
`normalization.json` retains original and stored hashes for those conversions.
The independent evidence index records its original hashes alongside the stored
hashes; test output content and results are unchanged.

Limits: these are storage-correctness and regression checks. ARMv6-M and browser
checks above are compilation only. This change does not resolve the complete
annotated linear-combination document's resource limits. Its ongoing native HTML
path still reaches WorkLimit with Work=100,000,000; that separate implementation
and failing reproduction remain work in progress. No task or acceptance status
is promoted by this record.
