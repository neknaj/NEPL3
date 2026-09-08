# Doc Ruby layout regression

This records the static HTML fix developed from `b29a420480b94d34876fe39a7451a446aadb240d`
and implemented in `aecfbdc91700e1c291b79d70aa3fde8191752b54` and
`58f84716997b345dcda39746d3a6708a33b5ec41`. It does not complete a runtime
acceptance group or switch a document's canonical source.

The previous grid stylesheet requires `baseline-source`, which the tested WebKit
does not implement. A caption/table fallback preserves the same typed spans and
Ruby last-line / Anno first-line baselines. Explicit `white-space:pre` preserves
the existing atomic max-content contract; otherwise long readings can wrap inside
table captions even on a wide page. Explicit Doc Breaks remain effective.

The main agent implemented the change. Huygens independently inspected the source,
executed the production-output corpus and rejected an initial runner false positive:
a missing line-reservation marker could skip that check. The repaired runner requires
the markers, explicit breaks and distinct multiline positions. Carson manually
updated the three affected Doc drafts; Huygens reviewed those against the Markdown
source and authoring policy.

- Final production output: 13 Doc inputs × 2 widths × 3 font sizes × 3 line heights
  × Chromium/Firefox/WebKit = 702 passing measurements with document JS disabled.
- Old grid CSS: the root's expanded harness rejects 162 WebKit measurements.
- Independent review: 702 final measurements, 39 mutation/control cases and four
  Python tests passed. Original failures and intermediate results remain separate.
- Native Doc tests: 76 passed. Workspace Clippy and pre-archive repository check passed.
- Chapter 13 was exported with production APIs and viewed over a nonroot HTTP URL
  in all three engines at 375/1280 widths. Source, HTML, CSS, manifest and screenshots
  are retained. This is not a canonical-cutover or Pages deployment record.

`payloads.json` binds exact byte lengths, SHA-256 values and restoration paths.
Payloads have `.fixture` appended so historical sources, malformed probe inputs and
their Markdown links are not interpreted as current repository contracts. Restore
them to the listed `restore` paths when replaying a recorded script. Original
CRLF/BOM and failed executions are retained; they are not normalized into success.
Each owner manifest binds its own selected inputs and outputs. The root receipt
identifies its earlier negative-runner version separately from the final version.

These are Windows-host Playwright 1.62.0 measurements (Chromium 151.0.7922.34,
Firefox 153.0, WebKit 26.5). CI independently runs matching pinned Playwright
engines on Linux. Automated accessibility snapshots are not human screen-reader,
Apple Safari device, print, Wasm, Playground or full document migration acceptance.
