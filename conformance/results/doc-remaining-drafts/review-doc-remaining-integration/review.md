# Remaining reviewed Doc drafts: integration readiness

Fixed target: main `5a0b72f93dad54848f5616347bae59cf1be2a345`. All eight selected drafts are ready for an exact-byte draft-only import. No current canonical-source drift or new authoring correction was found. The eight destinations do not exist at this target, so importing the selected complete blobs creates eight additions and does not need text merging. This review performs no import, production edit, generated conversion or canonical cutover.

All paths below are relative to `doc/migration/authored/`; full commit IDs, Git blob IDs, SHA-256 and byte lengths are in result.json.

| Draft | Selected commit | Required source/review distinction |
| --- | --- | --- |
| 10-integration.nepld | e3a4de2b3ebc12834a47e51833a979da168f2275 | Complete selected blob; the commit itself is only an M/Ruby repair. |
| 11-conformance.nepld | ebac378083eff21051094685f148069ec0ccae68 | Complete selected blob; preserve prior exact InlineCode `]]>` formatting choice. |
| 12-model-invariants.nepld | 9dd5a5c14883bd375eca2813057e29e9835e9ac3 | Reviewed original is corrected 5657677, not this branch's older Markdown. |
| 13-reproducibility.nepld | 7e9e52ce53767a9c9a90650945dc091e6de65547 | Complete selected blob; preserve the additional exact backslash-u00xx InlineCode formatting. |
| 14-web-ui.nepld | d50f1688de44e70d027fa678e6d5b3bed3890196 | Complete selected blob; planned UI contracts remain planned. |
| guide/README.nepld | ca6081affbee132a217d0e0bd327a96845ff5e95 | Single-path addition; canonical input is doc/README.md. |
| guide/authoring.nepld | 98eb0c23fb10459d35c180bfe36746e25fcba571 | Single-path addition; retain the explicit Unicode same-page anchor. |
| guide/development.nepld | 3887fffc9fc08ae449a374bb97ff301db7786df1 | Final corrected blob, or cherry-pick 446000e then 3887fff in that order. |

The minimal uniform import method is to take each selected commit's blob at that exact path, not cherry-pick the five Ruby-only changes onto absent files. Those commits modify older manuscript files and would produce modify/delete conflicts when selected alone. README and authoring are single-file additions and can also be cherry-picked directly. Development's 3887fff is a modification, so a cherry-pick approach must first add its reviewed initial 446000e file. Importing the complete authoring branch would bring unrelated manuscripts and is unnecessary. Frozen `drafts/` files here are direct Git bytes, not regenerated text, and provide another exact source for the root's isolated import.

I read the six relevant independent review reports and traced their source/draft snapshots, rather than treating the author's drift table as approval. All six original manifests and their 131 payload entries were rehashed. Each selected Git manuscript and each current canonical Markdown blob appears byte-for-byte in those actual reviewed payloads. The author's prior drift table at 58e3e969 is saved only as input metadata; all eight equality checks were independently repeated against fixed main 5a0b72f.

The two source-history distinctions were directly reread against their authored passages:

- Chapter 12 includes the earlier two-paragraph correction covering HTML list/table/img, authority of design/markup.json and chapter 19, four typed href variants, SVG image prohibition versus HTML img, and lexical URI checks versus actual asset existence/content/authorization. Main's original exactly equals that corrected source, SHA 81b22608f80254f7d5979b754436b9d62da0bf25aefddf01a6fec772fdcf9cc5.
- Development's initial review explicitly held a stale blanket portable-status paragraph. The separate review-development-ci-delta covers 337057d Markdown and 3887fff manuscript. It retains foundation CI versus product-entrypoint completion, browser build versus execution, and release evidence conditions. Main exactly equals the corrected source, SHA f51f37998c2e606f3788ce892ef1f34f39b46e8a7533fbce5986087e6c634927. The initial guide review alone is insufficient provenance for the final development file; retain the supplemental manifest too.

Current doc/authoring.md and formal design/forms.json are byte-identical to the reviewed policy/forms. The previous full content reviews explicitly cover paragraph/Sentence preservation, contextual Kanji-only Ruby/okurigana, literal versus prefix choice, ordered code/link/table/list values, and no artificial Anno quota. I verified their fixed annotated draft bytes are still the selected bytes. These existing checks are reusable for unchanged text; this preparation review does not claim another independent full reading or execution of all eight long chapters. The authoring guide's Unicode heading required a specifically bounded exception to its historical ASCII-only structural auditor, as documented in the original review; that is not a production-parser proof.

All eight originals are byte-identical to their reviewed sources at this target, so no follow-up prose is needed before draft-only integration. Relative links and their historical Markdown targets remain preserved; they are not newly proved resolvable from the authored directory. Existing resource-limit results, production parsing, HTML export, page/anchor compatibility, separate human meaning review and canonical migration remain separate requirements. No runtime tests or browser checks were executed here.

For evidence integration, the six source manifests and their absolute original paths/hashes are enumerated in result.json. If archiving them, preserve each original manifest and its complete payload set, including its historical limits and (for review-b-ruby) its earlier chapter 05 scope. Do not rewrite these manifests to imply that every included historical case belongs to this eight-draft import. After import the root can verify exactly the eight selected blob IDs plus explicitly scoped evidence/local attributes, with all canonical/runtime files unchanged.
