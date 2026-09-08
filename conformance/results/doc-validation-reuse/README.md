# Doc validation reuse evidence

Independent reviews cover page preparation at `8b6088e0ff818fae5d8176bc2690dbba42ed43f5` and source uniqueness at `13b2155`. Native and WASI comparisons exercise production APIs, portable boundaries, source conflicts, canonical identities, failure order, cancellation and resource limits. Original manifests and payload bytes are preserved.

The page review preserves and explicitly excludes initial measurements contaminated by a shared Cargo target directory. Its final comparisons rebuild before and after in separate target directories. The 69,199-byte Reader observation still stops during Document digest computation; it is not HTML completion evidence. No resource limit is raised by this change.

Integration command logs are separate from independent review. Neither these tests nor the small real-source examples establish all runtime acceptance groups, documentation cutover or Pages deployment.
