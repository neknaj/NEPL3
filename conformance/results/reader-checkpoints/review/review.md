# Reader native checkpoints: independent review

Fixed production under review: 92d7cd2e49f31b8ae0a3702f764e1e8b3c844bdf, parent73180760a888a193f88d9801ce86313d98f724d0. Git archive froze582 source files. No live production, specification or Git mutation by reviewer. Private copies/probes are under this directory. Root's checkpoint-notes.md is retained as separately attributed evidence, not an independent result.

## Representation and rollback invariant

Inspected all collector mutations in runtime/mod.rs, runtime/validate.rs and provider.rs, checkpoint.rs, and the complete changed portable TransformReplyContext and spec03 contract. Saved::Mark stores seven lengths plus cursor/state/overflow; state is budgeted cloned data, not a borrowed mutable reference. New Machine/Frame is private. Public ReaderFrame/ReaderCheckpoint/schema are unchanged.

The seven collectors are not globally append-only: roots are bundled, and transforms replace a suffix. The relevant invariant is that every active ancestor's saved prefix remains immutable. Child frames are popped before Node bundles its roots suffix or a completed Transform replaces its own view suffix. Node only split_offs its own starting root length and adds a new element/root; it does not edit earlier elements. append_view reindexes the incoming value, not prior elements. Transform truncates from its own starting elements/roots offset. Other collectors only append after successful preparation, or truncate on rollback. A failed Choice child and a failed Repeat iteration restore at their own entry; Repeat keeps the original prefix for min failure. Look/Not restore cursor/state and the seven suffixes. NeedMore unwinds all active frames. Failed/Stopped retain the accepted collector rather than rolling it back.

Owned continuations resume as Saved::Owned. They may restore complete values without requiring mark-prefix inequalities; newly pushed frames capture the then-current collector. This is necessary at a genuine owned transport boundary and does not accept externally supplied internal marks. External echo still checks the complete private continuation. Transform's portable validation still receives the exact saved checkpoint's view offset; changing its argument from whole frame to usize did not broaden authority.

Await materialization checks every saved length against current, allocates checked Vec capacity, copies each prefix with CopyCost, and copies state/phase. It retains Machine.current until all fallible preparation succeeds. The final outward continuation clone is precharged separately from constructing its owned representation, and the private slot is installed only after preparation succeeds. It does not publish pointer/length-only checkpoints as portable data.

## Independent production API tests

review_checkpoints.rs uses real TokenizationSession/ReaderSession, checked plans/schema/context, source admission and typed provider replies. review_helpers.rs reuses the fixed tests' schema/context/terminal fixture builders only; plans, inputs, expected semantics and mutation matrix are reviewer-authored.

- Input `ab`: prefix provider returns one view root, generated source/map, Capture, diagnostic/event and updated U64 state. Seven transactional bodies cover Choice, Look, Not, Repeat min failure, Map, Decode, and double Node. Each runs native synchronous callback and owned fallback under Matched/NeedMore/Stopped conditions:42 routes. Final complete results retain only the first prefix and exact original span0..1, state1, source/map/fact/diagnostic/event. NeedMore returns no rolled-back reports/sources. Cancelled retains previously accepted source/map/diagnostic/event and trace overflow. Complete/NeedMore/Stopped semantic outputs compare exactly between transports after normalizing only cumulative report Usage; transport costs are intentionally not asserted equal.
- Before the second owned provider reply, ten mutations alter each of seven checkpoint collectors, cursor=u64::MAX, state and overflow. Every forged echo is rejected as Continuation, then the original continuation resumes successfully. The test does not mistake forged IDs for actual huge allocations.
- Input `文` repeated17 times: Look+Node/Seq/Repeat verifies17 original UTF-8 byte spans and51 consumed bytes, under caller depth7. For Source/Work/Allocation/Nodes/Depth, six caps0/1/half/needed-1/needed/needed+1 produce complete exact results or the precise typed stop;20 stopped cases per target. Caller depth returns0. Successful native Usage is Source51/Work1600/Depth12/Nodes35/Allocation177145; wasm32 Allocation122077, other fields equal.
- Fixed managed package:58 tests native and WASI, including runtime38, builtins17, context1 and portable2. Existing Transform Map/Decode native/NDF/CBOR terminal/error/retry/resource regressions pass; existing owned prefix materialization and malformed-echo retry pass. This is actual managed portable coverage, not a newly invented generic ReaderContinuation encoder. No claim of fresh standalone continuation wire roundtrip beyond existing published codecs.

Builds use Rust/Cargo1.97.0 and Wasmtime44.0.1. run.py records exact argv/cwd/runner, full output, exit and600second process deadline. WASI is a real wasm32-wasip2 build/run. New checked_mul/isize capacity guards are inspected; arbitrary usize::MAX allocation is not claimed to have been executed. The wasm32 success/stop matrix and malformed MAX cursor exercise actual32bit behavior.

## Before/after allocation evidence

An isolated parent7318076 production archive receives only the new growing-view test module, with its original assertions unchanged. At128/256 visible Unicode nodes, native logical allocations are35,281,442/137,397,794 and WASI25,843,510/100,831,030. The unchanged3x doubling bound fails, as expected; retain nativeexit101 and WASIexit3/abort logs as failed tests, not successes. New92d7cd2 gives1,328,906/2,648,842 native and914,662/1,822,950 WASI, passing the same managed assertions. No production code in the baseline archive was instrumented.

## Additional Work accounting defect found before approval

92d7cd2 resume_saved converts owned frames by charging Vec slot and storage Allocation, then looping Frame::owned over the entire c.frames without per-frame or count-based Work. The array is real previously issued pending state, so this is a new width-dependent native conversion, even though its values are trusted.

review_frame_work.rs reproduces using public ReaderSession, Call wrapped in32 or128 Discard frames. After Await, it consumes only Work in the same permitted Budget to sweep remaining resume capacity. For adjacent caps587→588 and1835→1836, the complete frame storage and conversion precede the next drive WorkLimit. Native allocation jumps15,640/62,488 (=Vec slot24 + N×Frame488); WASI10,508/41,996 (=12 + N×328). The actual additional successful Work charge is1 in both cases. This is not a physical allocator/OOM claim: it is a logical precharge/processing-order contract violation. Root was asked to charge frame count before allocating/converting. Correction status is recorded in a separate addendum once received and independently rerun.

## Evidence limits

Initial independent probe compilation used incorrect public field names/ViewElement fields; original failed source/logs are preserved. Those were reviewer fixture errors, not product failures. A helper remains unused and yields a harmless warning; no warning suppression or production change was made. The initial unrestricted generated collector tests were expanded with explicit terminal/echo cases; final logs bind final source.

No limit increase, manuscript rewriting, full Doc workspace success, or large chapter completion is inferred. Root's large chapter Work/Output failures remain failures; this bounded runtime improvement does not solve all document throughput. No catalog status change is made solely to close a task.
