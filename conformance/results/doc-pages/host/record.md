## Final re-review: root filename correction

The earlier portability note below describes the initial snapshot. Root corrected the host boundary; filename-correction.json binds the exact pages.rs/test/main deltas independently imported. Host conflicts now reject case-differing shared segments before filesystem creation. Output segments with trailing dots or CON/PRN/AUX/NUL/COM1-9/LPT1-9 basenames (including extensions) are rejected. Core semantic registration remains case-sensitive; this is a host output profile.

Rebuild and all three managed export tests pass. Ten additional actual CLI probes (filename-cli-results.json) verify eight reserved/case/trailing-dot collision rejections with NO output directory, a valid shared-folder export, and existing-output rejection with unchanged sentinel. Windows filename convenience/case-collision issue is therefore corrected for the explicitly listed profile. Prior logs are preserved as historical pre-fix results, not current success/failure claims.

The first scratch probe accidentally named its fixture directory nul, which Python could not open on Windows; the harness names were corrected to reserved-* and rerun. This setup failure is not a production failure. No live implementation was changed by reviewer.

# Independent multi-page host export review

Frozen dirty snapshot: sources.json (1776 files); native Windows x86_64-pc-windows-msvc; isolated target directory. No live source edits by reviewer.
Scope: tools/src/doc/export/pages.rs, shared export shell, CLI routing, production host tests. Core page resolution and HTML page renderer have separate reviewers.

Result: no blocking host correctness finding in the inspected scope. Three managed export tests pass; the unmodified 13757-byte annotated Doc HTML production test also passes under existing operation budgets. Actual CLI matrix recorded in cli-results.json and individual cases/*/run.log.

Positive: nested mutual page links use ../b/index.html and ../a/index.html; each folder has its own assets/doc.css. Shared-folder CSS is deduplicated. Generated file SHA-256 values match the final manifest. The existing validated shell/CSP remains script-free and uses relative CSS URLs. Pages rendering completes before output creation.

Negative actual CLI cases reject: parent/absolute/backslash output routes, non-html routes, reserved manifest parent, source parent/absolute escape, missing source, invalid UTF-8, input over 10000000 bytes, combined input over 10000000 bytes, manifest over 65536 bytes, malformed JSON, unknown field, invalid version, zero/129 pages, trailing source, missing page, and unresolved external URI. These failures created no output directory. Existing output is rejected before work and its sentinel remains unchanged.

Output writing uses create_dir for a new root, then create_new files; manifest is written last. Case-only route collision A.html/a.html on Windows fails during create_new with os error 80. This leaves an explicitly incomplete directory with no manifest and does not overwrite the first file. This is a tested write-failure path, not a successful export.

Portability note: route/path validation is case-sensitive and accepts Windows device basenames such as CON.html. The CLI successfully created an actual CON.html file and manifest using Rust's filesystem path handling. This is not an observed traversal or overwrite vulnerability, but the generated filename set is not currently guaranteed convenient across all ordinary Windows filename consumers. A future portable-artifact filename profile could reject device basenames and case-folded collisions earlier. Root was informed; no extra root requirement was assumed.

The single route assets/doc.css/nested.html is valid because it emits its CSS at assets/doc.css/assets/doc.css, without a competing root assets/doc.css file. The existing managed test correctly rejects a real file/parent collision across two pages.

Source confinement: static inspection confirms local_path canonical containment before read. A real directory-junction escape is rejected with no output (junction.json). File-symlink creation itself was denied by host privilege (WinError1314), so that distinct setup/test is not claimed passed (symlink.json). No attempt was made to bypass permissions.

Limits and failure semantics: read_source uses take(MAX_SOURCE_BYTES+1) and UTF-8 decoding; manifest uses take(65537), deny_unknown_fields, version and page count checks. Parsing/lowering use separate bounded budgets per page; output resolve/render/serialize use their shared output budget. This does not claim one end-to-end document generation budget. Manifest JSON/file-map host allocations are separately bounded by the CLI input caps rather than all charged to the core output budget.

No complete standalone browser/CSS rendering, WASI filesystem host, Pages deployment, concurrent malicious filesystem replacement, or entire Doc migration acceptance is claimed. These results only establish the tested native host export slice. A manifest must be parsed and hashes checked; mere path existence is insufficient evidence after a partial write or external modification.
