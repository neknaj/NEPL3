<!-- Generated from doc/spec/33&#45;doc&#45;commitment&#45;profile.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page doc&#45;commitment&#45;profile; source SHA-256 74923b421b21e39e1b466b1dc0714b8da7dcfd7909f350f5e9e3b5516c4141fb; alias input SHA-256 73c92aedab201fe170dfc10b3935b22d3612b66356a27a842ab1794f58409322; page input SHA-256 7dcdd6940c86b726731fd65a6d1c3280e66d3361794b310731bb0a9d395ab6e6. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="identity-only-doc-pageset-profile"></a>

# Identity\-only Doc PageSet profile

[正本（NEPL3d）](<33-doc-commitment-profile.nepld>)

`pages::compact::resolve` selects PageIdentityProfile\:\:CompactV1\. It requires the optional FoundationGroupCommitmentCodec rather than producing archive transport\. LegacyV1 and ArchiveV1 retain their existing preimages\, domains and adapters\.

A private document preimage is exactly a two\-element NDF List containing\:

1. The structurally checked Doc DocumentSyntax skeleton whose typed embed group is empty\; all other fields\, embed kinds\/references\, sources\, origins\, views and mappings are unchanged\.
1. Bytes32 of the complete Foundation group commitment from spec32\.

The original native Doc and maximum consuming occurrence depth of every guest are validated first\. A provider\'s component count must match the native group\. The retained native proof stays operation\-local\; the skeleton is not standalone native Doc syntax\. No fabricated schema record or transport decode API is used\.

A private PageSet preimage is a two\-element List containing the ordered page list and checked PageFiles\. Each page is a two\-element List of checked PageRegistration and the private document preimage\. Files bind registrations and exact content bytes\. These lists are internal hash preimages only\, not portable PageSet\/archive values\.

Domains are `NEPL3.Doc.Compact.Pages.v1\0`\, `NEPL3.Doc.Compact.Document.v1\0`\, and `NEPL3.Doc.Compact.Guest.v1\0`\; the last wraps the existing v2 entry commitments\. The explicit plan adapters recompute this profile and compare canonical plan data under `NEPL3.Doc.Compact.PagePlan.v1\0`\. Cross\-profile plans reject\. CheckedPages exposes the selected profile\; native structures\/text helpers retain original limits\, outer\-depth and borrowed\-document scope\. Registration and link\-resolution ordering remain shared with existing resolvers\. Stops retain their sticky reason\.

All native\/schema validation\, source admission\, preimage construction and hashing use the same cumulative Budget\. No serialized archive bytes are produced\, and no output limit is reset or increased\. This profile alone does not select a tools renderer or prove historical\-corpus generation succeeds\.
