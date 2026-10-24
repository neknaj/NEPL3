<!-- Generated from tools/audit/math/README.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page math&#45;renderer&#45;tests; source SHA-256 f108181b5cdce8d2398bf0308a31b3c4daf291d338b4a78e507a54d6e412e507; alias input SHA-256 a55c6bd38185399c2bf4e8e8870da26bc705f8616dd56387a3b8686d42afa705; page input SHA-256 4c59a521e8b385721d1b21974e848b7c89a0d69b0cc0ea349a0c2a4844623e3e. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="math-renderer-integration-tests"></a>

# Math renderer integration tests

[正本（NEPL3d）](<README.nepld>)

This is test infrastructure\. The Rust integration test parses real NEPL3 Math\, lowers\/checks it\, and sends production TeX to the pinned KaTeX package\. The Node adapter performs no NEPL3 semantics\. It returns renderer output for assertions\; it is not the trusted product host\, markup validator\, or Worker protocol\.

```sh
npm ci --prefix tools/audit/math
cargo test --locked -p nepl3-tools --test math tex::pinned_katex -- --ignored
```

The test requires Node and installed packages and fails if either is absent\. It is explicitly ignored in the ordinary Rust\-only suite\. The explicit command is required to claim renderer integration coverage\. Each rendering receives fresh macros\, strict errors\, disabled trust and finite KaTeX expansion\/size limits\. Passing does not certify fonts\, CSS\, browser layout\, accessibility\, resource termination or validated document artifacts\. Generated outputs stay out of Git\.
