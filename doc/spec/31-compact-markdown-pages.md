<!-- Generated from doc/spec/31&#45;compact&#45;markdown&#45;pages.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page compact&#45;markdown&#45;pages; source SHA-256 6f8388941772934dfa2a1ff1ca6dcfd7a983e76fd81dd1cea39e43b9aa921032; alias input SHA-256 138d52aec4aab90d16d091058fb6e09a354e86a79de37785c5468dfce6ca2c2a; page input SHA-256 7700117ea5c7bf1db9cbe39c594a482363bd67a427e97ca680a9b43964f2a589. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="explicit-compact-markdown-footnote-pages"></a>

# Explicit compact Markdown footnote pages

[正本（NEPL3d）](<31-compact-markdown-pages.nepld>)

The opt\-in renderer `nepl3-tools.markdown-footnotes-compact-pages/1` selects identity\-only CompactV1 PageSet resolution\. All pages in one manifest must use this profile\; mixing it with legacy identities is rejected\. Legacy renderer names retain their existing identities\, semantics and output routing\. HTML export rejects this footnote\-only profile\; the explicit footnotes\-pages command accepts it\.

The profile inherits footnote path\/alias and SVG rules\. Styles\/alias counts and safe SVG input validation precede archive resolution\. Math remains validated using its selected Math\/Sentence\/Doc surfaces\; a compact guest commitment is not a Math lowering proof\. Document bodies\, footnotes\, anchors\, links and SVG bytes have the same viewing semantics while document\/set\/guest identities deliberately differ\.

Host compilation uses its unchanged standard surface setup\; no archive descriptors are required by this profile\. Each render attempt creates fresh codec admission and charges one cumulative output Budget\. The identity\-only capability validates complete native documents and hashes private preimages without producing archive transport buffers\; it does not increase or reset any resource limit\.

Frozen inputs retain the selected renderer string\. Receipt input PageSet\, document and Math guest commitments use the explicit CompactV1 identities\. Page context includes the renderer selection\, and retries use that same identity route\. No default manifest is automatically upgraded\. Historical\-corpus recovery requires running that corpus with this profile under its original normal resource limits\.
