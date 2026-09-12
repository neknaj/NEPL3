# Checked Doc site payload checkpoint

Implementation reviewed at `0c12e9c596a549e2edb9a0585211146673f6e660`.
The actual site input is the previously checked PR105 bundle at input commit
`2390e31127b4d8173c872f61abc48689a24c3cc4`; it was not rebuilt or relabelled.
The site bytes are also preserved under `../doc-site-build/bundle/`.

Independent final review approved packaging and CI handoff after rejecting
non-integer manifest versions. Seven local tests passed normally and with Python
optimization. Earlier independently reproduced junction and scandir failures,
their rejected rechecks, and version acceptance repro are retained.

Remove the `.fixture` suffix to restore ordinary evidence files. The three
identical raw tars are stored once as `final-review/identical-tars.tar.gz.fixture`
to stay within the repository review-size limit. Decompress it with Python
`gzip.decompress` and write the same resulting bytes to `final.tar`, `first.tar`
and `second.tar`. The final reviewer manifest seals all 16 restored originals.
No tar was rebuilt during compression. Each tar has 22 regular members, 1,177,600 bytes, SHA-256
`cc053272f49f8d4d79fc756560df5401bfdc22b8c1e1a1177ae0250cdc265cd9`.
The initial review reports historical open findings, subsequently corrected.

This checkpoint proves local packaging and scoped independent review. It does
not prove public deployment, publisher recovery, LKG promotion, cross-platform
CI completion, or T19/T20/full acceptance. No acceptance status was changed.
