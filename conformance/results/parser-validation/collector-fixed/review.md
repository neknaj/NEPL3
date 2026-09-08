# Independent fixed owned-collector review

Fixed commit: 9b11ed30dd797d8a9fad8bf3b1765d91299de5d5, compared directly with 5c7f4f3b087c0147fda0be9c8db7e9d6bae1e0f3.
Initial before comparison remains sealed separately under ../review-owned-engine/manifest.json (SHA db5d3329b430db3c818da8011f0f75018c228db0b0b59e54485d3ef910c15315).

## Source review

IntegrityHost captures Limits, cumulative Usage, active depth and existing stop immediately before each provider or reservation callback and examines the values immediately after it. This check is independent of the callback's Ok(Some), Ok(None), or Err return. Its sticky violated observation is checked by the deferred API before applying `?` to its own result or exposing a RecoverableHostReply to the engine's old host error gate. Violation closes the tokenizer and returns BrokenPrefix; engine maps that to BrokenCollector and closes before normal budget-poll/Stopped publication.

This addresses both the failed reset+cancel probe and the transport-error gate bypass. Ordinary callback errors retain the original owned retry behavior. Usage comparisons are monotonic instead of exact, allowing legitimate callback work. A token-entry high-water comparison alone is not substituted for the callback-entry observation.

Budget::with_depth now restores the saved caller depth, matching with_depth_at_least's ownership rule, rather than decrementing an externally replaced depth. The regression exercises a nested depth-7 caller and cancellation after replacement at depth 8; restoration returns first to 7, then to 0, preserving Cancelled. This prevents unsigned underflow observed in the reservation route. The enclosing integrity guard still rejects replacement; restoring depth does not authorize the altered Budget.

No new blocking issue was identified in this delta. The earlier findings about preserving NeedMore/Await copies, accepted ownership during depth preflight, and source-closure union remain applicable.

## Independent probes

The previous scratch tests are reapplied to a separately extracted snapshot. Test-only Action names are prefixed Review to avoid collisions with the author's new managed tests. No production behavior is patched.

- Four real provider cases: same-Limits Budget replacement, with/without cancel, with None or callback Err. All must return a fatal error; previously three returned successful results.
- Four independent reservation cases with the same matrix and actual escaped Text input. All require BrokenCollector; cancellation must survive as observed_stop.
- The actual native and owned-provider paths compare complete outcomes, diagnostics, additional sources and source maps for 12- and 24-parent inputs.
- Private test-only fault injection shortens each of the four saved prefix dimensions and checks BrokenPrefix even with Cancelled. A separate check keeps a valid Stopped reply live and recovers a nonstopping rejected prefix.

Pristine production core/reader/engine tests and these independent probes run separately on native Windows and wasm32-wasip2/Wasmtime. Exact counts, exits, tool versions, and hashes are in sources.json and manifest.json. No full-workspace, browser, Doc export, or bare-metal results are inferred from this scoped review.

No production, canonical manuscript, task or acceptance state was edited by this reviewer.
