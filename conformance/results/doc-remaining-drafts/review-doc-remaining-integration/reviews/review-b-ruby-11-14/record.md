# Independent chapter 11/12 Ruby repair review

Fixed manuscript revisions: 11 at ebac378083eff21051094685f148069ec0ccae68 and 12 at 9dd5a5c14883bd375eca2813057e29e9835e9ac3. Each is compared with its parent manuscript and the original Markdown. No production file or manuscript was edited.

No additional blocking finding was identified. Both complete before/after structures match under the review projection that replaces Ruby with its base and normalizes Concat/adjacent Text, retaining every Sentence boundary and all other constructors, fields and order. This proves preservation of underlying prose and non-Ruby structure, not identity of the complete annotated Doc meaning.

## Source revision distinction

Chapter 12's authored text already includes an authorized earlier correction from 5657677e9b4a1cc44830b9a365206822920983fe, integrated into the draft before this Ruby repair. The same-branch Markdown is older. Both Markdown versions are fixed in this archive. The corrected source SHA is 81b22608f80254f7d5979b754436b9d62da0bf25aefddf01a6fec772fdcf9cc5. Direct Git diff and source reading confirm the two changed HTML safety paragraphs: table/list/img, the design/markup.json and chapter 19 authority, typed href variants, and the distinction between URI syntax and asset existence/content/authorization. The authored text retains those actual corrected conditions; they are not a new change introduced by the Ruby repair.

Chapter 11 has one preexisting InlineCode addition relative to Markdown formatting: literal bytes ]]> in D05, originally ordinary Markdown prose. The other 24 InlineCode values match the original Markdown sequence exactly. The whole before/after comparison preserves this existing formatting choice and all bytes.

## Checks and direct reading

- Chapter 11: 155 Sentences, 6 Sections, 80 Paragraphs, 64 list items, 25 InlineCode nodes, one link, no RawCode or table. All 55 required groups and the nine S06 failure cases remain; task-scoped evidence versus group acceptance, runner requirements, dynamic required-group counting, typed evidence fields, identity/log verification, and non-success treatment of unrun cases were directly read against the original Markdown. No lost or reversed requirement was found.
- Chapter 12: 184 Sentences, 7 Sections, 32 Paragraphs, 9 InlineCode nodes, one link, and the 18 table rows (header plus 17 constraints) match the corrected Markdown. Ownership of references/environments/ForeignClosure, typed nested stop causes, byte-position map cycles, token/view/source ownership, private Checked proofs, Circuit IR arities/width/bit order, and the complete Markup restrictions were directly compared.
- All 737/701 Ruby occurrences were structurally checked for Kanji-only bases (including iteration marks), nonempty readings, and absence of unannotated Kanji in ordinary narrative Text. All 435/368 distinct base/reading pairs were read, including their context and okurigana boundaries. No concrete correction was required. Code is exempt from prose annotation and remains unchanged. No invented Anno quota was imposed.

The independent checker uses the fixed formal structure audit for constructor/category parsing and its own sentence-literal expansion and comparison. Original code sequences, link targets/order, and table cells/order are checked separately. Its first exact-code check exposed the two source/formatting distinctions described above; the checker now records those explicit proven distinctions rather than silently removing arbitrary differences.

## Limits

This is independent content and structural review. Production parser/lower/HTML execution, native/WASI, source position identity, runtime resource admission, heading compatibility, and the required separate human review/canonical migration are not established here. Root coordinates canonical status; these manuscripts remain drafts. Chapters 13 and 14 are not covered by this manifest and will be reviewed after their revised commits arrive.
