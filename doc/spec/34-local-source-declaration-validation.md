# Reusable validation of local source declarations

`SourceAdmission::checked_source_set` returns an immutable borrow of an owned
`SourceStore` containing exactly the supplied local declarations, never the
admission ledger's union. The returned lifetime belongs to the admission ledger.

Admission and duplicate rejection remain mandatory. `SourceSetError::Duplicate`
denotes a repeated local SourceId/revision; `SourceSetError::Source` retains
source conflict and budget stop errors. Admission precedes duplicate detection
on a miss. A cache hit requires budgeted exact equality of every declaration in
the same sequence. Independently decoded snapshots are not trusted by digest
alone. Empty, reordered or changed local sets remain distinct inputs.

This is a declaration check, not a portable validation receipt or proof of graph,
registry, view, mapping, symbol or rendering correctness. Callers preserve their
operation's cumulative Budget and SourceAdmission. Cache replacement completes
only after source-set construction succeeds. A later consumer failure may leave
this valid local set cached, but all consumer-specific checks still run on retry.

Math structural validation uses this exact local store after finalized-registry
and shape checks. Origins, maps, node locations, views and embedded syntax retain
their independent checks. The store borrow ends before embedded guests require
mutable admission. Duplicate errors remain `StructureError::DuplicateSource`;
source errors and sticky stop reasons retain their existing typed conversion.
