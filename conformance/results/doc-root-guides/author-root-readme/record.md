# Root README manual Doc draft

Source commit: b83716cd14fe7d6d57579e9d6a9ec4ad2877dba2.
Source: README.md, SHA256 6f73257c8fa7995ec11328aedc6b487fc6f11f9f6e6299b0c17af9cd2d446176.
Draft: doc/migration/authored/project/README.nepld, SHA256 6ba8e901529f97efb7da3ff80d69c28df37d4cd83a2efe8a346a195d94426f31.

Only the assigned draft was authored. Source README, production, schema, registry, assets and the other agent's CODEX manuscript were not edited. No commit/push was made, as requested for the shared root worktree.

Read the fixed source, AGENTS, authoring.md and Doc image/asset/link/table signatures. Manually retained the title, seven paragraphs (18 sentences including the badge-only sentence), one two-column table with header plus five rows, all 15 link targets/labels/order, one Strong span, three InlineCode values and the 149 UTF-8 byte sh RawCode block including its final LF. Ordinary narrative/cells use sentence literals; code, links, Strong and the image use explicit sentence/inline constructors. Japanese readings attach only to Kanji bases; no new translation was invented. The source's current local HTML/draft work remains distinct from canonical cutover and Web publication.

The CI badge uses the existing typed nesting:

    link external <workflow URL>
      image asset <original badge.svg URL> none "CI"

The image URL is preserved exactly as an unresolved AssetRef id, the surrounding target is the original workflow URL, and the alt is exactly CI. Spec05 DG05 allows an AssetRef Text id with optional digest; asset reading/digest/MIME/display preparation is a separate operation. No digest or asset bytes were invented, no network fetch occurred, no raw HTML or new constructor was introduced. This source preserves the image requirement; it is not a resolved/renderable asset or a claim that a particular CI status is current. Host asset mapping, actual badge resource/digest policy, no-network export behavior and HTML rendering remain outstanding. A live status badge cannot be silently represented by a fixed pass label or discarded to make rendering succeed.

Self-check: the existing formal-form structural parser accepts the complete Article; the scratch checker expands literal annotations for inspection, checks all ordinary Kanji are Ruby-covered, confirms 99 Kanji-only Ruby bases/readings, compares paragraph base text exactly to Markdown, compares table cell order and content, link labels and targets, Strong span and raw/inline code bytes. UTF-8 LF and the original source hash are verified. An initial hand draft placed the suffix レビュー outside one link label; this was corrected to Concat inside that same Link before the successful exact link-label check. No source semantics were changed.

This is author self-check, not independent content review, production parser/lower/HTML, asset resolution, route/anchor publication or canonical migration acceptance. The scratch checker is not a new production tool. Original source/context, draft, normalized paragraph text, readings, result and hashes are retained here for the independent reviewer.
