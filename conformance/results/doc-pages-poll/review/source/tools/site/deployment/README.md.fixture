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
