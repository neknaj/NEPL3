# Independent bootstrap preparation-stop investigation

Fixed source: 16f9f7651cf2d7cae2ea0f06baab281a4547f7bf. Production worktree and staged evidence archives were not edited. A git-archive scratch copy was compiled into an isolated target.

The managed test with observation-only boundary/cap logging reproduces its original failure (cargo exit 101). Complete measured Work is 112035, so its first cap is 112035 - 32768 = 79267. That cap stops inside profile.resolve at tools/src/bootstrap/runtime.rs:291; map_err(boundary) emits Boundary("Stopped(WorkLimit)"). Consumed Work is 78779. No formal parser reply exists: this is before ParseEnvironmentSet::prepare, ParseSession::new, or read.

A separate test-driver-only sweep allows the unexpected-error branch to record and continue instead of returning immediately. This second run is observation of the remaining cap range, NOT a passing production regression test. Production functions, charge values, limits and error conversions are unchanged. It identifies 34 preparation errors:

- 30 caps from 79267 through 94115 (512 step): profile.resolve, line 291.
- cap 94627: ParseEnvironmentSet::prepare, line 331, stringified Boundary(Stopped(WorkLimit)).
- caps 95139, 95651, 96163: ParseSession::new, line 335, stringified Reader(Schema(Stopped(WorkLimit))).

The unrestricted operation reaches the first parser read at Work 96187. The sampled errors all precede this mark and any formal reply. They should use PreparationStopped carrying the caller's actual Usage. They should not construct a fake ParseReply or be classified as extra input. After a real accepted reply exists, the existing Stopped-with-reply handling is a separate contract.

The static preparation boundary includes package.check and semantic_identity (233/235), provider signature (246), profile resolution (291), source insertion (294/295), environment digest and entry (303/304), context/ParseEnvironmentSet (320/331) and ParseSession construction (335). Their nested typed stop variants can be lost by blanket Debug-to-Boundary mapping. An outer pre-reply preparation wrapper can classify a genuinely stopped shared Budget without depending on error text. Integrity failures must retain their explicit fatal classification; this investigation does not authorize converting arbitrary internal/broken-collector errors into Stopped.

This supports the optimization-exposed test-range explanation: the fixed total-relative sweep now extends into setup. Raising its lower bound would hide the real classification defect; preserving correct preparation failures is preferable. Exact results for a pre-optimization commit were not re-run here, so this is not a proof of which individual optimization first changed that range.

Evidence: reproduce.log is the unchanged managed test's actual failure with observation only. sweep.log is the exploratory all-cap run with one altered test-driver error branch. Source files before/after and both instrumentation scripts are retained. Native Windows only; no WASI or full workspace claims. Fixes belong to the main implementation agent.
