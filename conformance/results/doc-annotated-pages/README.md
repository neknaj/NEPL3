# Explicit page-set annotated Markdown projection

Implementation checkpoint: `972dd0756dc01705c8f25666d32244e5205b30dd`.
This implements a typed host projection API using the production Doc PageSet
resolver. It does not add a context CLI, filesystem discovery, canonical registry
integration, or an HTML renderer for passive Markdown files. T21/T16 and the
required acceptance catalog remain incomplete.

The exact immutable set is revalidated, including source identities. Registered
relative/page links become relative Markdown destinations; passive files reject
fragments. Doc fragments require an actually emitted semantic anchor. The entire
set must render before any artifacts are returned. The same finite caller budget
covers resolution, projection and destination checking. The returned identity is
the input PageSet identity, not a hash of aliases or the final distribution.

## Validation

- All 86 native Doc integration tests passed, including five new page-set tests.
  The actual chapter-one draft reaches its explicitly supplied chapter-22
  Markdown destination. Missing targets, file fragments, duplicate routes,
  unsupported later pages, mutual/self links and sticky resource stops are tested.
- The five new tests passed on `wasm32-wasip2` using Wasmtime 44.0.1.
  The first invocation used a quoted Windows executable path that Cargo split
  incorrectly; the test process never executed. The failed log is retained.
  The corrected runner is `wasmtime run` from PATH.
- Workspace/all-target Clippy, formatting, diff and repository checks passed.
  The broad native test run precedes the two-line identity rustdoc clarification;
  the WASI rerun and repository check include it. Executable behavior is unchanged.
- Independent review exercised 19 public-API records on native and WASI with
  identical JSON results: seven route relationships, Unicode anchors and labels,
  first-receiver CBOR, modified plans, source conflicts, six resource dimensions,
  caller depth and cancellation. It also compared the actual chapter-thirteen
  single-Article output with separate old/new binaries; all bytes matched.
- The specification addition and its manually authored Doc counterpart received
  separate authoring review. Previous manuscript bytes are unchanged.

Initial test mistakes are recorded separately from production failures. A shared
codec retained admission state between different source fixtures and between the
reference and limited-budget runs. Independent operations now have fresh
admission, while a stopped operation retains its budget and codec on retry. A
one-variant Parallel was not valid Doc input; the unsupported-profile test now
uses two variants. The architecture assertion now checks text actually present
in the original source through an independent Markdown parser. Finally, route
collision checks use equal names: the typed Doc namespace is case-sensitive;
case-folding protection belongs to a filesystem host. The two initial five-test
logs are retained. The earlier three-test failures were observed interactively
and are not represented by a fabricated raw log.

`payloads.json` binds the source files and every payload to byte length and SHA-256.
Reviewer files are selected by their original manifests, excluding build caches
and executables. `.fixture` preserves diagnostic and source bytes, including
intentional whitespace; this does not relax production formatting checks.
Each row's `restore` path is the original owner-relative name. Any reserved
directory component in the archive filename is prefixed with `saved-` only for
storage. No payload is executed by this archive. Final exact-head CI remains a
separate merge gate.
