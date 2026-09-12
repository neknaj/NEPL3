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
