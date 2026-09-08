# Explicit Doc output budget evidence

Production review pins `b84e9a3854dd23538154194054c28713714ba123`; authoring review pins `6ac1b1a`. Original manifests and all payload bytes are retained. The production review preserves an initial zero-test filter run and uses corrected seven-test native/WASI runs as evidence. The authoring review preserves a failed review-helper boundary check and its correction; that was not a document defect.

The shared output allowance remains Work 100M by default. Two predetermined Reader executions use the same 69,199 input bytes, source ID and route. Default execution stops during serialization and creates no output directory. Explicit Work 200M with the other seven limits unchanged succeeds using Work 101,271,279. This is a separate selected execution, not recovery from a stopped Budget. Source and binary digests, command logs, exact input manifests and script-free HTML/CSS are saved. The independent reader review repeats both executions with its own pinned production build and confirms byte-identical HTML, CSS and manifest. Full Markdown semantic comparison remains unexecuted.

Full integration tests and lint logs are recorded at their own source commit. These results do not establish physical heap/time bounds, browser rendering, all documentation links, canonical Doc migration or Pages deployment. Markdown remains canonical.
