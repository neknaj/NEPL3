# Doc page-set implementation checkpoint

Worktree: `feat/doc-page-links`, based on `b303282` (reviewed markup routes
plus the CI correction from `a12270f`). Existing worktrees were preserved.
Root implemented; independent reviewers inspected fixed source snapshots.

Implemented production paths: typed PageSet and link plan, canonical NDF
receivers/revalidation, Doc HTML page-set rendering, actual output-anchor
checks, and host multi-file export from real NEPL3 inputs. Page source IDs
reserve distinct decode namespaces; the host implementation identity includes
that source pipeline. Core remains no_std + alloc with no filesystem access.

Root validation:

- Full native workspace: 468 passed, 1 ignored, 0 failed. This run preceded
  the final host identity inclusion and expanded portable filename negatives;
  relevant targeted tests were rerun afterwards.
- Final core PageSet WASI: 5 passed. Actual source-to-pages WASI: 2 passed.
- Final native host export: 3 passed. Format, Clippy and both schema
  generators passed. See `root/pages-checks.json` and individual raw logs.
- Initial repository check rejected archived intentionally malformed JSON.
  Raw negative fixtures now retain bytes under `.raw`; original paths and
  hashes are mapped in archive.json. Final repository check passed. This is
  an archive representation correction, not a runtime success substitution.
- Chromium and Firefox: 4 JavaScript-disabled navigation checks covering HTTP
  under a non-root path and file URLs. Each followed the actual index link,
  loaded nested CSS and verified 6 sections, 1 list, 6 items, 10 inline code
  elements, no script and no horizontal overflow at 1200 pixels. WebKit and
  browser CI execution are not claimed.

Independent evidence is under core/, host-identity/, html/, host/, migration/.
These contain fixed-source hashes, execution commands, raw failures and
corrected probes. Executable binaries are omitted; original binary identities
remain in reviewer manifests. Leading UTF-8 BOM normalization and invalid
fixture suffix changes are explicitly mapped in archive.json.

The first migration candidate is generated from doc/spec/00-contract.md and
was compared independently with both local and multi-page HTML: 23 ordered
blocks, 10 inline code elements and a 6-item list match. Review found unsupported
Markdown handling gaps (hard breaks, entities, indented code, setext headings,
code-span whitespace and loose lists). The converter now rejects these instead
of flattening them; managed and independent negative cases passed. The current
candidate's bytes did not change because its original source has none of those
features. CI now checks candidate drift and generates the linked artifact on
each native OS; actual remote execution for this new commit remains pending.

This does not complete T21, T19/T20, full PreparedArticle, guest/asset resolution,
or the four-language product. Markdown remains canonical pending stable legacy
anchors, a Markdown projection and human meaning review. Pages deployment and
recovery are not implemented by this checkpoint. Runtime acceptance states are
not changed merely because these scoped tests pass.
