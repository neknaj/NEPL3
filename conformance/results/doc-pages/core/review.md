# Independent Doc PageSet review

Result: no blocking discrepancy found in the tested semantic PageSet scope. This is a registration/link-resolution plan, not full PreparedArticle, asset existence, rendering, HTML placement, filesystem authority, or site deployment proof.

## Fixed inputs and reproducibility

The 1,518-file workspace was copied before subsequent HTML page-set implementation. `snapshot.json` records base 30201c4ff71b05806828c186074754309b0be304. `files.json` hashes every dependency and production source; `source-verification.json` confirms all remained identical after tests. All builds used this workspace and separate review target directories. Versions, full commands, deadlines, exit codes and full logs accompany this record. The generator was run without --write and passed.

Managed tests: nepl3-doc-core pages 5 and tools doc pages 1 passed on both native and wasm32-wasip2. These are bounded test counts, not a workspace/task completion claim.

## Independent execution

`probe/src/main.rs` contains three actual source documents, including Japanese Ruby/Anno titles. Each passes the existing real compiled Grammar/parser/lower path in both owned and native mode, with equal DocumentSyntax within each original input. Primary source IDs p, p:0 and p:0:1 exercise the compact length/id/revision reservation namespace. Every resulting source ID/revision is distinct across the documents. Two internal links resolve exactly to the opposite page's same-named label; the third page retains one each Foreign, Asset and External Link requirement.

The plan identity is independently calculated as SHA256 of the prescribed NEPL3.Doc.Pages.v1 NUL domain plus canonical PageSet CBOR. Actual initial CBOR receivers retain the same plan and admit all primary/generated source bytes exactly once. Eight schema-valid plan modifications (digest, omitted links, omitted remaining requirements, link order, page/node/target indices and fragment), three changed input sets (route, registration order and text), conflicting URI for an existing source identity, and omitted source declarations are rejected. A fresh SourceAdmission with SourceBytes=0 stops with SourceLimit and retains that sticky reason. The source input remains unchanged.

Canonical source table order can change across wire reception; raw sender/receiver DocumentSyntax vector order is not asserted. Actual plan identity, canonical encoding and closed source contents are the relevant comparison. Raw PageSet decoding validates document structure/source closure; registration uniqueness and link semantics are checked by resolve. Raw plan decoding recomputes resolve and compares the result; it does not mint CheckedPages.

The existing 13,757-byte linear-combination document still exports through the real source host at unchanged stage limits. HTML SHA256 is 158cc157066c05a55cfa444a9e90a7bc12bf98054492f1df02fbfcb465f39ada. This establishes the reservation-namespace regression, not a single end-to-end budget or new rendering proof.

`model-probe/src/main.rs` reuses only fixed managed test fixture construction helpers and adds independent expected cases: 14 relative paths, duplicate registration IDs/source paths/routes in both orders, file/directory prefix collisions, Unicode/space logical paths, page-local fragment lookup and caller depth 7. Relative resolution is exact and case-sensitive, normalizes safe dot/dot-dot segments, and rejects root escape, absolute paths, empty segments, directory-only results and implicit extension/index fallback. Removing the target page's label fails even if the requesting page retains the same label.

For resolve and portable plan replay, sampled Work/Allocation/Nodes/Depth/Output caps produce 222 exact sticky stops and 20 exact successes per target, plus cancellation. Successful results equal the unrestricted plan; stopped calls do not return a partial successful plan. Caller depth is composed/restored. Actual source admission is separately tested by the fresh SourceLimit=0 case above. These samples are not an exhaustive all-limit or allocation-failure proof.

## Code and contract observations

Page and node order are retained in links/remaining requirements. Relative paths refer to logical registration sources, not filesystem access. Page targets use explicit IDs; fragments are resolved against target-page labels. Foreign requirements keep their explicit page/owner and are not evaluated by this operation. Full source closure is checked before canonical identity, including all pages and generated sources. Replayed plans are tied to the current ordered PageSet, not merely URL strings.

Source reservation names use byte-length framing, explicit primary identity/revision and a counter. Native and owned paths share the namespace convention. The supplied primary names remain host data; collision safety is also enforced by source identity validation, rather than silently replacing existing snapshots.

The post-freeze delta is separately copied and hashed in post-freeze-delta.json. The only production pages.rs change is into_plan(self) returning the stored plan by move. It was inspected, not included in the executed original snapshot. Test-only panic/unwrap cleanup and print removal do not alter production semantics. Added specification text about the later HTML backend is outside this execution scope. Path-qualified diff files preserve these deltas; the earlier pages.rs.delta.diff name was overwritten during collection and is retained only as a collection artifact.

## Setup failures and limits

The first probe compilation had an ambiguous sum type, corrected to sum::<u64>(); its failure log is retained. PowerShell-to-Python setup replaced Japanese literals with question marks. Those earlier actual binaries/logs/source are explicitly retained under ascii-setup names and are not Unicode evidence. The model's question-mark logical path was correctly rejected as InvalidRegistration; model-unicode-setup files preserve that setup error. Correct UTF-8 literals were subsequently written using explicit Unicode escapes and both probes rebuilt/reran successfully on native and WASI. No production change was made to obtain these results.

Only review snapshots, probes and evidence were written. Production, shared documentation and Git state were not edited.
