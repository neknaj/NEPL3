# Canonical Markdown page contexts

Implementation: `14df7e9d77faa86715c4605b28a853812980c4b0`.
This adds the host `doc-canonical markdown <new-directory>` operation and
`nepl3-tools.markdown-annotated-pages/1` canonical registry profile. It uses
actual Doc sources, explicit Markdown destinations and the production PageSet
resolver. It does not discover ambient Markdown files or mark a chapter migrated.

The captured registry, sources and aliases bind a length-framed input context
identity, separate from the PageSet identity and final output hashes. Legacy
single-page projections retain their existing bytes. All pages render before
staging starts; the completion receipt is written last. Known file/directory
prefix conflicts fail before writes. An operating-system failure may still leave
an incomplete staging directory.

## Executed checks

- At fixed clean `8981ba013618c38232fee58ea00b8b479d17ca74`, 12 canonical
  tests and all 86 Doc integration tests passed, with formatting, workspace
  Clippy, current canonical projections and repository checks. Each command
  records a clean, unchanged HEAD before and after execution.
- Final `14df7e9` adds the path-prefix preflight and regression. Eleven canonical
  tests passed, excluding the two expensive real-document tests already run at
  `8981ba0`; formatting, Clippy, current projections and repository checks passed.
- Independent review ran eleven native tests at final `14df7e9`, checked all
  674 source dependencies and its isolated probe overlay, independently rebuilt
  the framed SHA-256 and tested ten identity mutations. Seven sticky stops,
  caller usage/depth, legacy byte compatibility, late failures and public host
  staging/check are covered. The actual four-page Markdown outputs and receipt
  are byte-identical to the reviewed `819f0e9` results.
- The specification change and manually authored chapters 16/21 received
  separate authoring review. Root tested both complete Doc drafts through the
  production parser, lowerer and label checker. This does not establish their
  HTML output or canonical cutover.

Actual four-page output used Work 300,000,000 / Allocation 750,000,000 selected
before entry; other output limits are recorded. Default Work 100,000,000 failed
and remains a failed run. The two full draft checks selected per-phase Work
600,000,000 / Allocation 1,500,000,000 in advance; chapter 21 parsing used
419,222,830 Work. These are logical resource counters, not measured heap bytes.
Parsing/lowering have independent per-page operation budgets. The shared output
budget covers resolution/rendering/metadata, and host input capture is separately
bounded. Receipt usage is explicitly **before receipt serialization**, not final
operation consumption. No global default limit was raised.

## Failures retained

The independent Work1 probe found string copies charged after allocation;
`819f0e9` prepays Work, and the unchanged probe now records Allocation256 for
both 16- and 2048-character page IDs. A legacy-only file-prefix conflict formerly
wrote one file before OS error183; `14df7e9` now rejects it before creating the
stage. Original failures, repaired probes and additional negative cases remain.

Earlier root budget tests failed on the default four-page budget, receipt digit
width assumptions, and the default chapter-21 parsing budget. The initial broad
batch passed 77 library tests (one pre-existing ignored) and 86 Doc tests, but
source changed during that batch. Its explicit scope correction is retained;
it is not fixed-HEAD evidence. The later guarded batches supersede that claim.
Independent probe compilation/schema-input mistakes are also preserved as
failures, rather than counted as production passes.

## Evidence and scope

`payloads.json` records exact lengths/SHA-256, source commit and original restore
paths. Reviewer payloads come only from their strict manifests; no build caches,
binaries or workspaces are copied. Every saved payload has a `.fixture` suffix
and keeps its original bytes; `saved-*` directory names replace reserved archive
components. Restore files using each entry's `owner` and `restore` path before
replay. Reviewer `record.md`, command records and phase-labelled reconstruction
scripts explain which frozen source and probe to use. Do not run an older-labelled
script over newer sources or overwrite archived observations during replay.

This host review ran on Windows native. Independent WASI/Linux/macOS execution,
filesystem race resistance, Pages deployment, chapter-01 cutover and complete
T21/T16 acceptance are not claimed. The earlier typed PageSet WASI evidence is
a separate scope. Catalog statuses remain unchanged.
