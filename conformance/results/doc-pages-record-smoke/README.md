# Submitted payload public smoke connection

Implementation 12543b2. Independent review found and reproduced acceptance of a
301-second HTTP pass when the transaction allowed 3600 seconds. The fix preserves
an independent smoke deadline capped at 300 seconds and rejects late success.
Five final tests passed normally and optimized. Independent exact-300/301-second
and late-ack cases are preserved with the original reproduction and review.
Source snapshots match implementation Git blobs after LF normalization.

Root pre-fix full suite: 76 passed (not final-source evidence). Root final scoped
smoke suite: 5 passed; repository checks passed. Raw logs are stored separately.
No live HTTPS deployment, current-publication identity, LKG or rollback is proved
by these local Git tests with injected smoke reports. Standalone HTTP smoke tests
remain separate, and acceptance status is unchanged.
