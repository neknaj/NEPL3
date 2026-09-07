# Independent review: real local Doc export and browser-log correction

Final bounded result: the reviewed local export and corrected audit helper pass. Two export implementation gaps were independently reproduced and returned to root before completion: missing shell-depth reservation, then the first repair's queue Work/capacity accounting. Root implemented the corrections; the reviewer made no production or Git edits. No unresolved blocker remains in this tested scope.

## Frozen phases

The initial runtime copy has 912 files (files.json and snapshot.json). The base commit alone is not the tested implementation: it includes the recorded uncommitted foundation and Doc overlay. workspace/ remains byte-identical after all tests.

The root integration worktree C:/projects/NEPL3-doc-html-export supplied these explicit subsequent overlays:

1. delta.json: tools/src/doc/export.rs and tools/tests/doc/export.rs, final shell-depth/queue repair. export source SHA 6f13526fa53ca59b0073d859d1bacded9ee3c4afd79b71eefe1d075232ac7a01. workspace-fixed/ contains initial source plus this delta.
2. identity-delta.json: host.rs test-only suffix and main.rs help text, copied into workspace-final/. The host's executable body is unchanged except trailing whitespace, but source::host_identity hashes the whole host.rs file, including its test suffix. Consequently the profile digest changes. Final CLI repeat results are explicitly from this final identity, not relabeled results from the earlier binary.
3. audit-delta.json: final browser.py lazy Playwright import, managed test_corpus.py and CI wiring. extract_cases AST is unchanged from the independently retested implementation.
4. spec-delta.json: final spec20 local development-host paragraph, read and checked against the implemented boundaries.

The final 913-file effective inventory is effective-files.json. source-verification.json verifies all initial and final SHA values. A comparison with the integration worktree found the same executable dependencies after the above overlays; older foundation/reader/engine test files remain different in our isolated baseline. This review does not claim to rerun root's whole-integration test suite. Rust/Cargo 1.97.0 and Wasmtime 44.0.1 logs are retained.

## Browser helper correction

The unchanged original raw Cargo log from the preceding HTML review now succeeds through the production helper, without removing the libtest prefix. original-browser-input.log preserves those exact bytes. The independent matrix verifies plain/BOM/raw agreement and rejects ten malformed cases: missing case, duplicate, arbitrary prefix, malformed test prefix, absent dots, invalid hex/UTF-8, extra field, unknown name, and missing payload. It does not broaden recognition to arbitrary marker substrings.

The final helper also passes the two new managed tests using plain Python without Playwright on the module path. Its real main() runs the same unmodified raw log and production CSS through Chromium and Firefox: 126 checks pass with document JavaScript disabled. browser-extraction-final.json, browser-raw-final-run.json/.log and browser-result-final.json retain commands/results. The old failure and its original code remain in review-doc-html-current; these are correction evidence, not replacement of the old failed run.

## Real source and filesystem path

Read tools/src/doc/source.rs, export.rs, main.rs and the tests directly. Compiled packages come from the existing checked Doc/Math/Circuit/Grammar bootstrap fixtures through the real Grammar compiler, registered schemas, effective profile, environment preparation and ParseSession. NativeHost supplies the existing actual reader; the standard owned fallback remains. Complete input requires that remaining bytes are ASCII space/tab/CR/LF only. Recoveries, stops and unsupported provider outcomes do not become a successful export. The source document is not parsed by an alternate implementation.

Local generation follows validated parse tree -> Doc lower -> prepare_local -> render -> shared typed markup validate -> shell depth check -> shared serialization. External Link, Asset and Foreign still return NeedsResolution. The fixed shell contains no user-generated markup strings; user text reaches it only through the checked serializer. The stylesheet is the backend's exact bytes. CSP permits same-origin stylesheets and disables default fetch, scripts and forms. No shell process, command interpolation, dynamic output paths from source content, or external fetch occurs in write().

write reads at most 10,000,001 bytes, rejects more than 10,000,000 and rejects invalid UTF-8. Generation completes before output directory creation. Existing output is refused; create_dir also provides the actual final exclusive creation check. Only fixed document.html, assets/doc.css and manifest.json child paths are used. The manifest is last. An I/O failure after directory creation can leave an incomplete directory; this is documented, not transactional rollback or crash-safe publication. The caller must handle that failure; this review does not claim filesystem durability or hostile concurrent directory-mutation resistance.

Independent actual CLI matrix (cli.py, cli-runs.json and per-case logs):

- Input path containing spaces, ampersand and literal square brackets succeeds. Two new output directories contain identical HTML, CSS and manifest bytes. Manifest source/file hashes, options and script-free declaration match actual bytes. CSS equals the compiled backend asset.
- Existing output, input-as-output and missing parent fail. Existing files/input hashes remain unchanged, and no missing parent is fabricated. Two concurrent processes targeting one new directory produce exactly one complete winner; the losing process does not overwrite its bytes (concurrent-write.json).
- Invalid UTF-8, over-cap input, malformed syntax, trailing token, unresolved label, external link, asset and foreign Code all fail before creating output. The three external requirement cases specifically retain NeedsResolution; they are not silently omitted.
- The actual 13,757-byte linear-combination document succeeds. A real HTTP server serves only the isolated output root at a non-root URL. Both JS-disabled Chromium/Firefox load its CSS, expose no scripts or broken local links, preserve equal full Article text hashes, and retain the expected sections/table and nonempty Ruby/Anno. Requests remain local. served-export-browser.json is our own execution, not a copy of root's result.

The initial managed export test passed native and WASI. After the shell repair, both managed export tests pass native and WASI. The CLI's filesystem matrix is native; it is not claimed as a WASI filesystem test. Actual generation through the managed public API is covered on both targets.

## Original shell-depth failure and repair

Original input is `article ja sentence cons ` followed by strong repeated 250, 251 or 252 times and `text "x" nil body nil`. The original real CLI accepted all three. An independent HTMLParser counted maximum non-whitespace node depths of 256, 257 and 258 after adding html/body. This violates spec20's explicit requirement to account for shell ancestors. Original files, successful exports and logs are retained.

The corrected implementation starts the validated fragment root at shell depth three. The exact original files are rerun: 250 succeeds with byte-identical HTML; 251 and 252 fail with OutputDepth before creating output. Fixed CLI logs include the unchanged input SHA. This is a real production API before/after reproduction, not merely a private helper test.

The first repair introduced an unmetered frontier: Work was charged at pop, after width-proportional child enqueue, and initial Vec growth charged two tuple slots while its first default push allocated capacity four. The exact helper was copied for a local bounded probe, not inserted into production. With Work1, width1 incurred Allocation64 while width10000 incurred Allocation320032 before WorkLimit. shell-delta-first.rs, shell-probe-before.rs, shell-queue-before.exe and its full output preserve the original implementation. This local probe does not claim that public generate was configured with one remaining Work unit.

The final exact helper uses pre-enqueue Work and charges the full requested new capacity before reserve_exact. The unchanged width1/10000 check now stops at Allocation16 on both targets. The first informational line in those unchanged probe logs demonstrates the old default-Vec baseline; it is not the final helper's allocation. A separate growth probe checks 1000 actual capacity transitions against cumulative charges, Allocation0/15/16, Work0 and cancellation preserving an existing frontier on native/WASI. All pass.

## Final identity and artifact check

The final CLI is built from workspace-final/ including the host source-identity suffix and new help text. Two fresh exports of the actual large document are byte-identical in all three files:

- document.html: 158cc157066c05a55cfa444a9e90a7bc12bf98054492f1df02fbfcb465f39ada
- assets/doc.css: 95e1239989b6be84d51d5cbf23a0c48f655d43392fc1e1e0e3791d5235d413d2
- manifest.json: 8c5eedd7e8cd4c6b78d2572c771ad684513f08461a97d3775919d56303b4791f

HTML/CSS are identical to the independently browser-tested bytes. Manifest usage changes when the new shell traversal is added, and profile identity changes with the host source suffix; neither is hidden by comparison against an outdated manifest. final-cli-runs.json records final profile identity and exact commands. All subprocess runs have explicit deadlines; actual CLI binaries and private probe native/WASI artifacts are retained.

## Remaining scope

This is a local development-host export, using checked compiler fixtures, separate parse/lower/output budgets and host-side file/manifest handling. It is not full PreparedArticle, resource existence/resolution, generated package distribution, a production suite CLI, general Artifact authority or all-language completion. Manifest JSON formatting and I/O are host work, not falsely included in the recorded semantic operation budgets. The review independently read the CI export/upload steps: artifact upload is not deployment. CI-wide root checks and targets not run here remain root evidence. No acceptance catalog status was changed by this reviewer.
