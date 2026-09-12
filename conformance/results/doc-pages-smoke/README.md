# Doc HTTP smoke checkpoint

Bounded docs-only HTTP checker: actual source and commands are preserved here.
Independent review reproduced a query-only false success, then confirmed its
rejection after the checker began fetching canonical URLs. Both historical and
corrected reports are retained. Eleven tests (seven payload, four HTTP methods
with multiple failure cases) passed. The local repository check also passed.

Root and independent loopback runs each checked 26 responses from the actual
14-chapter/22-file site already preserved in `../doc-site-build/bundle/`.
That site's input revision is historical and is not relabelled as the checker
commit. No public HTTPS or Pages deployment was performed. Browser, control-plane
identity, journal, recovery and LKG need their own evidence; no acceptance/task
was marked complete. Receipts explicitly retain publication_verified=false.

Restore each `.fixture` by removing only that suffix. Raw evidence bytes are
preserved including original line endings. `seal.json` binds this archive.
