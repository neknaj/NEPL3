# Math renderer integration tests

This is test infrastructure. The Rust integration test parses real NEPL3 Math,
lowers/checks it, and sends production TeX to the pinned KaTeX package. The Node
adapter performs no NEPL3 semantics. It returns renderer output for assertions;
it is not the trusted product host, markup validator, or Worker protocol.

```sh
npm ci --prefix tools/audit/math --ignore-scripts
cargo test --locked -p nepl3-tools --test math tex::pinned_katex -- --ignored
cargo test --locked -p nepl3-tools --test math tex::native_fixed_katex_asset_binding -- --ignored
```

The test requires Node and installed packages and fails if either is absent.
It is explicitly ignored in the ordinary Rust-only suite. The explicit command
is required to claim renderer integration coverage. Each rendering receives fresh
macros, strict errors, disabled trust and finite KaTeX expansion/size limits.
Passing does not certify fonts, CSS, browser layout, accessibility, resource
termination or validated document artifacts. Generated outputs stay out of Git.

The native asset-binding test requires the real fixed CSS/font/license bytes. It checks the complete inventory, content hashes, identity framing, tamper rejection and preparation/materialization budgets; it is also invoked explicitly by the fixed-assets CI step. It does not certify document insertion or executing renderer identity.
