<!-- Generated from doc/spec/34&#45;local&#45;source&#45;declaration&#45;validation.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page local&#45;source&#45;declaration&#45;validation; source SHA-256 0557b947a32f9a00b2c168c54a4d858c6a9e52caee3aa5af72ce977ef23be959; alias input SHA-256 2b8f0f9835c4e517729b11bc0c1ec93fed18e3fd605a6bd6d68628e85947b81d; page input SHA-256 17b31014fc78ea5b56e429c02d6a0665076d882763698008b30e8a9eb114876b. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="reusable-validation-of-local-source-declarations"></a>

# Reusable validation of local source declarations

[正本（NEPL3d）](<34-local-source-declaration-validation.nepld>)

`SourceAdmission::checked_source_set` returns an immutable borrow of an owned `SourceStore` containing exactly the supplied local declarations\, never the admission ledger\'s union\. The returned lifetime belongs to the admission ledger\.

Admission and duplicate rejection remain mandatory\. `SourceSetError::Duplicate` denotes a repeated local SourceId\/revision\; `SourceSetError::Source` retains source conflict and budget stop errors\. Admission precedes duplicate detection on a miss\. A cache hit requires budgeted exact equality of every declaration in the same sequence\. Independently decoded snapshots are not trusted by digest alone\. Empty\, reordered or changed local sets remain distinct inputs\.

This is a declaration check\, not a portable validation receipt or proof of graph\, registry\, view\, mapping\, symbol or rendering correctness\. Callers preserve their operation\'s cumulative Budget and SourceAdmission\. Cache replacement completes only after source\-set construction succeeds\. A later consumer failure may leave this valid local set cached\, but all consumer\-specific checks still run on retry\.

Math structural validation uses this exact local store after finalized\-registry and shape checks\. Origins\, maps\, node locations\, views and embedded syntax retain their independent checks\. The store borrow ends before embedded guests require mutable admission\. Duplicate errors remain `StructureError::DuplicateSource`\; source errors and sticky stop reasons retain their existing typed conversion\.
