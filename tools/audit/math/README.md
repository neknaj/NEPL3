# Math renderer integration tests

This is test infrastructure. The Rust integration test parses real NEPL3 Math,
lowers/checks it, and sends production TeX to the pinned KaTeX package. The Node
adapter performs no NEPL3 semantics. It returns renderer output for assertions;
it is not the trusted product host, markup validator, or Worker protocol.

```sh
npm ci --prefix tools/audit/math
cargo test --locked -p nepl3-tools --test math tex::pinned_katex -- --ignored
```

The test requires Node and installed packages and fails if either is absent.
It is explicitly ignored in the ordinary Rust-only suite. The explicit command
is required to claim renderer integration coverage. Each rendering receives fresh
macros, strict errors, disabled trust and finite KaTeX expansion/size limits.
Passing does not certify fonts, CSS, browser layout, accessibility, resource
termination or validated document artifacts. Generated outputs stay out of Git.
