<!-- Generated from doc/spec/28&#45;doc&#45;source&#45;archive.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page doc&#45;source&#45;archive; source SHA-256 9aa4e0c4c020c6e0627e29dc5e7869c93d78a394a76423508b646758168ed3c5; alias input SHA-256 2b48da747a150f5fe34e6c80410d4b8f1361332828d27cb121938587a3631bdb; page input SHA-256 c386ae073e929531f0226aac59758079eafcf96b0bd56ef84163630d07eafe1c. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="explicit-doc-source-archive-v1"></a>

# Explicit Doc source archive v1

[正本（NEPL3d）](<28-doc-source-archive.nepld>)

`nepl3.doc-archive` revision 1 defines `DocumentArchive` with ordered fields `skeleton: nepl3.doc/1 DocumentSyntax` and `group: Bytes`\. The latter is a checked GroupArchive\/1 byte stream\. The JSON contract and Rust descriptor are compared by the repository contract checker\, including the Doc\/Foundation dependency closure\. The host must also register the source\/group archive contracts required by its explicit archive codec\. Missing contracts reject\; they never select a v1 fallback\.

This is an opt\-in transport API\. Existing DocumentSyntax\, PageSet\, preparation\, HTML\, Text and Print APIs\, operation domains and descriptor identities are unchanged\. The ordinary FoundationValueCodec trait remains usable without archive support\. The optional FoundationGroupArchiveCodec capability has a real concrete wire implementation and no success\-shaped default\. It shares its ordinary codec\'s registry\, SourceAdmission and cumulative Budget\. Schema capability selection is codec boundary work\; once selected\, group encoding preserves caller\-relative semantic depths and checks a distinct codec registry independently\.

<a name="n-656e636f64696e67"></a>

<a name="encoding"></a>

## Encoding

Prepare the original native Doc structure and its maximum occurrence depth for each shared embed\. Encode that exact group through the depth\-aware compact Foundation operation\, retaining GroupArchive bytes and ordered v2 guest component commitments\. A provider result with a different component count rejects\.

Encode the otherwise unchanged Doc value with an empty Foundation group at the typed DocEmbeds\.group field\. Preserve embed kinds and references\, document sources\, origins\, views and mappings\. Validate the skeleton structurally as DocumentSyntax\, then validate the complete DocumentArchive envelope before returning it\. The skeleton is not independently valid native Doc syntax when it contains embeds\. It does not replace the original native validation or prove render readiness\.

The returned guest components are the v2 Foundation entry commitments defined in [the group archive contract](<27\-group\-source\-archive\.md>)\. No new document or preparation\-plan identity is defined by this transport API\.

<a name="n-6465636f64696e67"></a>

<a name="decoding"></a>

## Decoding

Validate the explicit envelope and its Doc skeleton against their selected registry identities\. Decode GroupArchive bytes into a native group\, restoring and validating every local source declaration set\. Decode the document\'s own source scope using the same admission ledger\; conflicting document\/guest identities reject\. A globally admitted declaration cannot fill a missing local set\.

Private generated typed reader hooks restore DocValue\.embeds and DocEmbeds\.group without expanding the native group back into v1 NDF\. All ordinary fields retain the existing read order and Work charges\. The placeholder group must be empty in all three tables\; a nonempty placeholder rejects\. Opaque group payloads are not rewritten\. Origins\, mappings and views retain the existing document\-local scope\.

Finally validate the complete restored native Doc structure\, including the maximum depth of each shared embed across shallow\/deep owner paths under the active outer Budget depth\. Successful GroupArchive decoding alone cannot discharge this Doc requirement\. Wrong archive\/source membership\, missing declarations\, incompatible registries and malformed native values reject before a document is returned\.

<a name="n-7265736f75726365"></a>

<a name="resource-and-acceptance-scope"></a>

## Resource and acceptance scope

All descriptor lookup\, capture\, validation\, allocation\, canonical encoding\, hashing\, restoration and Doc validation use the supplied Budget\. Caller\-side raw NDF serialization\/deserialization must use that same budget for a single composite boundary operation\. Stops retain their reason and consumed usage\; no successful partial document or archive is returned\.

Native\/CBOR roundtrip\, alternate codec parity\, unchanged v1 boundaries\, route mismatch\, empty groups\, nonempty placeholders\, source conflicts\, deepest shared occurrence\, provider cardinality\, cancellation and exact\/one\-short budgets are transport acceptance requirements\. This milestone does not select a compact PageSet\/preparation\/rendering route or establish real\-data generation under 4B Work\.

Page\-set structural transport is specified separately in [Doc PageSet archive](<29\-doc\-pages\-source\-archive\.md>)\.
