# Flat HTML and MathML composition

HTML needs to contain the checked MathML produced for Doc math nodes. Adding
owned recursive fragments in both directions would make nesting depend on Rust
call-stack traversal and destruction. `HtmlNode::MathElement` instead uses the
same flat arena and U64 child references as HTML elements. This extends the
development schema's digest; receivers still require the exact selected digest.

The owning namespace determines the allowed attributes and children. Shared
MathML rules check both representations. Only `math` starts a foreign root in
HTML, and only `mtext` accepts HTML phrasing. `MathMlNode::Html` denotes an actual
HTML/Text root; an HTML span may itself contain another inline math root. ID,
anchor and Ruby ancestry checks continue across these boundaries without resetting
state. Tree traversal, serialization and destruction remain iterative/flat.

The [HTML Standard](https://html.spec.whatwg.org/multipage/embedded-content-other.html)
classifies math as phrasing/flow and permits HTML phrasing in token elements.
NEPL3 deliberately uses the narrower existing `mtext` integration profile and
requires inline math in phrasing slots. The display restriction is a NEPL3 output
contract, not a claim that HTML forbids block-display math in a paragraph.

This supplies the structural composition boundary. A complete PreparedArticle,
resource ownership, guest-root remapping and the KaTeX adapter are separate
requirements; structural validity does not establish them or browser layout.
