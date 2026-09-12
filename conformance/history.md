# Historical evidence in Git

Archive revision: `d87ca8c4ba18e74bc71efb0e2958af87ac971e1b`. Permanent ref: `archive/evidence-2026-09-13`.

These closed review archives are retained in Git rather than copied into the current source tree.
They are historical observations, not current acceptance, publication or LKG assertions.
No current production/test input or task/review reference uses these directories.
Restore the **entire baseline** in a separate checkout to inspect cross-archive manifests;
do not mix a partial restoration with current source, or execute archived review scripts as current tests.

```sh
git fetch origin archive/evidence-2026-09-13
git worktree add --detach ../NEPL3-history d87ca8c4ba18e74bc71efb0e2958af87ac971e1b
```

Keep the archival ref when removing unused worktrees. The original bytes and root .gitattributes,
source snapshots, raw logs and manifests remain available without rewriting their seals.

| Archive | Original Git tree |
| --- | --- |
| `actions-update` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/actions-update) |
| `binding-analysis` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/binding-analysis) |
| `borrow-mapping-union` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/borrow-mapping-union) |
| `custom-query` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/custom-query) |
| `doc-adjacent-lists` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-adjacent-lists) |
| `doc-annotated-blocks` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-annotated-blocks) |
| `doc-annotated-markdown` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-annotated-markdown) |
| `doc-annotated-pages` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-annotated-pages) |
| `doc-authoring-18` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-18) |
| `doc-authoring-adrs` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-adrs) |
| `doc-authoring-delivery` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-delivery) |
| `doc-authoring-domain` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-domain) |
| `doc-authoring-entries` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-entries) |
| `doc-authoring-foundation` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-foundation) |
| `doc-authoring-inventory-references` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-inventory-references) |
| `doc-authoring-progress` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-progress) |
| `doc-authoring-review` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-review) |
| `doc-authoring-rp2040` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-authoring-rp2040) |
| `doc-canonical-architecture` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-architecture) |
| `doc-canonical-circuit` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-circuit) |
| `doc-canonical-contract` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-contract) |
| `doc-canonical-doc-html-audit` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-doc-html-audit) |
| `doc-canonical-extensions` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-extensions) |
| `doc-canonical-html-budget` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-html-budget) |
| `doc-canonical-html-delivery` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-html-delivery) |
| `doc-canonical-html-fragment` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-html-fragment) |
| `doc-canonical-math-html` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-math-html) |
| `doc-canonical-math-regression` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-math-regression) |
| `doc-canonical-model-invariants` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-model-invariants) |
| `doc-canonical-page-context` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-page-context) |
| `doc-canonical-source` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-canonical-source) |
| `doc-core` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-core) |
| `doc-external` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-external) |
| `doc-host-capacity` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-host-capacity) |
| `doc-input-paths` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-input-paths) |
| `doc-labels` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-labels) |
| `doc-local-html` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-local-html) |
| `doc-markdown-breaks` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-markdown-breaks) |
| `doc-markdown-code` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-markdown-code) |
| `doc-markdown-sentences` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-markdown-sentences) |
| `doc-mixed` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-mixed) |
| `doc-native-host` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-native-host) |
| `doc-output-budget` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-output-budget) |
| `doc-pages-acquire` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-acquire) |
| `doc-pages-artifact-input` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-artifact-input) |
| `doc-pages-ci-candidate` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-ci-candidate) |
| `doc-pages-ci-observations` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-ci-observations) |
| `doc-pages-create-transport` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-create-transport) |
| `doc-pages-creation-gate` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-creation-gate) |
| `doc-pages-deployment-receipts` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-deployment-receipts) |
| `doc-pages-download` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-download) |
| `doc-pages-execute-attempt` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-execute-attempt) |
| `doc-pages-integration` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-integration) |
| `doc-pages-journal` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-journal) |
| `doc-pages-journal-remote` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-journal-remote) |
| `doc-pages-macos-fixture` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-macos-fixture) |
| `doc-pages-poll` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-poll) |
| `doc-pages-prepared-upload` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-prepared-upload) |
| `doc-pages-receipt-journal` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-receipt-journal) |
| `doc-pages-record-smoke` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-record-smoke) |
| `doc-pages-recorded-wait` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-recorded-wait) |
| `doc-pages-recovery-payload` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-recovery-payload) |
| `doc-pages-recovery-release` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-recovery-release) |
| `doc-pages-remote-wait` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-remote-wait) |
| `doc-pages-smoke` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-smoke) |
| `doc-pages-stage` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-stage) |
| `doc-pages-status-journal` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-status-journal) |
| `doc-pages-status-transport` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-status-transport) |
| `doc-pages-stored-payload` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-stored-payload) |
| `doc-pages-submit-attempt` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-pages-submit-attempt) |
| `doc-phase-limits` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-phase-limits) |
| `doc-plain-text` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-plain-text) |
| `doc-project-guides` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-project-guides) |
| `doc-projection` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-projection) |
| `doc-proof-reuse` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-proof-reuse) |
| `doc-reading-style` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-reading-style) |
| `doc-remaining-drafts` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-remaining-drafts) |
| `doc-resource-links` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-resource-links) |
| `doc-root-guides` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-root-guides) |
| `doc-self-fragments` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-self-fragments) |
| `doc-sentence-owner` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-sentence-owner) |
| `doc-signature-projection` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-signature-projection) |
| `doc-validation-reuse` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-validation-reuse) |
| `doc-webkit-layout` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/doc-webkit-layout) |
| `editor-regions` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/editor-regions) |
| `facts-envelope` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/facts-envelope) |
| `form-lookup` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/form-lookup) |
| `global-binding` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/global-binding) |
| `grammar-diagnostics` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/grammar-diagnostics) |
| `grammar-shared-kind` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/grammar-shared-kind) |
| `html-anchor-identifiers` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/html-anchor-identifiers) |
| `markup-routes` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/markup-routes) |
| `native-head` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/native-head) |
| `native-host` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/native-host) |
| `native-reader-storage` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/native-reader-storage) |
| `nested-continuation` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/nested-continuation) |
| `parser-validation` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/parser-validation) |
| `portable-ci` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/portable-ci) |
| `portable-head` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/portable-head) |
| `portable-tree` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/portable-tree) |
| `reader-checkpoints` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/reader-checkpoints) |
| `reader-source-prefix` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/reader-source-prefix) |
| `reader-transform` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/reader-transform) |
| `recursive-foreign` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/recursive-foreign) |
| `region-query` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/region-query) |
| `rename` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/rename) |
| `report-codec` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/report-codec) |
| `seed-import-stack` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/seed-import-stack) |
| `session-source-index` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/session-source-index) |
| `shared-lexical` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/shared-lexical) |
| `source-edits` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-edits) |
| `source-index-locality` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-index-locality) |
| `source-lookup-indexes` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-lookup-indexes) |
| `source-map-adjacency` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-map-adjacency) |
| `source-map-locality` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-map-locality) |
| `source-map-order` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-map-order) |
| `source-reference-index` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-reference-index) |
| `source-sharing` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/source-sharing) |
| `stream-digest` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/stream-digest) |
| `tokenizer-native-host` | [Original records](https://github.com/neknaj/NEPL3/tree/d87ca8c4ba18e74bc71efb0e2958af87ac971e1b/conformance/results/tokenizer-native-host) |

Boundary totals: 120 directories, 8406 files, 154303564 bytes, 876848 JSON/JSON-fixture lines.
Counts describe the archived bytes; they are not new execution or review verdicts.
