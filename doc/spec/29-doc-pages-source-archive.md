# Explicit Doc PageSet archive v1

`nepl3.doc-pages-archive` revision 1 defines `ArchivedPage`, with ordered fields
`registration: nepl3.doc/1 PageRegistration` and
`document: nepl3.doc-archive/1 DocumentArchive`, and `PageSetArchive`, with ordered
fields `pages: List(ArchivedPage)` and `files: List(nepl3.doc/1 PageFile)`.
This opt-in structural transport preserves page order, registrations and file bytes.
The repository checker compares its JSON and Rust descriptors and closes dependencies.

Encoding uses each original document's native structure and maximum guest depths.
The crate-private encoder retains those exact operation-local proofs and ordered
compact guest components for a future consuming resolver. Public transport values
are data, not validation proofs. The selected archive descriptors must be present;
no legacy fallback is allowed. The enclosing envelope is checked before return.

Decoding validates the envelope, reconstructs each document through the explicit
Doc archive decoder, and uses one codec, source-admission ledger and cumulative
Budget for every page. Conflicting source revisions across documents reject.
Local declarations cannot be supplied by a different page's admitted source.
File bytes are opaque transported data, never parsed or executed by Doc core.

This structural API does not check registration collisions, labels or links and
does not establish rendering readiness. Empty sets have the same structural status
as legacy transport; consuming resolution must apply its own nonempty rule.
No compact PageSet identity, preparation-plan route or Markdown renderer is selected
by this transport alone. Existing v1 routes, domains and descriptor identities remain
unchanged. End-to-end historical generation is a separate acceptance condition.

Explicit consuming identities and link resolution are defined in [archive resolution](30-doc-pages-archive-resolution.md).
