# Authenticated CI observations and live upload verification

Base a3630c25b249efaee279b3b40ff39f9a6785f2c3. The root implemented fixed-route,
isolated authenticated JSON GETs and verified main, run, attempt jobs, and artifact
metadata against live GitHub. Archive downloads in this interoperability probe
used gh, not a new production download adapter. No Pages POST occurred.

The two real branch CI artifacts for run 34703467910 attempt 1 passed selected
and uploaded with the same original tar and manifest digests. These pins came
from the diagnostic receipt: this establishes upload interoperability, not an
independent content oracle or main publication eligibility. See live report.

The prior candidate site suite passed 100 tests in 391.164 seconds; it predates
the new reader. Reader tests passed 4 normally and 4 with -O. Independent review
also exercised actual isolated IPC, invalid input, and a killed sleeping child.
Repository check passed. All raw archived files are byte-preserved fixtures,
identified by manifest.json. No runtime acceptance or public deployment is
inferred. Remaining work includes authenticated archive acquisition, current
state/permission gates, publisher orchestration, recovery and actual deployment.
