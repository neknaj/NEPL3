# Preparation stop correction review

Fixed commit 459dcbc (full SHA source.json), independently extracted from Git without altering the live worktree or staged archives. Reviewed the entire runtime/tests delta against 16f9f765.

No new blocking issue found. Preparation classification is introduced only before the first parser invocation: package and signature checks, schemas, profile/source setup, environment/context and ParseSession construction. The helper returns PreparationStopped with the actual caller Usage only when that Budget reports a stop; otherwise it returns the previous error. Explicit unsupported-provider/signature failures retain their nonstopping Boundary behavior. Formal-reply handling after initial read, including stopped host/trivia/finishing work retaining its accepted reply, is unchanged.

The old failed cap sweep now runs without weakening its failure expectations. Independent observation confirms that the same 34 caps previously returned as stringified Boundary errors now return PreparationStopped with exactly the same eight Usage fields. The original source error log remains in ../review-bootstrap-stop. Additional zero-Work direct-with_tree managed regression passes, keeping sticky stop and Usage correspondence. Observed complete-parse trailing-comment stops still have a real Complete reply and pass the existing report Usage assertion; no reply is fabricated for setup stops.

An independent test passes a compiled fixture with a nonexistent root context directly to the real runtime::with_tree. It remains a nonstopping Boundary error while budget.poll() is Ok, rather than being reclassified as a stop. This test-only malformed input is isolated in the scratch copy. No production edits.

Scoped native bootstrap suite: 9 passed, 1 ignored. The ignored test remains unrun, and this is not a full workspace/WASI/Doc-export claim. Instrumentation adds cap/Usage logging only; the original sweep assertions remain active. results.json records exact cap Usage matching and formal-stop caps. Production source and scratch test source are retained with hashes.
