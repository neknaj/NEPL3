// Generated from design/forms.json by tools/generate/grammar.py. Do not edit.
use super::*;
#[rustfmt::skip]
#[derive(Clone,Copy,Debug,Eq,PartialEq)]
pub enum Category {
Root,
Declaration,
Policy,
Field,
ReadSpec,
LexRule,
ReaderExpr,
CharClass,
Binding,
Style,
Selector,
}
#[rustfmt::skip]
#[derive(Clone,Debug,Eq,PartialEq)]
pub enum NodeKind {
Language { name: NameLiteral, revision: NatLiteral, root: NameLiteral, declarations: NodeList },
Category { name: NameLiteral, mode: NameLiteral },
Namespace { name: NameLiteral, policy: NodeId },
Reader { name: NameLiteral, expression: NodeId },
Mode { name: NameLiteral, rules: NodeList },
Form { kind: NameLiteral, category: NameLiteral, spelling: TextLiteral, fields: NodeList, bindings: NodeId, styles: NodeList },
Leaf { kind: NameLiteral, category: NameLiteral, token: NameLiteral, bindings: NodeId, styles: NodeList },
Extension { alias: NameLiteral, provider: TextLiteral, signature: TextLiteral },
Lexical,
Global,
Open,
FieldDeclaration { name: NameLiteral, read: NodeId },
Local { category: NameLiteral },
Foreign { alias: NameLiteral, category: NameLiteral },
ListOf { element: NodeId },
WithMode { mode: NameLiteral, read: NodeId },
Builtin { reader: NameLiteral },
Take { kind: NameLiteral, reader: NameLiteral },
Skip { reader: NameLiteral },
Literal { text: TextLiteral },
Scalar { class: NodeId },
Seq { parts: NodeList },
Choice { alternatives: NodeList },
Many { body: NodeId },
Some { body: NodeId },
Optional { body: NodeId },
Repeat { min: NatLiteral, max: NatLiteral, body: NodeId },
Look { body: NodeId },
Not { body: NodeId },
Commit { body: NodeId },
Capture { name: NameLiteral, body: NodeId },
Region { role: NameLiteral, body: NodeId },
Node { kind: NameLiteral, body: NodeId },
Discard { body: NodeId },
Ref { name: NameLiteral },
Decode { decoder: TextLiteral, body: NodeId },
Map { provider: TextLiteral, body: NodeId },
Then { first: NodeId, provider: TextLiteral },
Call { provider: TextLiteral },
Eof,
Takecount { count: NatLiteral },
Until { delimiter: TextLiteral },
Any,
Whitespace,
Identifierstart,
Identifiercontinue,
Digit,
Asciiletter,
Chars { text: TextLiteral },
Except { text: TextLiteral },
Range { lo: TextLiteral, hi: TextLiteral },
Visit { child: NameLiteral },
Import { child: NameLiteral },
Propagate { child: NameLiteral },
Scope { plans: NodeList },
Group { plans: NodeList },
Bind { namespace: NameLiteral, field: NameLiteral },
Reference { namespace: NameLiteral, field: NameLiteral },
Export { namespace: NameLiteral, field: NameLiteral },
Sequential { declarations: NameLiteral, body: NameLiteral },
Recursive { declarations: NameLiteral, body: NameLiteral },
None,
Custom { provider: TextLiteral },
Style { selector: NodeId, class: TextLiteral },
Head,
SelfSelector,
FieldSelector { name: NameLiteral },
CaptureSelector { name: NameLiteral },
}
#[rustfmt::skip]
impl NodeKind {
pub fn category(&self) -> Category { match self {
Self::Language {..} => Category::Root,
Self::Category {..} => Category::Declaration,
Self::Namespace {..} => Category::Declaration,
Self::Reader {..} => Category::Declaration,
Self::Mode {..} => Category::Declaration,
Self::Form {..} => Category::Declaration,
Self::Leaf {..} => Category::Declaration,
Self::Extension {..} => Category::Declaration,
Self::Lexical => Category::Policy,
Self::Global => Category::Policy,
Self::Open => Category::Policy,
Self::FieldDeclaration {..} => Category::Field,
Self::Local {..} => Category::ReadSpec,
Self::Foreign {..} => Category::ReadSpec,
Self::ListOf {..} => Category::ReadSpec,
Self::WithMode {..} => Category::ReadSpec,
Self::Builtin {..} => Category::ReadSpec,
Self::Take {..} => Category::LexRule,
Self::Skip {..} => Category::LexRule,
Self::Literal {..} => Category::ReaderExpr,
Self::Scalar {..} => Category::ReaderExpr,
Self::Seq {..} => Category::ReaderExpr,
Self::Choice {..} => Category::ReaderExpr,
Self::Many {..} => Category::ReaderExpr,
Self::Some {..} => Category::ReaderExpr,
Self::Optional {..} => Category::ReaderExpr,
Self::Repeat {..} => Category::ReaderExpr,
Self::Look {..} => Category::ReaderExpr,
Self::Not {..} => Category::ReaderExpr,
Self::Commit {..} => Category::ReaderExpr,
Self::Capture {..} => Category::ReaderExpr,
Self::Region {..} => Category::ReaderExpr,
Self::Node {..} => Category::ReaderExpr,
Self::Discard {..} => Category::ReaderExpr,
Self::Ref {..} => Category::ReaderExpr,
Self::Decode {..} => Category::ReaderExpr,
Self::Map {..} => Category::ReaderExpr,
Self::Then {..} => Category::ReaderExpr,
Self::Call {..} => Category::ReaderExpr,
Self::Eof => Category::ReaderExpr,
Self::Takecount {..} => Category::ReaderExpr,
Self::Until {..} => Category::ReaderExpr,
Self::Any => Category::CharClass,
Self::Whitespace => Category::CharClass,
Self::Identifierstart => Category::CharClass,
Self::Identifiercontinue => Category::CharClass,
Self::Digit => Category::CharClass,
Self::Asciiletter => Category::CharClass,
Self::Chars {..} => Category::CharClass,
Self::Except {..} => Category::CharClass,
Self::Range {..} => Category::CharClass,
Self::Visit {..} => Category::Binding,
Self::Import {..} => Category::Binding,
Self::Propagate {..} => Category::Binding,
Self::Scope {..} => Category::Binding,
Self::Group {..} => Category::Binding,
Self::Bind {..} => Category::Binding,
Self::Reference {..} => Category::Binding,
Self::Export {..} => Category::Binding,
Self::Sequential {..} => Category::Binding,
Self::Recursive {..} => Category::Binding,
Self::None => Category::Binding,
Self::Custom {..} => Category::Binding,
Self::Style {..} => Category::Style,
Self::Head => Category::Selector,
Self::SelfSelector => Category::Selector,
Self::FieldSelector {..} => Category::Selector,
Self::CaptureSelector {..} => Category::Selector,
} }
pub fn spelling(&self) -> &'static str { match self {
Self::Language {..} => "language",
Self::Category {..} => "category",
Self::Namespace {..} => "namespace",
Self::Reader {..} => "reader",
Self::Mode {..} => "mode",
Self::Form {..} => "form",
Self::Leaf {..} => "leaf",
Self::Extension {..} => "extension",
Self::Lexical => "lexical",
Self::Global => "global",
Self::Open => "open",
Self::FieldDeclaration {..} => "field",
Self::Local {..} => "local",
Self::Foreign {..} => "foreign",
Self::ListOf {..} => "listof",
Self::WithMode {..} => "withmode",
Self::Builtin {..} => "builtin",
Self::Take {..} => "take",
Self::Skip {..} => "skip",
Self::Literal {..} => "literal",
Self::Scalar {..} => "scalar",
Self::Seq {..} => "seq",
Self::Choice {..} => "choice",
Self::Many {..} => "many",
Self::Some {..} => "some",
Self::Optional {..} => "optional",
Self::Repeat {..} => "repeat",
Self::Look {..} => "look",
Self::Not {..} => "not",
Self::Commit {..} => "commit",
Self::Capture {..} => "capture",
Self::Region {..} => "region",
Self::Node {..} => "node",
Self::Discard {..} => "discard",
Self::Ref {..} => "ref",
Self::Decode {..} => "decode",
Self::Map {..} => "map",
Self::Then {..} => "then",
Self::Call {..} => "call",
Self::Eof => "eof",
Self::Takecount {..} => "takecount",
Self::Until {..} => "until",
Self::Any => "any",
Self::Whitespace => "whitespace",
Self::Identifierstart => "identifierStart",
Self::Identifiercontinue => "identifierContinue",
Self::Digit => "digit",
Self::Asciiletter => "asciiLetter",
Self::Chars {..} => "chars",
Self::Except {..} => "except",
Self::Range {..} => "range",
Self::Visit {..} => "visit",
Self::Import {..} => "import",
Self::Propagate {..} => "propagate",
Self::Scope {..} => "scope",
Self::Group {..} => "group",
Self::Bind {..} => "bind",
Self::Reference {..} => "reference",
Self::Export {..} => "export",
Self::Sequential {..} => "sequential",
Self::Recursive {..} => "recursive",
Self::None => "none",
Self::Custom {..} => "custom",
Self::Style {..} => "style",
Self::Head => "head",
Self::SelfSelector => "self",
Self::FieldSelector {..} => "field",
Self::CaptureSelector {..} => "capture",
} }
pub(super) fn visit(&self, visitor: &mut impl Visitor) -> Result<(), ModelError> { match self {
Self::Language { name, revision, root, declarations } => {
visitor.literal(LiteralRef::Name(name))?;
visitor.literal(LiteralRef::Nat(revision))?;
visitor.literal(LiteralRef::Name(root))?;
visitor.list(declarations, Category::Declaration)?;
Ok(()) },
Self::Category { name, mode } => {
visitor.literal(LiteralRef::Name(name))?;
visitor.literal(LiteralRef::Name(mode))?;
Ok(()) },
Self::Namespace { name, policy } => {
visitor.literal(LiteralRef::Name(name))?;
visitor.node(*policy, Category::Policy)?;
Ok(()) },
Self::Reader { name, expression } => {
visitor.literal(LiteralRef::Name(name))?;
visitor.node(*expression, Category::ReaderExpr)?;
Ok(()) },
Self::Mode { name, rules } => {
visitor.literal(LiteralRef::Name(name))?;
visitor.list(rules, Category::LexRule)?;
Ok(()) },
Self::Form { kind, category, spelling, fields, bindings, styles } => {
visitor.literal(LiteralRef::Name(kind))?;
visitor.literal(LiteralRef::Name(category))?;
visitor.literal(LiteralRef::Text(spelling))?;
visitor.list(fields, Category::Field)?;
visitor.node(*bindings, Category::Binding)?;
visitor.list(styles, Category::Style)?;
Ok(()) },
Self::Leaf { kind, category, token, bindings, styles } => {
visitor.literal(LiteralRef::Name(kind))?;
visitor.literal(LiteralRef::Name(category))?;
visitor.literal(LiteralRef::Name(token))?;
visitor.node(*bindings, Category::Binding)?;
visitor.list(styles, Category::Style)?;
Ok(()) },
Self::Extension { alias, provider, signature } => {
visitor.literal(LiteralRef::Name(alias))?;
visitor.literal(LiteralRef::Text(provider))?;
visitor.literal(LiteralRef::Text(signature))?;
Ok(()) },
Self::Lexical => {
Ok(()) },
Self::Global => {
Ok(()) },
Self::Open => {
Ok(()) },
Self::FieldDeclaration { name, read } => {
visitor.literal(LiteralRef::Name(name))?;
visitor.node(*read, Category::ReadSpec)?;
Ok(()) },
Self::Local { category } => {
visitor.literal(LiteralRef::Name(category))?;
Ok(()) },
Self::Foreign { alias, category } => {
visitor.literal(LiteralRef::Name(alias))?;
visitor.literal(LiteralRef::Name(category))?;
Ok(()) },
Self::ListOf { element } => {
visitor.node(*element, Category::ReadSpec)?;
Ok(()) },
Self::WithMode { mode, read } => {
visitor.literal(LiteralRef::Name(mode))?;
visitor.node(*read, Category::ReadSpec)?;
Ok(()) },
Self::Builtin { reader } => {
visitor.literal(LiteralRef::Name(reader))?;
Ok(()) },
Self::Take { kind, reader } => {
visitor.literal(LiteralRef::Name(kind))?;
visitor.literal(LiteralRef::Name(reader))?;
Ok(()) },
Self::Skip { reader } => {
visitor.literal(LiteralRef::Name(reader))?;
Ok(()) },
Self::Literal { text } => {
visitor.literal(LiteralRef::Text(text))?;
Ok(()) },
Self::Scalar { class } => {
visitor.node(*class, Category::CharClass)?;
Ok(()) },
Self::Seq { parts } => {
visitor.list(parts, Category::ReaderExpr)?;
Ok(()) },
Self::Choice { alternatives } => {
visitor.list(alternatives, Category::ReaderExpr)?;
Ok(()) },
Self::Many { body } => {
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Some { body } => {
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Optional { body } => {
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Repeat { min, max, body } => {
visitor.literal(LiteralRef::Nat(min))?;
visitor.literal(LiteralRef::Nat(max))?;
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Look { body } => {
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Not { body } => {
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Commit { body } => {
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Capture { name, body } => {
visitor.literal(LiteralRef::Name(name))?;
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Region { role, body } => {
visitor.literal(LiteralRef::Name(role))?;
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Node { kind, body } => {
visitor.literal(LiteralRef::Name(kind))?;
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Discard { body } => {
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Ref { name } => {
visitor.literal(LiteralRef::Name(name))?;
Ok(()) },
Self::Decode { decoder, body } => {
visitor.literal(LiteralRef::Text(decoder))?;
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Map { provider, body } => {
visitor.literal(LiteralRef::Text(provider))?;
visitor.node(*body, Category::ReaderExpr)?;
Ok(()) },
Self::Then { first, provider } => {
visitor.node(*first, Category::ReaderExpr)?;
visitor.literal(LiteralRef::Text(provider))?;
Ok(()) },
Self::Call { provider } => {
visitor.literal(LiteralRef::Text(provider))?;
Ok(()) },
Self::Eof => {
Ok(()) },
Self::Takecount { count } => {
visitor.literal(LiteralRef::Nat(count))?;
Ok(()) },
Self::Until { delimiter } => {
visitor.literal(LiteralRef::Text(delimiter))?;
Ok(()) },
Self::Any => {
Ok(()) },
Self::Whitespace => {
Ok(()) },
Self::Identifierstart => {
Ok(()) },
Self::Identifiercontinue => {
Ok(()) },
Self::Digit => {
Ok(()) },
Self::Asciiletter => {
Ok(()) },
Self::Chars { text } => {
visitor.literal(LiteralRef::Text(text))?;
Ok(()) },
Self::Except { text } => {
visitor.literal(LiteralRef::Text(text))?;
Ok(()) },
Self::Range { lo, hi } => {
visitor.literal(LiteralRef::Text(lo))?;
visitor.literal(LiteralRef::Text(hi))?;
Ok(()) },
Self::Visit { child } => {
visitor.literal(LiteralRef::Name(child))?;
Ok(()) },
Self::Import { child } => {
visitor.literal(LiteralRef::Name(child))?;
Ok(()) },
Self::Propagate { child } => {
visitor.literal(LiteralRef::Name(child))?;
Ok(()) },
Self::Scope { plans } => {
visitor.list(plans, Category::Binding)?;
Ok(()) },
Self::Group { plans } => {
visitor.list(plans, Category::Binding)?;
Ok(()) },
Self::Bind { namespace, field } => {
visitor.literal(LiteralRef::Name(namespace))?;
visitor.literal(LiteralRef::Name(field))?;
Ok(()) },
Self::Reference { namespace, field } => {
visitor.literal(LiteralRef::Name(namespace))?;
visitor.literal(LiteralRef::Name(field))?;
Ok(()) },
Self::Export { namespace, field } => {
visitor.literal(LiteralRef::Name(namespace))?;
visitor.literal(LiteralRef::Name(field))?;
Ok(()) },
Self::Sequential { declarations, body } => {
visitor.literal(LiteralRef::Name(declarations))?;
visitor.literal(LiteralRef::Name(body))?;
Ok(()) },
Self::Recursive { declarations, body } => {
visitor.literal(LiteralRef::Name(declarations))?;
visitor.literal(LiteralRef::Name(body))?;
Ok(()) },
Self::None => {
Ok(()) },
Self::Custom { provider } => {
visitor.literal(LiteralRef::Text(provider))?;
Ok(()) },
Self::Style { selector, class } => {
visitor.node(*selector, Category::Selector)?;
visitor.literal(LiteralRef::Text(class))?;
Ok(()) },
Self::Head => {
Ok(()) },
Self::SelfSelector => {
Ok(()) },
Self::FieldSelector { name } => {
visitor.literal(LiteralRef::Name(name))?;
Ok(()) },
Self::CaptureSelector { name } => {
visitor.literal(LiteralRef::Name(name))?;
Ok(()) },
} }
}
