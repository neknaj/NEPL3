# Browser source-position slice

This standalone test adapter calls the actual `nepl3-core` SourceSnapshot and
LineIndex APIs. It is not a production API, a JavaScript reimplementation, or a
formal E01 acceptance decision. It has no new production dependencies.

The ABI version is 1. Fixture IDs 0, 1 and 2 identify respectively the fixed
BOM/Japanese/emoji/mixed-line-ending input, empty input, and Japanese text ending
in CRLF. Fixture 3 is the exact catalog `E-unicode` input `a𠮷b\r\n文書`:
14 UTF-8 bytes on two lines. Its supplementary-plane character occupies bytes
[1, 5), UTF-16 columns [1, 3), and UTF-32 columns [1, 2) on line zero.
The shared 228-case oracle includes both conversion directions at its seven
valid boundaries, CRLF/scalar/surrogate interior rejection, and all previous
173 cases unchanged. The native/WASI test also checks that the named catalog
source and range endpoints agree with these executed observations.
Encoding IDs 0/1/2 mean UTF-8/UTF-16/UTF-32. Mismatch IDs 0/1/2/3 select
the original snapshot or a changed revision/content/source ID. Unknown selectors
return an adapter error rather than success.

`source_position` returns `(line << 32) | character`; `source_offset` returns a
byte offset. Results with bit 63 set are errors: 1 Bounds, 2 ScalarBoundary,
3 LineTerminator, 4 Position, 5 SnapshotMismatch, 255 unexpected/adapter failure.
Arguments and results use JavaScript BigInt, never Number for u64 values.
The `source_byte` export binds exact UTF-8 fixture bytes, not merely their lengths.
The Python oracle is fixed data in `conformance/inputs/browser/source-position.json`;
it is not generated from either conversion operation.

On a Linux development host with the pinned Rust target and Playwright engines:

```
rustup target add wasm32-unknown-unknown
python -m pip install -r tools/audit/doc_html/requirements.txt
python -m playwright install --with-deps chromium firefox webkit
python -m unittest discover -s tools/emulators/browser -p 'test_*.py'
python tools/emulators/browser/run.py --repository . --output dist/browser-foundation
```

Commit source first; the runner rejects dirty source and reused output directories.
It builds the Wasm from its own checkout, executes all cases in all three engines,
and retains raw stdout/stderr, command status, versions, source/input/binary hashes,
and observations. Each engine runs with a wall-clock deadline and Linux `/proc` descendant
tracking, including detached browser children. Cleanup uses PID/start-time identity
across reparenting, tries SIGTERM, then SIGKILL and verifies termination; a hung synchronous Wasm call fails instead of being skipped. Failed
runs retain their output and cannot be presented as passed receipts. The runner
is for trusted development inputs, not an arbitrary-code sandbox. Clean-checkout
checks before/after do not detect transient edits or authenticate execution.

Raw logs, `fixture.wasm`, and receipts belong in `dist/` or CI artifacts, not
`conformance/results/`. Artifact retention is 14 days in CI. This browser slice
does not assemble seven-target AcceptanceEvidence, prove LKG/publication, or
change any acceptance status. Missing engines and failed checks are failures.
