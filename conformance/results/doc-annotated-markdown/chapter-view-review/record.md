# Chapter 13 annotated Markdown content review

Independent reviewer: infrastructure. Read-only production/manuscript review; only this scratch evidence directory was changed. Fixed generation: 7b449ca2d1af3ed0ef135b8a0b9ca929e8a54569. Policy: Doc all-notes viewing projection, not Doc roundtrip or canonical cutover.

## Result

No content omission found in the supplied GitHub-rendered HTML against the actual authored Doc. All 37 substantive blocks match an independent typed-tree walk exactly: 8 headings and 29 paragraphs, heading depths h1/h2/h2/h3/h3/h2/h2/h2, all 33 InlineCode payloads, both external URI targets, and 674 Ruby readings at their original positions. Concat boundaries preserve visible order and spaces. The actual chapter has 147 Sentence nodes and 1,226 Concat nodes. It has no Anno, Strong, Emphasis, lists, tables, or RawCode: this review supplies no actual-chapter coverage for those constructors.

The full generated text was read, including URI/source identity, descriptor normalization and hash domains, package semantic versus execution identity, provider/owner boundaries, HTML/XML escaping examples, and limits/partial-result qualifications. Entity decoding in the independent HTML parser preserves code such as `&amp;`, `&#xD;`, and `]]>` as their literal code payloads.

The original Markdown and Doc base text match exactly across all 37 blocks, including whitespace and heading hierarchy. One pre-existing presentation difference is retained separately: the Doc author made `\u00xx` an InlineCode in the descriptor paragraph, while the canonical Markdown has those same bytes as ordinary text. Thus old Markdown has 32 code spans, and the draft/output have 33. This is an authoring-stage difference, not a renderer omission. Both original external URI targets and their ordered labels are preserved.

## Independent parser limitation

Mistune 3.3.2 was also run on the generated Markdown with `escape=False`. It parses all 37 blocks but treats the two RFC links in block 5 as plain Markdown syntax; the other 36 blocks match the Doc expectation. The generated destinations use backslash-escaped punctuation inside angle brackets. The supplied actual GitHub API result correctly produces both links. `mistune-mismatch.json` retains the observed mismatch; no parser-wide compatibility claim or expectation waiver is made. This difference is not classified here as a GitHub renderer loss or as a fully diagnosed Mistune bug.

The official [CommonMark escape rules](https://spec.commonmark.org/0.31.2/#backslash-escapes) and [Mistune API guide](https://mistune.lepture.com/en/latest/guide.html#abstract-syntax-tree) were consulted. The executed local Mistune version is 3.3.2; it is recorded rather than inferred from the documentation version.

## Evidence and execution scope

`check.py` uses the formal structure parser and a separately preserved literal decoder to obtain the authored tree, then implements its own all-notes walk and HTML event reader. It does not call the production Markdown writer to calculate expected output. Unknown actual tree kinds or HTML tags fail the check. The original source and all four compared artifacts are frozen and hashed in `manifest.json`; `result.json` and `check.log` record the actual independent assertions.

Root's generation receipt and GitHub request/receipt are copied with root provenance. The reviewer independently checked that the request's text equals the frozen generated Markdown and its mode/context are gfm/neknaj/NEPL3. Root's tool generation exit 0 is inspected evidence, not a reviewer rerun. The supplied HTML is tied to the root receipt hash. No network render or production Rust pipeline was rerun by this reviewer.

The old eight anchor mappings were independently reviewed elsewhere; this review confirms their presence in the supplied artifact only. It does not establish browser navigation, GitHub automatic-heading collision behavior, visual readability, accessibility, link destination reachability, semantic Doc roundtrip, or canonical migration acceptance. No browser was run.

The initial supplementary anchor assertion incorrectly expected the bare eight aliases to be the entire HTML name list. Its failure is retained in `anchor-check-initial.log`. Direct inspection shows GitHub prefixes names with `user-content-` and the writer also emits separate section anchors. The corrected assertion checks exactly one occurrence of each prefixed alias, without rejecting those additional intended section anchors. This was a reviewer assertion correction, not a production change or browser navigation result.

Final root rerun addendum: root reports final production 2e8fafa and records its command/binary in `root-final-result.json`. The reviewer directly compared `.tmp/chapter13/final.md` with the frozen `generated.md`: every byte matches (SHA-256 d986348f693b3225172bd23c5de9e86b03e8de1385b782898f22de380c542ec3). The content findings therefore apply unchanged to this final output. This comparison does not relabel the root rerun as independently executed.
