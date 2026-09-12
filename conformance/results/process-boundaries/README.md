# Evidence/process boundary correction

Tested source: `1f8b7d6b0587ac54254f42afb38af981a0fe7b2a`.
Command manifests and raw logs were collected under `dist/repository-after-archive`
and `dist/site-tests-after-archive`, using `tools/conformance/runner.py` at the
tested revision. They record exact arguments, revision, environment, outcomes and
SHA-256. Raw logs are not retained as permanent Git source. CI stores the site
collector package as an artifact with an explicit retention period; its expiry
must not be mistaken for permanent archival. No source copies or review-specific
executable scripts are included in this record.

- Native Windows format, Clippy, workspace tests,
  repository contracts and task projection checks passed. Rust harness summaries:
  578 passed, 0 failed, 1 existing ignored. rustc and Cargo versions are raw logs.
- 114 host regression tests passed after archive
  removal, including the unchanged real-Doc payload input.

Independent reviews (separate agents; implementation by root):

- Pauli inspected publisher responsibilities, duplicate evidence mechanisms and
  the 95 initially unclassified historical scripts from Git blobs. No additional
  generic framework was warranted; independent expected-value algorithms must
  remain independent of production code.
- Bernoulli verified task dependency/completion semantics, scoped record links,
  CI portability responsibilities and the final archive diff. All 8,406 removed
  files across 120 groups match baseline blobs and the history index. No current
  code/test/task/review reference points to them; the remote archival ref exists.
- Ohm independently ran the seven collector tests and found that `./` and case
  variants bypassed the historical-directory guard. Root fixed normalization and
  resolved-directory checks; Ohm reran all seven tests and confirmed the fix.

Other review fixes: preserved Windows junction, macOS ancestor-path, socket and
process tests in the native matrix; repaired task-table generation; separated
historical scope from current acceptance; corrected an intermediate encoding
error before the first checkpoint commit.

Limits: no archived review program was re-executed. Restore the entire preserved
baseline for old cross-archive verification. Hash verification detects record
inconsistency, not authorship or independent proof of execution. These results
do not mark any complete acceptance group, live Pages publication or LKG passed.
CI provides the separate native-OS/WASI/browser/bare-metal checks; consult the
run attached to the PR rather than infer them from this Windows execution.

The publisher additions from unmerged PRs #120/#121 were subsequently deferred
so that this correction can target main independently. The counts above describe
the explicitly identified intermediate revision, not the final publisher suite.
