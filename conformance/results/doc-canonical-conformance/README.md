# Chapter 11 canonical Doc migration evidence

Reviewed implementation: `c0d4d126e9ac0a9f0872d2528344c743b70bc473`.
This checkpoint adopts chapter 11 as the twelfth canonical Doc page and fixes
the development tool's acceptance-definition reader for generated Markdown.
It does not complete T21, T16, Pages deployment, or any runtime acceptance group.

The authored document preserves all 57 required IDs, the nine S06 cases,
thirteen independent lists, and the historical r3 count. Two Ruby readings
were corrected. The redundant properties alias was removed because the
unchanged heading already supplies the legacy GitHub destination.

The acceptance reader now reads Markdown events rather than raw lines. Tests
cover escaped colons, duplicates, malformed prefixes, presentation containers,
and hidden inline HTML continuing across soft/hard breaks. Original failing
reproductions and the initially rejected implementation review are retained.

## Execution and limits

- The first Markdown generation stopped with NodeLimit and emitted no bundle.
  This failure is preserved under `node-stop`; no other exhausted resource is
  inferred from it.
- The twelve-page host selection uses Markdown work 1,000,000,000, nodes
  40,000,000, allocation 2,500,000,000; HTML work 500,000,000, nodes 20,000,000,
  allocation 1,000,000,000. Core and per-page defaults are unchanged. Additional
  work/allocation headroom is a finite host allowance, not proof that every
  previous limit failed or a measurement of physical heap requirements.
- `initial-generation` records the intermediate outputs before alias and
  acceptance-reader corrections. Only `generation`, `html`, and `markdown`
  are the resolved generation from b0072db439099fc6c57cb636ae199abd11828cd2.
  The subsequent inline-HTML reader fix does not alter rendering; the canonical
  check at the reviewed implementation regenerated all twelve pages successfully.
- Both resolved runs match byte-for-byte: thirteen Markdown bundle files and
  fourteen HTML bundle files, including manifests. Previous eleven HTML pages
  and stylesheet are unchanged. Three existing Markdown projections change
  only their context metadata.
- Format, Clippy, repository check, fourteen targeted canonical tests, thirteen
  task tests, and the full canonical projection check succeeded. The expensive
  three-draft corpus was not rerun locally; its existing CI gate remains enabled.
- Independent Chromium, Firefox and WebKit checks cover two widths and two
  non-root routes with JavaScript disabled, direct-file viewing, all required
  IDs, stable anchors, relative navigation and screenshots. Actual GitHub blob
  navigation checks cover all seven legacy destinations. WebKit direct-file
  checking uses HTTP(S) blocking rather than its problematic offline flag.

## Archive and restoration

`payloads.json` records source identities and the length and SHA-256 of every
raw `.fixture` payload. These files preserve original bytes, including logs;
they are not generated source or independently maintained documentation.
Reviewer reports under `review` and `display` define their inspected scope.

To restore the reviewed static HTML without compiling, run:

```sh
python conformance/results/doc-canonical-conformance/scripts/restore11.py.fixture conformance/results/doc-canonical-conformance restored-conformance
```

The destination must be fresh. This checks and restores fourteen files and
validates the manifest's thirteen HTML/CSS hashes. Root independently performed
this restoration before archiving. Historical generation scripts record the
original worktree paths; restoration requires only this archive and Python.

CI results must be checked against the final PR head separately. These local
and independent checks do not imply human screen-reader/device approval or
execution of the formal catalog's runtime acceptance groups.
