# Author self-check: root CODEX guide

Source: b83716cd14fe7d6d57579e9d6a9ec4ad2877dba2 CODEX.md. The original
Markdown, AGENTS and authoring guide are frozen here. Only the assigned new
doc/migration/authored/project/CODEX.nepld was authored; no original source,
other author's file, production code or Git state was changed.

The manuscript was written manually after reading each source paragraph. The
script checks the result and does not generate or modify Doc text. All five
paragraphs retain their respective 1/4/4/3/7 sentences, including the historical
design version, T01/T16 not-complete qualifier, the explicit Doc authoring
exception, ordinary root implementation, and dependence on all registered final
acceptance evidence. No latest execution or completed status is substituted.

The body has thirteen normal sentence literals and six explicit sentences for
the three inline-code values and five links (several links share one sentence).
The title is also a literal. Every Kanji portion was given a contextual reading;
送り仮名, Latin identifiers, numbers and punctuation remain outside Ruby.
There were no source Anno, translation, table or block-code structures to retain
and none was added solely for decoration. Explicit escaped slash in literals
preserves the original slash bytes and is accepted by the existing Sentence
parser's escape match (sentence/parser.rs).

Relative link target bytes are the original CODEX.md namespace, unchanged:
AGENTS.md, doc/README.md, doc/development.md, implementation-status.json and
doc/spec/11-conformance.md. They are not retargeted to physical locations beside
this candidate. A future PageSet must bind the logical source location and
resolve destinations; this candidate alone is not a successful page export.

The self-check parses the full authored Article with the frozen structure
helper, compares paragraph base text against the Markdown after removing only
markup delimiters and source line wraps, and checks exact code/link strings.
It checks one natural sentence per unit and Kanji-only Ruby bases, then records
readings for review. Full text and readings were read again after this check.
This is author/structure evidence only. Independent review, actual production
parse/lower/HTML, link resolution and canonical migration remain separate.

The first checker invocation hit two checker-only mistakes: an unescaped Japanese
punctuation literal was changed by the shell-to-Python pipe, and a guessed helper
enum name Relative differed from the actual form RelativeTarget. The checker
now spells the punctuation as an ASCII Unicode escape and uses the exact frozen
form name. Neither failure required changing manuscript/source content.
