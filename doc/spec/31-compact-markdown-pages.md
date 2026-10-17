# Explicit compact Markdown footnote pages

The opt-in renderer `nepl3-tools.markdown-footnotes-compact-pages/1` selects
identity-only CompactV1 PageSet resolution. All pages in one manifest must use this profile;
mixing it with legacy identities is rejected. Legacy renderer names retain their
existing identities, semantics and output routing. HTML export rejects this
footnote-only profile; the explicit footnotes-pages command accepts it.

The profile inherits footnote path/alias and SVG rules. Styles/alias counts and
safe SVG input validation precede archive resolution. Math remains validated using
its selected Math/Sentence/Doc surfaces; a compact guest commitment is not a Math
lowering proof. Document bodies, footnotes, anchors, links and SVG bytes have the
same viewing semantics while document/set/guest identities deliberately differ.

Host compilation uses its unchanged standard surface setup; no archive descriptors
are required by this profile. Each render attempt creates fresh codec admission
and charges one cumulative output Budget. The identity-only capability validates
complete native documents and hashes private preimages without producing archive
transport buffers; it does not increase or reset any resource limit.

Frozen inputs retain the selected renderer string. Receipt input PageSet,
document and Math guest commitments use the explicit CompactV1 identities. Page
context includes the renderer selection, and retries use that same identity route.
No default manifest is automatically upgraded. Historical-corpus recovery requires
running that corpus with this profile under its original normal resource limits.
