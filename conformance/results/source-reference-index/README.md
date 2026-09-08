# Indexed snapshot reference resolution

Production checkpoint cf233ff uses the existing immutable source/revision index for get_ref/get/resolve, and still checks the full digest. Insertion order, source/URI distinctions, atomic edits, portable identities and the existing budgeted lookup contract are unchanged. No cache or unchecked validation proof is added.

Independent native/WASI tests and ARMv6-M compilation cover production APIs and additional identity/edit failure probes. Original manifests and payload bytes are preserved; excluded setup failures are retained as such. Root workspace tests passed 531 with one ignored; format, Clippy and repository checks also passed.

The unchanged authored review guide is compared using the same finite 2B Work allowance at baseline 76818219 and checkpoint cf233ff. Both stop at the same cursor with exactly the same diagnostic and Usage and produce no artifact. Elapsed times are observations, not a timing guarantee or a claim that the long-document limit is fixed. The separate unchanged authored chapter 13 comparison uses default finite limits and requires byte-identical generated artifacts. It does not establish canonical migration, browser accessibility, Pages deployment or whole-project acceptance.

Any BOM-containing original payload is stored under its mapped .fixture name without altering bytes. Restore the original names in scratch space when replaying those helpers.
