# Recorded deployment wait evidence

Implementation 5704a7c. Independent review found initial journal loading was
outside the remaining deployment budget. Entry now fixes an absolute deadline;
loading and persistence consume it. The original failing probe is preserved.
Eight real-Git and five poll tests passed independently normally and optimized.
Source snapshots match implementation Git blobs. Independent probes verify no
fetch after expiration, reduced remaining time after loading, and late success
persisted but reported as Deadline. The final root full suite passed 52 tests
and repository checks passed; raw logs/commands are preserved separately from
the earlier pre-fix run. This is local persistence and wait evidence, not
remote journal durability, live deployment, public identity, recovery or LKG.
