# Canonical Doc site build checkpoint

Production generation at input commit 2390e31127b4d8173c872f61abc48689a24c3cc4
produced 22 files (1,151,754 bytes) from 14 registered Doc pages. The primary
/NEPL3/ build and its repeat were byte-identical. The /acceptance/project/
variant changed only both indexes, build.json and manifest.json. The stored
primary bundle is the original generated payload; remove only the .fixture
suffix to restore it. Its own manifest verifies every restored file.

Root ran the production build three times, the final identity/link/browser
runner at both bases (192 cases), and workspace tests (576 passed, one existing
ignored), repository checks, Clippy and format. Compiler/executable identity
is separate from input-checkout identity; build.json does not claim the
executable was built from that input commit. Source/binary association for CI
is established by the same checkout build job and retained build logs.

Independent code review corrected compiler provenance, input/final-receipt
bounds and an untracked selected-source gap. The real public API rejects the
latter in a committed-clean fixture. Independent CI review also found that a
self-consistent empty or stale artifact could pass the initial checker. The
final checker requires the actual HEAD, config, full canonical route set,
source digests, renderer digest and nested manifest coverage. Original failure
reproductions are retained. Browser stubs in rejection probes isolate static
validation and are not browser execution evidence.

Independent display review additionally exercised 84 real link clicks and
reloads, CSP blocking with browser JavaScript enabled, and deep fragments.
WebKit's initial immediate-reload timeout and separate successful probes remain
recorded. Final fragment checks wait for initial placement before reload.
WebKit first-Tab navigation was not established; explicit focus/Enter worked.
The existing narrow tutorial table overflow remains separately documented in
the reading-style checkpoint. Full accessibility, Playground and public HTTPS
Pages deployment/recovery have not been established here.

The CI change adds both site builds and the final audit to the existing
required Doc browser job, retaining read-only PR permissions. Its exact-head
remote CI is a separate integration gate. T19 is recorded as in-progress. Neither T19 nor T20 is marked complete,
and no acceptance group is marked passed by this checkpoint. Archived scripts retain original local
paths; the archive seal covers their exact bytes and the saved output.
