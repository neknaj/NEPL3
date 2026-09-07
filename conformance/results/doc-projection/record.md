# Restricted Markdown compatibility view

Base: `2ddddd05e9334b83c9f654e3a1042555de5aefd4`, feat/doc-page-links.
Root implemented the typed Doc-to-Markdown writer and bounded CLI; separate
agents reviewed the writer and host boundary from fixed source snapshots.

The new path parses real Doc source, lowers it with the existing production
pipeline and calls Doc preparation checks before projecting supported typed
nodes. It is not a source-string replacement or a general Doc roundtrip.
Canonical Markdown remains unchanged. The first migration candidate's generated
view matches its original Markdown's text and block/code event sequence.

Root: 4 native and 4 WASI production-path tests passed, including punctuation,
code-delimiter/space preservation, unsupported shapes, sticky resource limits
and cancellation. Format, Clippy, candidate generator/drift and Python rejection
tests passed. Commands and raw logs are under root/.

Independent writer review: native 8 and WASI 8 passed (4 managed + 4 independent).
Three actual failures were reproduced and then corrected: adjacent Code nodes
merged into one Markdown span, lists across a Section boundary merged, and
final Markdown bytes were not charged to OutputBytes. The writer now refuses
ambiguous structures and charges every emitted byte before allocation/output.
Original and intermediate failures remain recorded separately from final passes.

Independent CLI review: 16 cases plus input/output-same-path protection.
Unicode/CRLF source digest, renderer/source comment, delimiter escaping,
UTF-8/size/path limits and create-new-only behavior were checked. Unsupported
input does not create a partial output file; later filesystem write errors may
still leave an incomplete file and are not reported as success.

CI builds the view separately from the HTML page-set directory and uploads it
as a separate artifact. The preceding page-set commit's CI passed at run
34160389457. Its downloaded five-file HTML/CSS/manifest artifact is byte-identical
to the local Windows export; comparison is under doc-pages/ci-parent/.
Remote CI for this new projection commit is not inferred from that earlier run.

Scope exclusions: general annotation/parallel/table/link/code-block projection,
legacy anchor aliases, canonical registry switching, human meaning approval,
Pages deployment/recovery and T21/T16 completion. These results do not change
required acceptance states to passed.
