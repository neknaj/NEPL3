# Checked form lookup evidence

Independent selection reuse (3f23aec) and form-index (9dbda46) native/WASI
reviews retain their original bytes. The index-before-fix record contains the
provenance-subject bug; index-fixed verifies its correction at abf9b4b.

The integration logs belong to a20fae7 before the diagnostic-only correction.
The correction has its own public check_detailed native/WASI proof. No runtime
acceptance status is inferred from these development checks.

At fixed 9dbda46 and unchanged budgets, the original 00/01/02 and 86 KB Grammar
drafts exported to HTML. Reader standalone reached NeedsResolution for external
links; its pages route, including the later Reader addition, stopped at WorkLimit.
The results retain source/binary digests and exact commands. Generated HTML is
not repeated in this archive; its export manifest records output digests.
