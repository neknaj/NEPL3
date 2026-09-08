# Independent authoring review: explicit page output budgets

Reviewed commit 6ac1b1aaf15d41f2ed764425c5f4a2ef09407621 directly against
the original spec21 at b84e9a3854dd23538154194054c28713714ba123 and the
same commit's authoring.md and AGENTS.md. No blocking content or authoring issue
found. The implementation's technical review is a separate task.

Only doc/migration/authored/21-doc-pages.nepld changes in 6ac1b1a. The original
specification is unchanged by this manuscript commit. The prior manuscript is
36,566 bytes and the final is 43,624 bytes. Removing the exact 7,058-byte addition
reconstructs the entire prior file byte for byte, including links, code, section
order and surrounding paragraph boundaries. Source and manuscript are UTF-8 LF.

All three new paragraphs were read directly, sentence by sentence (7 + 7 + 8).
The independently extracted base text equals the MD paragraph text after only
removing inline-code backticks and joining source line wrapping. No punctuation,
ASCII spaces, numbers, qualifications or exceptions are normalized away.

The first paragraph preserves all eight limits in exact order:
source_bytes, work, depth, nodes, allocation_units, output_bytes, diagnostics,
events. All eight are required together when supplied, nonnegative u64; null,
partial, unknown, negative and noninteger values reject. Omission retains the
development host defaults; zero is valid and stops when needed; there is no
unlimited sentinel or automatic increase. The setting is selected by the host
before execution, not by document text or provider, and does not alter
parse/lower, bare-metal, preview or ParseProfile limits.

The second paragraph preserves borrowing the caller Budget for the whole
resolve/render/serialize operation, checking an already stopped state at entry
(without excluding cancellation), retaining previously consumed Usage, and no
budget replacement or partial successful result after failure. All generation
precedes file output, while unfinished directories from I/O failure remain
distinct. Logical cumulative limits are not peak heap or wall-clock deadlines.
Package/Profile preparation, JSON/file containers, manifest construction and I/O
are explicitly outside shared output Usage.

The third paragraph preserves all parse/lower Limits and Usage and shared output
Limits/start/end Usage, plus the three-resource legacy output_usage display.
Execution identity is separate from semantic identity: UTF-8 domain
nepl3.local-doc-pages.execution/1, NUL, 32 semantic digest bytes, eight Limits
values in the declared order, then eight initial Usage values in that same order,
each u64 represented as eight big-endian bytes, followed by SHA-256. JSON object
iteration order has no authority. Different settings/start Usage produce a
different execution identity while content digests remain unchanged. A success
with a new setting does not revise prior default-budget failure evidence or
establish migration/Pages acceptance.

All six inline-code contents match the original exact bytes. They occur in six
explicit Sentence constructors; the remaining sixteen sentences use literals,
one natural sentence each. No unnecessary parallel, table, block, link, Anno or
decoration was introduced. The 132 Ruby occurrences (105 distinct pairs) were
read in context, including 非負/ひふ, 呼出/よびだ + し, 型付/かたつ + き,
全生成成功後/ぜんせいせいせいこうご and 壁時計/かべどけい. Bases contain only
the Kanji portions; kana, punctuation, numbers and Latin identifiers remain Text
or Code. Existing Anno and old manuscript content are unchanged.

The frozen repository's structure audit parses both the complete manuscript and
an independently wrapped addition. This is outer structural evidence, not a
production parser/lower/HTML run. No runtime, native/WASI, browser, canonical
cutover, human acceptance or full implementation success is claimed here.
Root separately reports the current 69,199-byte Reader succeeds with explicit
200M Work while default 100M still stops during serialization; this review does
not reexecute or reclassify either result.

check.py takes all source inputs from Git blobs and hashes the author's original
12 payloads, rather than relying on the self-check summary. Its initial generic
line-diff extraction aligned an identical `cons paragraph` at the end of the
insertion and therefore failed to parse the isolated wrapper, although the full
manuscript parsed. initial-check.py and initial-failure.log preserve that reviewer
helper failure. The corrected script rotates that exact unchanged line to the
start, asserts that removing the resulting addition reconstructs the old file,
and compares the resulting addition with the author's bytes. No source fix was
needed. Run `python check.py` to reproduce the successful review checks.
