# Independent subject correction review

PASS scoped correction abf9b4b608adeccb7a9bfb087afd276e7577c07d. Exactly two changed files from a20fae7: package/check.rs clears subject immediately before derived FormIndex construction; package test adds Work and Allocation final-one-short checks with typed reason, None subject and sticky budget. Earlier provenance validation errors are unchanged.

Frozen Git archive; no live production modification. Independent old public probe reused with ONLY expected subject changed from Some(Provenance(0)) to None. Native original and fixed observation have identical all Usage fields; only corrected subject changes. Original evidence retained at C:/projects/NEPL3-checked-form-index/.tmp/review-form-index, manifest SHA7ba526b0dd5423fb3d0514751eca1aad1d09eec030b90ce981ed3564fd5c1274.

Native and WASI each run all 21 production package tests plus independent reproduction (22 passed each). Both include real package.check_detailed index Work/Allocation stops and previous invalid provenance rejection. No broader workspace/Doc/browser tests repeated for this two-file correction. Physical allocator failure remains source-inspected rather than forced. This confirms the prior review's one diagnostic-attribution finding is resolved; no further blocking issue found.
