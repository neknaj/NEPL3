# Explicit canonical HTML output allowance

Production checkpoint: `4de46baf5f22f8dcb5e99d0312d9b61e36a44d6b`.
Development manuscript/test follow-up: `f071d09dd890423c8f5dc134bc0eeae38a0ccba1`.
Main integration: `49df9c709526cb1046a6f31d0c300f7c4b9274e7`.

Canonical registries may select `html_output_limits` using the existing eight
unsigned resource fields. Omission preserves the previous HTML defaults. One
finite budget selected before entry is passed to the existing PageSet HTML
generator; no new renderer, global limit increase, stopped-operation retry or
budget fallback is introduced. Markdown uses its distinct `output_limits`, while
parsing and lowering retain separate per-page limits. The raw registry bytes
still affect the new Markdown profile's context identity, including HTML settings.

## Execution and independent review

- All fifteen canonical native tests passed at fixed clean `4de46ba`, together
  with formatting, workspace Clippy, current canonical generation and repository
  checks. The tests exercise omitted/explicit defaults, byte equality, independent
  settings, changed execution identity with unchanged content, six stops before
  staging, and strict malformed values.
- At fixed clean `f071d09`, the expanded production draft test parsed, lowered
  and checked labels for complete chapters16/21 and the development guide.
  The finite per-phase Work600M / Allocation1.5B were selected before entry;
  observed parse Work was73,988,983 /419,222,830 /254,321,387 respectively.
  These are logical counters, not physical heap measurements. Clippy and format
  passed. Production code and chapter16 text are unchanged from `4de46ba`.
- After main's architecture cutover was integrated, fourteen canonical tests,
  all four current canonical projections, Clippy, repository and formatting
  checks passed at fixed clean `49df9c7`. The already verified full three-draft
  test was explicitly excluded from this integration rerun; its inputs did not
  change. Each batch records its fixed commit and unchanged clean tree checks.
- Independent native probes ran one before and four after tests. All three
  implicit-default HTML/CSS/manifest files equal the previous implementation's
  bytes and explicitly selected defaults. All eight resource fields were also
  set to distinct finite values; the reviewer independently reconstructed output
  and phase execution identities with fixed-order big-endian u64 records.
  Parsing/lowering limits, initial and observed usage, source/profile records
  and content bytes remain independent of the HTML output choice.
- Six zero-resource stops, thirteen malformed typed/raw configurations and two
  late source/route errors fail without staging. A stopped Markdown setting does
  not stop HTML and vice versa; changed raw settings appropriately change the new
  Markdown context while retaining its body. Separate manuscript review verified
  the eight specification sentences and six guide sentences, code values,
  Kanji-only Ruby and unchanged prior manuscript bytes.

The motivating five-page HTML WorkLimit log is retained as an initial observation.
That original command record has no executable hash or fixed source commit; it
is not described as a controlled before/after benchmark. Subsequent real five-page
generation belongs to the chapter18 cutover evidence. The reviewer also retains
its initial helper failures separately from successful exact replay.

This validates the Windows native host allowance contract. Independent WASI,
browser rendering, whole migration acceptance and live Pages deployment are
separate scopes. Required statuses are unchanged; final CI is a separate merge
gate. The resource option is not a claim that source-processing costs are optimal.

`payloads.json` binds source files and original restore paths to byte lengths and
SHA-256. Reviewer files are selected only by their strict manifests. `.fixture`
preserves exact bytes; reserved path components become `saved-*` in storage.
Restore using owner/restore fields before replay and follow each review record's
fixed source and probe instructions. Caches, workspaces and binaries are excluded.
