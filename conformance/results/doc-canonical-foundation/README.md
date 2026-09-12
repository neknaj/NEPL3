# Foundation specification canonical Doc evidence

Implementation checkpoint: `b8dfdd1160472e11d193f56ab2f4b5322ec0b873`.
Chapter 02 becomes the fourteenth canonical Doc page. The existing authored
source is moved without byte changes. Its preamble precedes the policy section,
and subsection 6.1 retains its nesting. Eighteen original code values remain;
the additional explicit `[start,end)` code span preserves the same original
FactDelta notation. No foundation runtime behavior is implemented by this move.

## Generation and finite host budgets

The first fourteen-page HTML generation stopped at AllocationLimit:
999,999,997 / 1,000,000,000. Increasing only allocation to 1,250,000,000 for a
new operation then exposed WorkLimit: 499,999,997 / 500,000,000, with nodes
19,497,622 and allocation 1,032,057,020. Neither failed run created an HTML
output directory. Both original failures are retained, not relabeled as success.

The resolved selection is HTML work 600,000,000, nodes 24,000,000, allocation
1,250,000,000. Node headroom is a finite allowance, not evidence of an observed
NodeLimit. Markdown budgets and core/per-page defaults are unchanged. These
are cumulative logical resource allowances, not physical heap measurements or
minimal resource proofs. Each retry starts a separate operation; stopped
budgets are never reset or resumed.

At e82157a, two production runs generated identical fifteen-file Markdown and
sixteen-file HTML bundles including manifests. The HTML recorded usage was
work 515,272,664, nodes 19,679,272, allocation 1,080,339,874. The prior thirteen
HTML pages and stylesheet remain identical. Five existing Markdown projections
change context metadata only. The adopted projections match the resolved run.

## Checks and independent review

At the implementation checkpoint, format, fourteen targeted canonical tests,
full fourteen-page projection verification, Clippy and repository check passed.
The expensive three-draft corpus remains a CI gate and was not included in the
local targeted command. The independent reviewer additionally ran ten Markdown
extraction tests after adoption, including the generic-code regression guard.

Independent display checks covered Chromium/Firefox/WebKit, two widths and two
non-root routes with JavaScript disabled, direct-file use without HTTP(S), the
preamble, nested subsection, nineteen code values and long annotation. Actual
GitHub checks verified eleven legacy destinations and ten semantic anchors.
Reviewers independently checked manifests, hashes, repeated output equality,
and unchanged previous pages. Human screen-reader and physical-device QA were
not performed. Final PR-head required CI remains a separate integration gate.

## Archive and compiler-free restoration

`payloads.json` binds the implementation source and raw fixture payloads to
lengths and SHA-256. Raw logs retain their original bytes. Restore reviewed
HTML into a fresh directory without compiling:

```sh
python conformance/results/doc-canonical-foundation/scripts/restore02.py.fixture conformance/results/doc-canonical-foundation restored-foundation
```

This restores sixteen files and verifies the manifest's fifteen HTML/CSS hashes.
Historical generation scripts retain their original worktree paths; restoration
needs only the archive and Python. This evidence does not complete T21, T16,
Pages deployment, or the formal runtime acceptance catalog.
