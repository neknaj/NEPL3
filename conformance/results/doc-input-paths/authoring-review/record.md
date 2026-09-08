# Independent Doc authoring review: physical input paths

Reviewed a6e304cd02ebee9f279c05a9e26b9ce617e727d8 against the original
spec21 at a604864a16bb6db33036d019b2ee4d8a99404b78 and the same-commit
authoring.md/AGENTS. No blocking content or authoring issue found. This is an
independent manuscript review, not a technical revalidation of filesystem code.

The sole change in a6e304c is authored21. All original Markdown bytes remain
unchanged. The 43,624-byte preceding draft is reconstructed exactly by removing
the new 4,364 bytes from the 47,988-byte draft. The two added paragraphs have six
and seven natural sentences; their base text matches the MD after only removing
inline-code delimiters and source wrapping. No words, punctuation, ASCII spaces,
examples, limits or qualification clauses were dropped.

I directly read the entire two-paragraph source and the corresponding 92 draft
lines. The first paragraph preserves optional physical input, relative to the
manifest directory, separately from logical source used for links. Omission and
explicit null select source as before. Explicit input requires nonempty text,
at most 4096 UTF-8 bytes, with all forbidden segment/character cases preserved.
Absolute paths and symlinks escaping the root remain prohibited. A missing
explicit input is not retried through source, and failure precedes output
directory creation. This is not confused with output_limits, whose null rule is
different in the preceding independently reviewed addition.

The second paragraph retains the exact example: source docs/intro.md, physical
input drafts/intro.nepld and link guide.md resolve against docs/guide.md. Physical
placement does not change link strings, extensions or logical lookup. A .md
logical name does not select a Markdown parser; the actual input remains Doc DSL.
Manifest provenance includes both selected input and logical source plus the
read bytes' digest. Changing only physical placement with identical Doc bytes
does not change PageSet semantic identity. This setting does not claim to resolve
unregistered pages, links to non-Doc files or assets.

All ten InlineCode values match exact original bytes, including the single
backslash in the forbidden-character example. Five explicit sentences contain
these code nodes; the other eight are sentence literals. The 69 Ruby occurrences
(56 distinct pairs) were read in context and checked for Kanji-only bases:
例/たと + えば, 読/よ + み替/か + えたり, 書/か + き換/か + えたり,
非空/ひくう, root外/がい and 読/よ + んだ are consistent. Kana, digits, ASCII
identifiers and punctuation remain outside Ruby. No Anno, table, links or other
structures were introduced unnecessarily, and all previous content is preserved.

The full draft and independently wrapped addition pass the frozen structure
helper. The supplied author manifest SHA256
9e105a02f276bd2f9dc23936dc8eec2fee1b2ca615e598a3b1f9a31f218f64da
and all 17 original payloads were rehashed, with source/draft/addition matched to
the independently extracted Git bytes. Author self-check is not the sole basis
of the conclusion; check.py and direct reading supply separate evidence.

The initial reused checker expected the line-diff ambiguity to align a paragraph
head at the opposite end. Here it aligned an identical closing nil instead. That
assertion failed before structural/text comparison. initial-check.py and
initial-failure.log preserve the reviewer-helper failure. The corrected check
moves only that identical boundary line, then asserts exact removal reconstructs
the previous file and matches the author's separately saved addition. No original
or draft change was required.

Production parse/lower/native/WASI/HTML, actual filesystem safety, complete
PageSet resolution, canonical migration and human acceptance were not executed
or inferred. The result covers the manuscript's faithful representation of the
new source contract and authoring requirements only.
