# CI candidate gate validation

Base: 70f2d7e71cdcf2c1838ddde72df4db2b3c1d4d01. Checks authenticated run/job observations against publisher-selected repository, workflow, run, attempt and main SHA. This is not artifact-attempt provenance, a freshness fetch, publication permission or LKG proof.

Root: 5 tests normal/-O passed; repository check exit 0; diff check passed. Independent review snapshots, normal/-O tests, boundary probes and historical API interoperability are byte-exact fixtures. Manifest paths acquire .fixture here. The historical run is accepted against its historical main SHA and rejected against the current main SHA; it is not an eligible current candidate.

Earlier full site run (PR119 scope before this candidate module): 87 tests passed in 383.067 seconds. The new candidate tests are separate. No runtime acceptance status is promoted and Pages is not deployed.
