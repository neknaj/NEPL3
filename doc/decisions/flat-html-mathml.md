<!-- Generated from doc/decisions/flat&#45;html&#45;mathml.nepld; renderer nepl3-tools.markdown-annotated-pages/4; page flat&#45;html&#45;mathml; source SHA-256 00f60dcd6825c63f98337b2846d7aa99f5efc683a5d4b3dbe9cceb317bb0e701; alias input SHA-256 bdbb77c04af9f630a118e638b4b07b3cb32a83339e2a9bb5db9c9e26b5184379; page input SHA-256 04b8b0103555528f4d0b3bf43652d6f759b19729f9a9d8e1110b28e2c496a4d0. All-notes viewing profile, not a Doc roundtrip encoding. Edit the Doc source. -->

<a name="flat-html-and-mathml-composition"></a>

# Flat HTML and MathML composition

[正本（NEPL3d）](<flat-html-mathml.nepld>)

HTML needs to contain the checked MathML produced for Doc math nodes\. Adding owned recursive fragments in both directions would make nesting depend on Rust call\-stack traversal and destruction\. `HtmlNode::MathElement` instead uses the same flat arena and U64 child references as HTML elements\. This extends the development schema\'s digest\; receivers still require the exact selected digest\.

The owning namespace determines the allowed attributes and children\. Shared MathML rules check both representations\. Only `math` starts a foreign root in HTML\, and only `mtext` accepts HTML phrasing\. `MathMlNode::Html` denotes an actual HTML\/Text root\; an HTML span may itself contain another inline math root\. ID\, anchor and Ruby ancestry checks continue across these boundaries without resetting state\. Tree traversal\, serialization and destruction remain iterative\/flat\.

The [HTML Standard](<https\:\/\/html\.spec\.whatwg\.org\/multipage\/embedded\-content\-other\.html>) classifies math as phrasing\/flow and permits HTML phrasing in token elements\. NEPL3 deliberately uses the narrower existing `mtext` integration profile and requires inline math in phrasing slots\. The display restriction is a NEPL3 output contract\, not a claim that HTML forbids block\-display math in a paragraph\.

This supplies the structural composition boundary\. A complete PreparedArticle\, resource ownership\, guest\-root remapping and the KaTeX adapter are separate requirements\; structural validity does not establish them or browser layout\.
