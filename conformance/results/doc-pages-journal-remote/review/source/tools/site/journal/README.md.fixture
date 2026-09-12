# Pages journal storage

This host-only module implements the local Git storage portion of
[the Pages recovery contract](../../../doc/spec/15-site.md). It is not the
publisher state machine and does not authorize an external write.

`load(bare_mirror)` returns the exact head, ordered typed events and original
evidence bytes. `append(bare_mirror, expected_head, event, evidence)` retains
all prior blobs and adds one event/evidence pair in a commit whose sole parent
is `expected_head`. It updates only `refs/heads/pages-state`, with Git's old-OID
comparison. A stale read or concurrent winner rejects the update. Unreferenced
objects may remain after a rejected CAS; they are not committed journal events.
Normal source repositories and symbolic journal refs are rejected.

Each sequence has `00000001.event.json` and `00000001.evidence.json` flat regular
blobs. Event format version 1 is UTF-8 JSON, sorted keys, compact separators,
one final LF, and exactly these fields:

| Field | Contract |
| --- | --- |
| version | integer 1 (not boolean or float) |
| sequence | consecutive positive integer, starting at 1 |
| kind | one of the event names listed in `model.KINDS` |
| transaction | 1–80 ASCII letters, digits or hyphens |
| run_id | integer in 1 through 2^64−1 |
| attempt | integer in 1 through 2^32−1 |
| source_commit | 40 lowercase hexadecimal characters |
| payload_sha256 | 64 lowercase hexadecimal characters identifying the raw tar |
| evidence_sha256 | SHA-256 of the exact accompanying evidence bytes |

Evidence must be a UTF-8 JSON object without duplicate keys or nonfinite
numbers. Its original whitespace is retained. The storage boundary does not
interpret deployment IDs, observations, smoke results or release verification
inside it. Those belong to the publisher's event-specific contracts and must
be validated before an event can change publication state. An event named
`Healthy` by itself is never proof that a deployment is healthy.

Limits: 1,024 events, 4,096 bytes per event envelope, 65,536 bytes per evidence,
32 MiB combined event/evidence bytes. Reads check contiguous pairs, regular
blob mode, exact envelope fields, integer/identity constraints, canonical event
bytes and evidence hashes. Appends reject limits before changing the ref.
Reads also audit the complete single-parent history: each commit must add only
its consecutive event/evidence pair, whose blobs must remain unchanged in the
current tree. Missing/shallow history, extra commits, merges and prior record
rewrites are rejected. Git replacement objects are disabled for these reads.
The protected remote branch still owns authorization and rollback prevention;
local structural validation cannot prove remote freshness or writer identity.

The local CAS is not remote durability. Before calling Pages, the future
publisher must normally fast-forward push the intent, confirm the protected
remote ref, and hold the required production concurrency lock. It must also
interpret unresolved intent, recovery and LKG state before issuing operations.
`remote.publish(mirror, expected_origin_url, expected_remote_head, new_head)`
adds the transport step: it requires a single matching fetch/push URL for origin,
validates local history and the immediate parent, compares the observed remote
head, normally pushes the explicit new SHA, then observes the remote head again.
Tags are not followed even if user Git configuration enables that behavior.
If the remote already has that exact new commit, it confirms without another
write, for resumption after a lost acknowledgement. Other changed heads, push
failures or unknown acknowledgement stop the operation; there is no force or
automatic retry. Tests use separate local bare servers and mirrors.

The caller still needs the production lock and verified protected remote:
normal fast-forward push rejects a competing descendant, but this transport
alone cannot prevent an administrator rollback or prove protection settings.
`Confirmation` is remote-ref evidence, never deployment permission. State
interpretation, API reconciliation and branch protection configuration remain
separate work. No NEPL3 remote pages-state ref is written by these tests.

Tests use real temporary Git repositories, including two racing writers and
reload after an intent. No test writes the NEPL3 remote or its source worktree.
Run `python -m unittest discover -s tools/site -p test_journal.py`.

The old-value check follows [Git update-ref](https://git-scm.com/docs/git-update-ref);
tree and parent construction use [mktree](https://git-scm.com/docs/git-mktree)
and [commit-tree](https://git-scm.com/docs/git-commit-tree).
