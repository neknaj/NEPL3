# Math specification canonical Doc evidence

Implementation checkpoint: `cd883dea39d47deacffa928e67933ff5a8707a27`.
Chapter 06 is the thirteenth canonical Doc page. Its source is the previously
authored draft with the preferred reading of 空文字 changed to からもじ. The
semantic contract is unchanged; this is not implementation of Math evaluation,
MathML, KaTeX, or a claim that runtime acceptance has passed.

The chapter retains its thirteen operation-list items, twelve inline-code
values, eight Sections and the link to the canonical chapter 17. Seven explicit
aliases preserve annotated legacy headings; two unchanged automatic GitHub
headings retain their own IDs without redundant aliases. All nine legacy
destinations and eight semantic anchors were independently checked on the
actual adopted GitHub blob.

## Validation

Two full production generations at c2cdfa7cebb5a6e08d083f7ba5ca54b69ce3cca6
produced identical fourteen-file Markdown and fifteen-file HTML bundles,
including manifests. No budgets changed for this migration. The previous
twelve HTML pages and CSS match exactly. Four existing Markdown projections
change only context metadata; their bodies are unchanged.

At the implementation checkpoint, format, fourteen targeted canonical tests,
the full thirteen-page canonical projection check, Clippy and repository check
passed. The existing expensive three-draft corpus was excluded from this local
targeted test command; its CI requirement remains enabled. Full final-head CI
is a separate integration gate, not implied by these local results.

Independent browser checks covered Chromium/Firefox/WebKit at 375 and 1280 px
under two non-root routes with JavaScript disabled, old and semantic anchors,
chapter 17 navigation/reload, direct-file viewing without HTTP(S), exact list
and code content, and visual screenshots. Human screen-reader use and physical
Safari/device QA were not performed. Independent source/cutover review also
recomputed output hashes, manifests, and adoption equality.

## Archive

`payloads.json` binds source files and all raw `.fixture` payloads to byte
lengths and SHA-256. `review`, `preflight` and `display` contain independently
recorded review scopes. Raw payloads preserve their original bytes.

Restore the static HTML without compiling into a fresh directory:

```sh
python conformance/results/doc-canonical-math/scripts/restore06.py.fixture conformance/results/doc-canonical-math restored-math
```

The script checks and restores fifteen files and validates fourteen HTML/CSS
hashes from the manifest. Root performed this compiler-free restoration before
archiving. Historical generation scripts retain their original worktree paths;
restoration depends only on this archive and Python. Evidence does not complete
T21, T16, Pages deployment, or the required runtime acceptance catalog.
