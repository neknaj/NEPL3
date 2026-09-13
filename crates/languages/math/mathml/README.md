# MathML backend

This `no_std + alloc` crate implements structural display from a borrowed
`CheckedExpression`, following specification 06 section 4. It does not evaluate
the expression. For example, `frac 1 0` remains a fraction, while multiplication
of an addition inserts visible parentheses according to binding power.

`render` returns a typed MathML fragment and `node_roots`, indexed by the input
Math arena. The latter links rendered roots to the caller's retained source and
Origin records. It does not certify an entire SourceMap, source admission, guest
identity, artifact resources or browser layout. The output is checked with the
markup structural validator before return. The neutral fragment codec is in
`nepl3_markup::portable::mathml`.

All non-annotation expression constructors are handled, including structural
matrix rows. Inline sums/integrals use scripts; block mode uses under/over.
Numbers use exact canonical finite-decimal printing. Shared input nodes retain
shared output roots, and serialization expands their occurrences in order.
All construction, numeric printing and validation use the caller's sticky Budget.

Annotation integration is unfinished. `DocGuest`/`Label` returns the typed
`AnnotationRequiresPreparation` error rather than removing annotation content.
The required next stage is prepared Doc phrasing through MathML `mtext`, including
Ruby/Anno. Markup now provides mixed-namespace depth/identity checks and XHTML
serialization; the Math annotation preparation/adapter still needs to use them.
This crate's presence is not completion of T24, Math rendering acceptance, KaTeX,
the document artifact pipeline or browser verification.

Tests use real NEPL3 parsing/lowering in `tools/tests/math/mathml.rs`, fixed
specification constructor inputs, manually derived XML, shared-node input and
resource stops. No separate review runner or copied source snapshot is needed.
