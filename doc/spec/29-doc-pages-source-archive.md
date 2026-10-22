<!-- Generated from doc/spec/29&#45;doc&#45;pages&#45;source&#45;archive.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page doc&#45;pages&#45;source&#45;archive; source SHA-256 d4b698ab6523bbe591e2911b577c64c024df79ea6777b75680dc80c0ea8bd095; alias input SHA-256 de6120d3d5d10a3b30c3431ce5f82ceed746a6b662bc4c0672b4ac2496267eb4; page input SHA-256 a4735d975ce09fa9e386cc32fa9c13d9ef0745a843394061084d99cd868e18fc. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="explicit-doc-pageset-archive-v1"></a>

# Explicit Doc PageSet archive v1

[正本（NEPL3d）](<29-doc-pages-source-archive.nepld>)

`nepl3.doc-pages-archive` revision 1 defines `ArchivedPage`\, with ordered fields `registration: nepl3.doc/1 PageRegistration` and `document: nepl3.doc-archive/1 DocumentArchive`\, and `PageSetArchive`\, with ordered fields `pages: List(ArchivedPage)` and `files: List(nepl3.doc/1 PageFile)`\. This opt\-in structural transport preserves page order\, registrations and file bytes\. The repository checker compares its JSON and Rust descriptors and closes dependencies\.

Encoding uses each original document\'s native structure and maximum guest depths\. The crate\-private encoder retains those exact operation\-local proofs and ordered compact guest components for a future consuming resolver\. Public transport values are data\, not validation proofs\. The selected archive descriptors must be present\; no legacy fallback is allowed\. The enclosing envelope is checked before return\.

Decoding validates the envelope\, reconstructs each document through the explicit Doc archive decoder\, and uses one codec\, source\-admission ledger and cumulative Budget for every page\. Conflicting source revisions across documents reject\. Local declarations cannot be supplied by a different page\'s admitted source\. File bytes are opaque transported data\, never parsed or executed by Doc core\.

This structural API does not check registration collisions\, labels or links and does not establish rendering readiness\. Empty sets have the same structural status as legacy transport\; consuming resolution must apply its own nonempty rule\. No compact PageSet identity\, preparation\-plan route or Markdown renderer is selected by this transport alone\. Existing v1 routes\, domains and descriptor identities remain unchanged\. End\-to\-end historical generation is a separate acceptance condition\.

Explicit consuming identities and link resolution are defined in [archive resolution](<30\-doc\-pages\-archive\-resolution\.md>)\.
