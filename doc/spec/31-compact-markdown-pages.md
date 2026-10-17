# Explicit compact Markdown footnote pages

The opt-in renderer `nepl3-tools.markdown-footnotes-compact-pages/1` selects
archive PageSet resolution. All pages in one manifest must use this profile;
mixing it with legacy identities is rejected. Legacy renderer names retain their
existing identities, semantics and output routing. HTML export rejects this
footnote-only profile; the explicit footnotes-pages command accepts it.

The profile inherits footnote path/alias and SVG rules. Styles/alias counts and
safe SVG input validation precede archive resolution. Math remains validated using
its selected Math/Sentence/Doc surfaces; a compact guest commitment is not a Math
lowering proof. Document bodies, footnotes, anchors, links and SVG bytes have the
same viewing semantics while document/set/guest identities deliberately differ.

Host compilation registers the required source/group/Doc/PageSet archive contracts
only for the selected compact profile. This is bounded compiler setup, like source
surface compilation, not document admission. Ordinary generation and preparation
retry use the same setup. Each render attempt still creates fresh codec admission
and charges one cumulative output Budget; no budget limit is raised or reset.

Frozen inputs retain the selected renderer string. Receipt input PageSet,
document and Math guest commitments use the explicit archive identities. Page
context includes the renderer selection, and retries use that same identity route.
No default manifest is automatically upgraded. Historical-corpus recovery requires
running that corpus with this profile under its original normal resource limits.
