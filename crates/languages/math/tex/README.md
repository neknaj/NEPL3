# Math → TeX

`render` consumes the existing checked Math expression and prepares TeX for a
generation-side host. It does not evaluate Math, execute KaTeX, or certify HTML.
The crate uses only `no_std + alloc` and the declared Math/core dependencies.

Fixed commands come from the constructor structure. Symbol/text/binder strings
are escaped literal content, never commands or caller-supplied macros. Operands
use conservative visible grouping; source printers and MathML are not inputs.
Foreign annotations, unsupported delimiters and control characters return typed
capability failures; callers must not drop them. ASCII apostrophe and backtick
also return `LiteralQuote`: the tested KaTeX text path changes them to curly
quotes, including when encoded with `char`. Dash ligatures are prevented without
splitting Unicode combining sequences. Identifiers use `mathord` with `textit`
to retain literal names and italic display; this does not certify the accessibility
semantics of KaTeX's generated MathML. Numeric and resource failures
remain distinct, and a stopped budget cannot be reset to turn fallback into success.

The result borrows the exact MathValue and records UTF-8 ranges for emitted
occurrences, including repeated appearances of a shared node. These ranges route
back to the original node's span/origin; they do not prove global source admission.

This implements the pure conversion portion of T24. Fixed-version KaTeX execution,
HTML validation, CSS/fonts, Node/Worker comparison and browser tests remain host
work. Successful TeX construction alone is not evidence of faithful browser output.
The native API is not a portable ABI or a completed renderer capability.

The Doc host's `MathDisplayHost::prepare_node` now prepares independent MathML
and optional TeX from the same lowered Doc Math input. `MathMLOnly` omits TeX;
`KaTeXPreferred` retains typed unsupported-node reasons with the MathML result.
The shared budget covers both preparations: exhaustion rejects the operation
even when MathML was already available. `TexPreparation::Ready` means renderer
input is ready, not that KaTeX or its output validator has run. Annotation
documents and origin mappings remain attached to the MathML fallback.
