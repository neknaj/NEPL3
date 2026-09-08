# Independent owned parser collector review

Reviewed production commit: 5c7f4f3b087c0147fda0be9c8db7e9d6bae1e0f3.
Baseline: b3f894b (full SHA in sources.json).
Production was clean when frozen with git archive. All probes modify only extracted scratch copies. No production edits.

## P1: same-Limits callback budget rollback can become ordinary Stopped or retry

The fresh public ParseSession host path was invoked using real resolved package, provider, tokenizer and parser APIs. A test-only host resets the mutable operation Budget to Budget::new(budget.limits()) on its second provider callback. Four callback results were tested:

| Response after rollback | Actual result |
| --- | --- |
| Ok(None) | Reader(Continuation) error |
| cancel + Ok(None) | successful Stopped(Cancelled) with progress |
| Err(Context) | owned Await accepted, normal retry reaches Complete |
| cancel + Err(Context) | successful Stopped(Cancelled) with progress |

The last three do not meet the newly stated integrity-failure termination rule. The two-case before experiment reproduces reset+cancel on b3f894b too: this is an inherited weakness, not a newly introduced regression. The API claims still need correction before approval.

The reader detects rollback against callback-entry Usage at runtime/mod.rs:828, stores Continuation, then returns a suspension. Tokenizer reply() rewrites report.usage from the current Budget. RecoverableHostReply::reject compares that rewritten Usage against the same Budget, losing the earlier integrity observation. Further, the engine's deliberately preserved callback-error gate accepts the wrapper whenever its adapter recorded the callback transport error, bypassing reject.

Recommended correction: preserve a distinct integrity observation immediately around every provider/reservation callback, independently of transport errors and before matching Ok/Err. Propagate it through the native recovery path as a fatal failure before the engine's old transport-error gate. A saved token-entry Usage alone does not preserve callback-entry Usage. Continuation is not an exclusive integrity-error tag: it also represents frame/continuation errors.

Audit all dispatch locations: runtime/mod.rs dispatch_native; tokenizer/session.rs drive_host Await (including Transform fallback), and Reserve. The latter two currently lack an immediate unconditional callback-entry Usage/Limits check on None/Err exits.

## Other observations

- Moving accepted.take inside Budget::with_depth_at_least preserves ownership when its preflight declines to call the closure.
- RecoverableHostReply couples its private mark with the exact owned reply, exposing no mark applicable to another collector.
- Normal callback error still reaches an owned retry boundary; reader-side invalid reply follows the old rejection gate.
- BrokenCollector is handled before finish's budget.poll and closes the parse session, preventing the explicitly represented broken collector from becoming ordinary Stopped.
- Actual NeedMore/Await continuation ownership/copies remain; only per-token backup is removed.
- The allocation boundary test's declared-primary union is correct: reply.sources owns additional sources and need not duplicate the request's primary snapshot.
- Long 12/24-parent inputs compare complete outcomes, diagnostics, generated sources and source maps between real native-host and owned-provider paths; they match. Both commits pass this comparison, with lower measured Work/Allocation after backup removal.
- Independent private fault injection covers all four missing-prefix vector cases with cancellation and confirms BrokenPrefix, plus valid Stopped retaining live ownership and nonstopping rejection recovering the entry prefix. The public host reset experiment separately shows why the current integrity detection is insufficient.

## Validation boundaries

Production reader+engine native suite: 126 tests passed. Separate probe logs retain failures, rather than converting them into acceptance evidence. WASI results are recorded separately in the final manifest after completion. The initial WASI build used a scratch copy to which test-only probes were added during compilation; it reproduced the failing probe, but is not claimed as a pristine suite run. A separately extracted immutable pristine directory is used for the definitive production WASI run.

No full workspace/Doc HTML/browser/bare-metal run was performed by this reviewer. No acceptance catalog state or authoring source was modified. Review remains changes-requested for the integrity issue until a fixed snapshot is rechecked.
