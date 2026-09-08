# Parser validation review archive

Each subdirectory preserves an independent review's original manifest and payloads.
The reviewed source commits, commands, targets, failures and verification limits are
recorded by those reports; this archive does not mark any runtime acceptance group passed.

`collector-before-fix` retains the original failing budget-tampering probes.
`collector-fixed` records their correction. The tree optimizations preserve validation
conditions while changing measured resource costs; stop positions and stopped Usage can
therefore change.

To respect the repository's 1 MiB per-file limit, the original
`selection-lookup/workspace-files.json` is stored as `workspace-files.json.gz`.
It is a source-file identity inventory, not a repository or executable archive. Decompress
that one file when replaying a script that expects its original filename. Its decompressed
byte length and SHA-256 must match the unchanged original manifest.

Run `python conformance/results/parser-validation/verify.py` to verify every archived
payload, transparently decompressing that inventory. The original evidence in the review
workspace remains unchanged.
