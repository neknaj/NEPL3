# Generation-side KaTeX call

`render.mjs` is the synchronous invocation layer shared by future Node and
browser Worker hosts. A caller injects fixed KaTeX 0.18.7; its absence does not
prevent independent Rust MathML startup. Real-NEPL TeX tests use this module.

This is an internal host API, not a complete portable operation envelope.
It returns **unchecked renderer output**, never an artifact or fidelity proof.
`html` is the visual profile; `htmlAndMathml` is the explicit comparison profile.
Neither detects broken CSS/fonts. Input/output limits count UTF-8 bytes and
reject isolated surrogates. Options use finite maxExpand/maxSize, strict errors,
trust=false and a fresh empty macro object. The pinned ParseError rawMessage
distinguishes expansion limits from ordinary parse errors without returning
unescaped exception text. Internal exceptions/version mismatches are violations.

The outer host still owns complete request/resource identity, reservation and
settlement, isolated execution, console capture, timeout/cancel and transport.
This call cannot interrupt synchronous KaTeX or measure its internal allocation.
KaTeX can emit metric warnings through its own console; the outer isolated realm
must capture those diagnostics. No exception does not mean faithful output.
maxSize can clamp dimensions, so producer/profile fidelity must be established
before admission. Arbitrary caller macros are not supported.

Markup/CSS/SVG validation, stylesheet extraction, CSS/fonts, accessibility,
Node/real-Worker comparison and Doc export remain under
[spec17](../../../doc/spec/17-math-html.md). Never insert this unchecked string
directly into a document. These outcomes do not authorize a stopped parent
operation to fall back.

Run `node --test tools/audit/math/render.test.mjs` after
`npm ci --prefix tools/audit/math --ignore-scripts`. The Rust integration test
separately feeds real NEPL expressions through production TeX and this call.
