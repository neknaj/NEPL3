# Independent actual Reader execution

PASS narrow actual-input verification using previously independently built fixed b84e9a3854dd23538154194054c28713714ba123 production executable; binary SHA6bdf4475e02fedb2add55d4fb41320e0bb527930d8c13ea447421ddbc3f71cec. All tools/src and crates source bytes in the executable checkout rechecked against original Git archive. Reviewer-only edits are test code outside production. The independent build has a different binary SHA from root's build, not a substituted root executable.

Copied exact original manifests and 69199-byte source SHA162fe9ec7faf8c671ef3769fc27342209577db38084d0201220b7871c430bab3 to new scratch. Both original input manifests match after removing explicit output_limits; id candidate, route reference/reader/index.html. Other seven output limits equal default; only Work100000000 versus200000000 differs, selected before each run.

Default run exits1 with serialize WorkLimit and creates no output directory. Explicit200M run exits0, final Work101271279. Independently hashed HTML218111bytes SHA6b0b428902327d3bd0a7d5da43f0418bb7bfda98d13e4da45bf876530e2b24c6; CSS1860bytes SHA95e1239989b6be84d51d5cbf23a0c48f655d43392fc1e1e0e3791d5235d413d2. These and manifest3099bytes are byte-identical to root's separate run. Default failure remains recorded and is not corrected into success.

No full Markdown versus rendered semantic text comparison was newly executed; existing subset guide parser does not handle this complete Reader chapter. This evidence establishes actual native production generation under selected limits and deterministic artifact bytes, not full-content acceptance, browser display, Pages deployment or all-Doc migration. No production edits or resource defaults changed by reviewer.
