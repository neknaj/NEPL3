# Bounded deployment wait review

Implementation 0a7d120. Five tests passed independently normally and under
Python optimization. Additional independent probes check the defensive
128-response limit, exact deadline boundary, public entry dependencies and
retention of prior responses when a later fetch fails. Source snapshots match
implementation Git blobs. Time is deterministic in these tests; neither a
600-second real wait nor actual GitHub status/publishing was exercised.

The root full suite passed 43 tests before discovery of the fifth polling test;
the final five polling tests subsequently passed separately. The existing
smoke timeout fixture emitted a server connection-reset traceback during the
successful full run. These are session observations, not sealed full-run logs.
Root cargo run --locked -p nepl3-tools -- check exited 0; raw logs are retained.
No current publication, journal reconciliation, LKG or final acceptance claim.
