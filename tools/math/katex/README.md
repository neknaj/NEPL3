# Generation-side KaTeX call

`render.mjs` is the synchronous invocation layer shared by future Node and
browser Worker hosts. A caller injects fixed KaTeX 0.18.7; its absence does not
prevent independent Rust MathML startup. Real-NEPL TeX tests use this module.

`node/render.mjs` runs that call in a fresh Node Worker per expression, using a
trusted host-configured local module URL. It checks input before structured
cloning, captures console records in the isolated realm, and bounds diagnostic
UTF-8 bytes (including a byte per record). Deadline and AbortSignal stop the
realm; the promise resolves only after exit and stdio drainage. Unexpected
direct stdio or premature exit is a provider violation. A termination failure
is recorded, and is not reported as a confirmed exit while the realm is alive.
The deadline includes module loading, not only rendering. Cancellation before
final resolution wins over output, including termination and stream drainage.
The owning document operation must still check its current
source/revision and stop state before using any result.

This Worker is a CPU cancellation mechanism for trusted renderer code, not an
OS capability sandbox or an exact memory bound. Node's timer is not a hard
real-time guarantee. Terminated calls omit unobserved diagnostics/usage rather
than inventing zero usage. A local deadline outcome alone never authorizes a
document fallback. The one-shot lifetime also prevents module-global state from
leaking between requests; amortized batch/pool performance is not claimed.

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
The Node helper now supplies the invocation-local isolation/capture/stop portion;
the browser Worker and complete operation protocol remain separate work.
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
Run `node --test tools/audit/math/node.test.mjs` for actual Node realm, warning,
missing-module, stdio/exit, cancellation and synchronous-loop termination tests.
The test-only renderer fixture exercises failures and is never a math oracle.
Worker lifecycle follows the [Node Worker API](https://nodejs.org/api/worker_threads.html).
