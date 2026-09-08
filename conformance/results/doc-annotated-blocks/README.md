# Annotated Doc block projection

Implementation: `652d3b2476416a1ead7332f12055d1a18018cfb6`, based on
`a163764e93e2809f8013d7bbe16721082f979143`. This is bounded evidence for the
`nepl3-tools.markdown-annotated/2` viewing profile, not whole-group acceptance.
No task or acceptance status is promoted by this record.

The renderer reads the existing checked Doc arena. It adds flat lists with
explicit starts and checkbox states, plus tables with explicit headers and
column alignment. It preserves annotations, empty cells and literal code pipes.
Nested/multiple-block list items, adjacent lists, headerless/zero-column tables,
cell breaks and other unrepresentable inputs remain explicit failures. Doc's
semantic model and independent HTML backend retain their wider support.

## Executed checks

- The same real Doc list/table input failed with `Unsupported` on the prior
  implementation and succeeded with `/2`; the failed host invocation created
  no output file. The two executable hashes are in `payloads.json`; executable
  binaries are not part of this archive.
- Root: 81 Doc integration tests passed; 70 tools unit tests passed with one
  existing ignored test. Format, workspace Clippy, canonical projection check,
  canonical HTML generation, repository contracts and diff checks passed.
- Five new managed tests independently parse generated Markdown and check
  lists, tables, literal code/backslashes, structure rejection, caller usage,
  sticky resource stops and cancellation.
- Independent code reviewer: 248 actual Doc/GFM property cases and 17 boundary,
  fresh-CBOR receiver and budget cases passed on both native and WASI. Property
  output bytes agree; boundary results agree apart from target-dependent Usage.
  The managed five tests also passed independently on native.
- An independent reviewer compared the manually updated chapter twenty-one
  manuscript to the normative delta and authoring rules. Source ownership and
  authorship remain explicit in the separate author/reviewer records.
- Chapter thirteen's Doc source, Markdown body, generated HTML and CSS are
  unchanged. Its generated comment and registry identify renderer `/2`.

The native host is Windows, Rust 1.97.0. Exact tools, commands, input hashes,
stdout/stderr and reviewer limitations are retained. Repository checks report
runtime acceptance as not run; they do not execute the acceptance catalog.

## Corrections and limits

The first root check failed Clippy because new test assertions used disallowed
`expect` and `panic`. These were changed to typed test failures, and the final
checks passed without relaxing lint policy. Independent review identified the
need for an `isize::MAX` capacity guard before allocating escaped table code;
the final implementation contains that guard. Ordered markers now use a fixed
stack buffer. Neither is claimed as a physical huge-allocation test.

The independent probe's initial ordinary `[x]` text was accidentally written as
invalid Doc annotation syntax; its corrected explicit Text input and initial
failure remain recorded. An attempted WASI run before building the probe is
likewise a failed invocation, not a runtime pass. Read final run records for the
successful native/WASI executions.

The extra root chapter-zero output demonstrates generation only. Its legacy
anchor checks, old converter-fixture separation and canonical cutover belong to
the next change. No new all-browser Markdown guarantee, Pages deployment,
screen-reader verification or T21/T16 completion is claimed here.

## Payloads

`payloads.json` identifies each byte-preserved `.fixture` by owner, original
relative path, size and SHA-256, and binds the changed source files to the
implementation commit. Restore files under their original relative names to
inspect scripts or rerun the probes; fixed paths and tool installations in host
scripts must be adapted to the local environment. Empty stdout/stderr files are
preserved, not fabricated as nonempty acceptance logs.

Author, independent design/code review, manuscript review, first failed checks
and final root checks are separate owners. Their original manifests remain
inside their payload groups. CI of the final PR head is an additional merge
gate, not inferred from these local results.
