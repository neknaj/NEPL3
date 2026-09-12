# Generic-code regression after annotated Math migration

PR103 head `0fd68387dec2bc2c60fc94ed49cb3d6d694c3e12` failed the existing
`corrected_foundation_and_math_documents_have_no_accidental_html_tags` test.
The Ubuntu job 103545435833 in run 34690738071 and the local reproduction are
retained. Its blanket no-HTML assertion rejected intentional Ruby/anchor HTML.
This failure was not merged or treated as successful CI.

The replacement checks the original regression directly: each generic type
remains exactly one complete inline-code value, and no bare T/Q/Span HTML
fragment occurs in the document. Removing each code span's backticks must lose
that code value and produce its specific generic HTML tag. The original small
malformed/fixed Markdown fixture remains unchanged. Production inventory
extraction and its reporting of raw HTML are unchanged.

Independent review required the baseline fragment rejection and exact tag
mapping; both corrections were implemented and re-reviewed. Root and reviewer
each ran all ten Markdown extraction tests successfully. Root also ran Clippy
for nepl3-tools/all-targets with warnings denied. Logs and review bytes are
length/hash-bound by payloads.json. Full final-head CI is still required before
integration; these targeted checks do not replace it.
