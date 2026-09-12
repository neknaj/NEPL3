# Pages deployment responses

`receipt.created` validates a bounded UTF-8 JSON creation response and binds its
ID and status URL to the configured repository. It never follows the returned
URL. The GET endpoint is constructed locally. The optional `page_url` is not
used to select a public destination; the pinned SiteConfig owns that URL.

The [GitHub REST documentation](https://docs.github.com/en/rest/pages/pages)
creation example has a `status_url` ending in `/status`, while the documented
GET endpoint has no suffix. Both matching receipt spellings are accepted and
normalized to the documented GET route. Cross-repository/ID/host URLs fail.

`receipt.observed` accepts only a response attributed by the host transport to
that exact GET endpoint. It distinguishes pending, succeeded, failed and unknown
states. Unknown server states require reconciliation, never implicit success
or unbounded polling. `deployment_lost` is unknown because the final state is
not known. Known temporary errors (`unknown_status`, `not_found`, and
`deployment_attempt_error`) also require reconciliation rather than automatic
retry in this adapter. Direct Receipt construction and observation entry both
validate ID/endpoint/digest consistency. The failure names are checked against
[deploy-pages](https://github.com/actions/deploy-pages/blob/main/src/internal/deployment.js).
Raw response digests bind observations to evidence; callers retain the bytes.
Duplicate keys, nonfinite numbers, invalid field types and oversized bodies fail.
Additional server fields are ignored rather than promoted into trusted state.

These parsers perform no HTTP calls or publication operations. A transport must
validate HTTP status, endpoint, authentication/TLS and response size before
attributing a response. A parsed `succeed` identifies only the requested
operation's result: it does not prove current publication, remote journal
freshness, payload identity, public smoke or immutable LKG storage. Publisher
state transitions and transport are subsequent implementation work.

Run `python -m unittest discover -s tools/site -p test_deployment.py`.

## Status transport

`transport.status` runs a single authenticated GET in an isolated Python child.
The validated receipt determines a path under the fixed `api.github.com` host;
HTTPS uses standard certificate/hostname verification, with no proxy, cookies,
redirect following or alternate-host configuration. GitHub API version is pinned
to `2026-03-10`. Credentials pass through stdin, never process arguments or
error output. Only a bounded original response crosses back as base64 and is
validated again by the parent.

The parent terminates and waits for the child if the request exceeds its
configured wait (at most ten seconds after subprocess creation). This bounds
slow headers, trickling bodies and DNS as well as normal I/O timeouts. Process
creation itself is not interruptible on every platform, and termination/wait
plus scheduling can add overhead; this is not a strict end-to-end realtime
guarantee. The
publisher still owns its total 600-second status-wait and transaction budgets;
this transport does not retry, cancel, deploy or assume unknown means success.

Only HTTP 200 with JSON MIME and identity content encoding is accepted. Bodies
are bounded to 64 KiB. Duplicate lengths, invalid lengths, conflicting transfer
framing and short declared bodies fail. A chunked body must be valid HTTP
chunk framing. HTTP/network/parse errors are sanitized and contain no token or
server body; failed requests cannot produce an Observation.

Transport tests use real loopback HTTP with only the HTTPS socket factory
replaced, and a real child killed at deadline. They prove HTTP parsing and
process termination, not live GitHub authentication or TLS interoperability.
Default TLS behavior follows the [Python HTTPSConnection contract](https://docs.python.org/3/library/http.client.html#http.client.HTTPSConnection).
Run `python -m unittest discover -s tools/site -p test_transport.py`.

## Deployment wait

`poll.wait(receipt, token, timeout=600)` connects status transport to bounded
observation. It returns a typed Report with the deployment ID, stop reason and
original successful HTTP responses. Pending replies wait five seconds (or the
remaining budget); each subprocess receives at most ten seconds and never a
fresh budget beyond the remaining wait. At most 128 responses are retained.
Unknown status, terminal failure and transport failure stop without retries.
They require publisher reconciliation. Expiration after a late `succeed` keeps
that response as evidence but returns Deadline, not Succeeded.

This wait performs no cancel/deploy/rollback and does not promote LKG. A
Succeeded report is one input to the publisher's public-identity/smoke/journal
checks, never proof of current publication. Overall process startup/cleanup
limits remain as described above. Deterministic time tests exercise deadline
edges without waiting 600 real seconds; the production entry uses monotonic
time, real sleep and the bounded transport.
Run `python -m unittest discover -s tools/site -p test_poll.py`.

## Creation receipt journal bridge

`deployment.journal.record_created` validates the API creation response and
records it immediately after its matching DeployIntent or RecoveryIntent in the
local Git journal, using the observed parent as CAS. All transaction/run/attempt,
source and payload fields are preserved. A repeated or stale acknowledgement
cannot append another receipt. `latest_created` reloads the exact original
response, revalidates its repository/endpoint/ID and compares the complete event
identity with the preceding intent. Generic storage accepting an event name is
not sufficient for this bridge to trust its evidence.

Receipt evidence version 1 has exactly `version`, `owner`, `repository`, and
`response` (base64 of the original UTF-8 response bytes). The serialized envelope
must fit the journal's existing 64 KiB limit; large responses therefore fail
before changing the ref. No digest-only replacement or truncation is used.

This bridge records an already obtained observation. It does not validate the
intent's deployment authorization, push it to the protected remote, reconcile
an unacknowledged API request or select a current publication. Those remain
publisher responsibilities. Tests use real temporary bare Git repositories;
no live remote or Pages write is performed.
Run `python -m unittest discover -s tools/site -p test_receipt_journal.py`.

`record_status` appends a validated original status response following that
receipt (or its pending observations), preserving all intent identity fields.
`status_history` locates the preceding creation receipt and revalidates every
following Observation, its exact request endpoint and original response. The
status evidence is exactly version 1, `request_url`, and base64 `response`.
Journal envelope bounds and CAS apply to each write. The bridge accepts more
observations only after Pending; success, failure or unknown status ends this
sequence and requires a separate publisher transition/reconciliation. Reload
also rejects a forged sequence added through the generic storage API.

A status history is an ordered set of API observations, not public identity,
smoke success or LKG proof. This bridge does not infer real-time ordering beyond
the journal order, nor authenticate raw responses supplied by callers: the
bounded transport and publisher must attribute the actual requests.

`wait_recorded` connects the bounded polling transport to this local journal.
Every received response is appended before another request or success return.
If append/replay/CAS fails, the exception stops the wait and no further fetch is
issued. The caller can reload the durable winning head instead of assuming the
failed append happened. The returned pair is the last recorded head and wait
report; late success is still recorded but the report remains Deadline.

The required `remaining_seconds` is the publisher's remaining deployment budget,
not a fresh timeout on restart. Initial journal read/validation and subsequent
recording consume this same deadline. The host must calculate it from its transaction
record; this function cannot reconstruct elapsed wall time from API responses.
Terminal/unknown history is refused, requiring explicit reconciliation. Local
writes here do not imply remote durability; remote publication of observations,
stop/incident records and the overall publisher state machine remain separate.

`wait_remote` adds the remote boundary: it confirms the receipt head against the
pinned origin before polling, then publishes and confirms each locally appended
observation before the next request or a successful return. Failure preserves the
local evidence and propagates; it does not retry a write or discard the unmatched
head. Resume requires explicit reconciliation of local and remote history.
Initial remote confirmation and subsequent pushes consume the same remaining
budget. Git subprocesses retain their bounded per-command timeout; this is not a
hard real-time interrupt of a Git operation already in progress. The caller still
owns the publisher lock, verifies remote protection and interprets terminal
observations before any deployment or LKG change. This function performs no Pages
deployment, publication smoke or LKG promotion.

## Immutable recovery storage receipt

`release.verify` checks the release metadata and the bytes obtained from its
three pinned assets. The recovery storage profile uses `payload.tar` (the
original Pages tar), `identity.json` (publication identity) and `smoke.json`
(public smoke evidence). It requires a `site-recovery/` transaction tag, pinned
release/asset IDs and SHA-256/size, a published non-draft immutable release,
exactly these uploaded assets, and matching actual downloaded bytes. Metadata
is bounded to 64 KiB and each asset to 40 MiB. No missing digest, duplicate
asset, mutable release or altered download is silently accepted.

The returned StorageReceipt records storage identity only. Before LKG
promotion, the publisher must separately validate the original tar using
`recovery.verify`, interpret identity/smoke contents, check current deployment
and remote journal, and authenticate metadata/download transport. This parser
neither downloads nor publishes releases, resolves tags to commits, or proves
that a supplied JSON smoke report is true. API `target_commitish` is not used
as tag-resolution proof. Existing LKG reference validation remains necessary
before a later deployment because a whole release can still be deleted.

Metadata fields follow the [GitHub releases REST contract](https://docs.github.com/en/rest/releases/releases).
Run `python -m unittest discover -s tools/site -p test_release.py`.

`release.recover` connects that storage check to `recovery.verify`: the pinned
payload asset digest must match the downloaded bytes, and the original raw tar
must satisfy the canonical archive, exact file closure and independently pinned
manifest checks. It returns the storage receipt and the unchanged original tar
bytes for staging. A matching release digest for arbitrary bytes is insufficient.
The caller supplies pins from its validated LKG journal record. Saved identity
and smoke semantics, authenticated download, current-publication reconciliation
and deployment authorization remain publisher responsibilities; this function
does not promote the release to LKG or deploy it.

## Single creation attempt

`create.create` sends one bounded POST attempt to the fixed Pages API through
an isolated subprocess. The request uses the pinned numeric artifact ID,
40-hex build version, `github-pages` environment and Actions OIDC credential.
Both credentials cross stdin only, not argv or diagnostic output. The response
uses the same bounded HTTP framing checks as status GET and is validated against
the requested repository before its original bytes and receipt are returned.

The publisher must confirm its durable intent, artifact provenance, permission,
lock and publication gates before calling this transport. This module does not
supply those gates or obtain credentials. Any worker/connection/response failure
after starting the attempt yields CreationUnknown, never an automatic retry or
an assumption that the server did nothing. Persist the returned receipt before
polling; reconcile an unresolved intent after loss of the response. Parent
process timeout kills and collects the child, with ordinary process scheduling
limitations rather than a hard real-time guarantee.

The request follows the [Pages creation REST contract](https://docs.github.com/en/rest/pages/pages#create-a-github-pages-deployment).
Tests exercise local HTTP and subprocess boundaries, not live API authorization.

## Intent-to-receipt submission

`submit.submit` connects an already authorized attempt to the remote journal:
validate request and expected local/remote heads, reject an already used
transaction/kind pair, append the intent with repository/artifact/build/environment
identity, confirm its remote push, issue one POST, append the original validated
receipt, and confirm that remote push before returning. Credentials are not
recorded in intent evidence. Recovery uses the same ordering with RecoveryIntent.

A failure after local append retains the evidence. Lost API response leaves a
remote intent; failed receipt push leaves the local receipt for reconciliation.
Calling submit again with that intent is rejected, even with an updated expected
head. Before appending any new intent, submit also verifies every historical
creation intent has an adjacent matching receipt with valid original API
evidence. A changed transaction ID or intent kind cannot bypass a missing receipt.
This does not establish that a receipted deployment has finished or is healthy;
the caller must not evade the remaining reconciliation gates with a new ID.
Eligibility, current-publication reconciliation, artifact provenance, remote
protection and the single-writer lock remain the publisher's prerequisites.
These storage operations cannot establish those facts from an Event alone.

The caller supplies the transaction's remaining budget, at most 3600 seconds.
Recording and push time consume it; no POST begins after the intent confirmation
has exhausted it. A received receipt is recorded even if time expires afterward,
and expiration still prevents a successful return. Each Git subprocess has its
existing bounded timeout; total wall time is not a hard real-time guarantee.
No public smoke, recovery decision or LKG promotion is inferred from submission.

`submit.execute` connects submission to `wait_remote` using the returned receipt
head. It subtracts all submission time from the caller's remaining transaction
budget and caps status waiting at 600 seconds. Unknown or failed submission never
starts status polling. The resulting head/report contain remotely recorded API
observations; even Succeeded is not public smoke or LKG promotion. Neither this
entry nor submit can be retried as a substitute for resuming recorded state.

## Public smoke evidence journal

`deployment.smoke.record` requires the expected remotely confirmed successful
API status history. Before HTTP, it checks the local site's exact manifest/file
closure, reconstructs only its canonical tar identity for comparison with the
submitted payload digest, and checks source commit and HTTPS base path. It does
not replace or upload a rebuilt payload. The public byte checker uses at most
300 seconds of the same remaining transaction budget.

The report must match manifest, source (on pass), URL and HTTPS transport. Its
publication_verified flag must remain false: byte smoke is not current Pages
identity or LKG proof. Passed/failed results are appended as SmokePassed or
SmokeFailed and remotely confirmed. A late HTTP pass is preserved but recorded
as SmokeFailed with deadline_exceeded. A late push cannot return success; its
journal evidence remains available to reconciliation. Invalid/oversized reports
or CAS/push errors are not truncated or turned into a successful smoke.

Call only after successful execute and publisher current-state eligibility
checks. These adapters do not infer that a recorded late status permits a new
budget, decide rollback, or promote LKG. Tests inject HTTP reports into this
connection; the standalone smoke tests cover HTTP byte checking separately.
