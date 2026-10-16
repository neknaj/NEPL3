# Explicit archive PageSet resolution

`pages::archive::resolve` opts into the archive identity profile. Ordinary
`pages::resolve` keeps the legacy profile. Both snapshot the original Budget limits
and current depth, check page/file registrations before encoding, preserve page order,
check labels, derive requirements, and resolve page/file targets and fragments in the
same order with the same cumulative Budget. Native structure proofs remain scoped to
the exact borrowed PageSet. No asset or guest lowering success is implied.

The archive profile uses the exact checked PageSetArchive envelope for the set digest
and each contained DocumentArchive envelope for its document digest. Domains are
`NEPL3.Doc.Archive.Pages.v1\0` and `NEPL3.Doc.Archive.Document.v1\0` respectively.
Each original group's compact v2 entry commitment is wrapped by
`NEPL3.Doc.Archive.Guest.v1\0` before requirement derivation. Existing legacy domains
are unchanged. These profiles deliberately have different document/guest identities
although their original document contents and link semantics may agree.

CheckedPages exposes PageIdentityProfile. Its plan, document_digest and into_plan
refer to that profile; consumers must select a compatible identity contract rather
than infer it from positional field shapes. This marker is native metadata, not a
portable proof. The existing PageLinkPlan data schema can carry either profile's
values, but the explicit archive plan adapters recompute archive resolution and
compare canonical plan commitments under `NEPL3.Doc.Archive.PagePlan.v1\0`.
Legacy plans reject through archive adapters and archive plans reject through legacy
adapters. Decoding a supplied plan never creates CheckedPages.

No default Markdown, HTML, Text or Print renderer is switched by these APIs.
Tools profile selection and replay receipts require a separate explicit integration.
The historical23page corpus must still be tested under the unchanged normal4B budget
before claiming recovery.

The explicit archive resolver reports a nested boundary stop as its top-level
PageError::Stopped while retaining the Budget's original sticky reason. Legacy
error classification is unchanged. The archive plan boundary shares this rule
through its normal PortableError conversion.
