# External language consumer checkpoint

This archive records a bounded public-API extension experiment and independent
reviews. It is not X01/X02 acceptance evidence or foundation extraction approval.
`payloads.json` hashes the preserved bytes. `fixture-paths.json` maps original
names to opaque `.fixture` files where BOM, line endings or deliberately invalid
JSON need preserving. Restore those names when replaying an archived probe.

- Architecture review: fixed foundation source at `8808c49`, independently
  checked unchanged through `7823098`; external native/WASI API probes, existing
  provider tests moved outside the workspace, and ARMv6-M compilation. Reused
  production test expectations are identified separately from independent cases.
- Consumer review: initial runner faults, corrected runner execution at
  `6507b14`, and independent native/WASI cases including a fresh receiving source
  store. `archive-scope.json` identifies excluded build caches from an initial
  recursive snapshot. Caches are not test inputs or release artifacts.
- Root execution: three consumer tests and six orchestration regressions;
  separate real external-directory runs. Orchestration fault injection does not
  execute Rust. The final fresh-receiver source hash is in
  `verification/consumer-fresh.json`.
- Initial CI at `75e73a7` failed when Cargo download progress on stderr was
  combined with JSON stdout. Its original artifact and job log are retained;
  the later cancellation of superseded runs does not turn that failure into a
  pass. `6507b14` separates and hashes both streams; its CI receipts are included.
- Chapter author/review: new chapter 22, exact prose/table/conditions/code and
  Ruby review. Delta author/review: only additions to existing guide/01/11 drafts.
- `execution/chapter22-verified` is actual checked Doc HTML and CSS. The root
  execution receipt hashes its source, specification and executable before and
  after generation. No real-browser validation is claimed here. The other three
  drafts reached link resolution requirements, not completed HTML.

The foundation remains unchanged. The consumer has its own workspace and
lockfile, but depends on foundation paths inside the monorepo. Independent
distribution, package/Profile loading, general process provider operation,
independent Rust/schema compatibility checks, Doc canonical cutover and the full
product remain unfinished. All X01/X02 acceptance states remain `not-run`.
