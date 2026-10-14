// Generated from design/forms.json by tools/generate/grammar.py. Do not edit.
use super::*;
#[rustfmt::skip]
mod adapters {
use super::*;
impl Value for NodeId {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"NodeId",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,38)?;
let f=fields(v,s,"NodeId",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for NodeList {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"NodeList",[self.items.put(s,c,b)?,self.span.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"NodeList",2)?;
Ok(Self {items:Value::read(&f[0],s,c,b)?,span:Value::read(&f[1],s,c,b)?})
}
}
impl Value for Node {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"Node",[self.kind.put(s,c,b)?,self.span.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,36)?;
let f=fields(v,s,"Node",2)?;
Ok(Self {kind:Value::read(&f[0],s,c,b)?,span:Value::read(&f[1],s,c,b)?})
}
}
impl Value for NodeKind {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Language {name,revision,root,declarations} => variant(s,"NodeKind","Language",[put_name(name,s,c,b)?,put_nat(revision,s,c,b)?,put_name(root,s,c,b)?,declarations.put(s,c,b)?],b),
Self::Category {name,mode} => variant(s,"NodeKind","Category",[put_name(name,s,c,b)?,put_name(mode,s,c,b)?],b),
Self::Namespace {name,policy} => variant(s,"NodeKind","Namespace",[put_name(name,s,c,b)?,policy.put(s,c,b)?],b),
Self::Reader {name,expression} => variant(s,"NodeKind","Reader",[put_name(name,s,c,b)?,expression.put(s,c,b)?],b),
Self::Mode {name,rules} => variant(s,"NodeKind","Mode",[put_name(name,s,c,b)?,rules.put(s,c,b)?],b),
Self::Form {kind,category,spelling,fields,bindings,styles} => variant(s,"NodeKind","Form",[put_name(kind,s,c,b)?,put_name(category,s,c,b)?,put_text(spelling,s,c,b)?,fields.put(s,c,b)?,bindings.put(s,c,b)?,styles.put(s,c,b)?],b),
Self::Leaf {kind,category,token,bindings,styles} => variant(s,"NodeKind","Leaf",[put_name(kind,s,c,b)?,put_name(category,s,c,b)?,put_name(token,s,c,b)?,bindings.put(s,c,b)?,styles.put(s,c,b)?],b),
Self::Extension {alias,provider,signature} => variant(s,"NodeKind","Extension",[put_name(alias,s,c,b)?,put_text(provider,s,c,b)?,put_text(signature,s,c,b)?],b),
Self::Lexical => variant(s,"NodeKind","Lexical",[],b),
Self::Global => variant(s,"NodeKind","Global",[],b),
Self::Open => variant(s,"NodeKind","Open",[],b),
Self::FieldDeclaration {name,read} => variant(s,"NodeKind","FieldDeclaration",[put_name(name,s,c,b)?,read.put(s,c,b)?],b),
Self::Local {category} => variant(s,"NodeKind","Local",[put_name(category,s,c,b)?],b),
Self::Foreign {alias,category} => variant(s,"NodeKind","Foreign",[put_name(alias,s,c,b)?,put_name(category,s,c,b)?],b),
Self::ListOf {element} => variant(s,"NodeKind","ListOf",[element.put(s,c,b)?],b),
Self::WithMode {mode,read} => variant(s,"NodeKind","WithMode",[put_name(mode,s,c,b)?,read.put(s,c,b)?],b),
Self::Builtin {reader} => variant(s,"NodeKind","Builtin",[put_name(reader,s,c,b)?],b),
Self::Take {kind,reader} => variant(s,"NodeKind","Take",[put_name(kind,s,c,b)?,put_name(reader,s,c,b)?],b),
Self::Skip {reader} => variant(s,"NodeKind","Skip",[put_name(reader,s,c,b)?],b),
Self::Literal {text} => variant(s,"NodeKind","Literal",[put_text(text,s,c,b)?],b),
Self::Scalar {class} => variant(s,"NodeKind","Scalar",[class.put(s,c,b)?],b),
Self::Seq {parts} => variant(s,"NodeKind","Seq",[parts.put(s,c,b)?],b),
Self::Choice {alternatives} => variant(s,"NodeKind","Choice",[alternatives.put(s,c,b)?],b),
Self::Many {body} => variant(s,"NodeKind","Many",[body.put(s,c,b)?],b),
Self::Some {body} => variant(s,"NodeKind","Some",[body.put(s,c,b)?],b),
Self::Optional {body} => variant(s,"NodeKind","Optional",[body.put(s,c,b)?],b),
Self::Repeat {min,max,body} => variant(s,"NodeKind","Repeat",[put_nat(min,s,c,b)?,put_nat(max,s,c,b)?,body.put(s,c,b)?],b),
Self::Look {body} => variant(s,"NodeKind","Look",[body.put(s,c,b)?],b),
Self::Not {body} => variant(s,"NodeKind","Not",[body.put(s,c,b)?],b),
Self::Commit {body} => variant(s,"NodeKind","Commit",[body.put(s,c,b)?],b),
Self::Capture {name,body} => variant(s,"NodeKind","Capture",[put_name(name,s,c,b)?,body.put(s,c,b)?],b),
Self::Region {role,body} => variant(s,"NodeKind","Region",[put_name(role,s,c,b)?,body.put(s,c,b)?],b),
Self::Node {kind,body} => variant(s,"NodeKind","Node",[put_name(kind,s,c,b)?,body.put(s,c,b)?],b),
Self::Discard {body} => variant(s,"NodeKind","Discard",[body.put(s,c,b)?],b),
Self::Ref {name} => variant(s,"NodeKind","Ref",[put_name(name,s,c,b)?],b),
Self::Decode {decoder,body} => variant(s,"NodeKind","Decode",[put_text(decoder,s,c,b)?,body.put(s,c,b)?],b),
Self::Map {provider,body} => variant(s,"NodeKind","Map",[put_text(provider,s,c,b)?,body.put(s,c,b)?],b),
Self::Then {first,provider} => variant(s,"NodeKind","Then",[first.put(s,c,b)?,put_text(provider,s,c,b)?],b),
Self::Call {provider} => variant(s,"NodeKind","Call",[put_text(provider,s,c,b)?],b),
Self::Eof => variant(s,"NodeKind","Eof",[],b),
Self::Takecount {count} => variant(s,"NodeKind","Takecount",[put_nat(count,s,c,b)?],b),
Self::Until {delimiter} => variant(s,"NodeKind","Until",[put_text(delimiter,s,c,b)?],b),
Self::Any => variant(s,"NodeKind","Any",[],b),
Self::Whitespace => variant(s,"NodeKind","Whitespace",[],b),
Self::Identifierstart => variant(s,"NodeKind","Identifierstart",[],b),
Self::Identifiercontinue => variant(s,"NodeKind","Identifiercontinue",[],b),
Self::Digit => variant(s,"NodeKind","Digit",[],b),
Self::Asciiletter => variant(s,"NodeKind","Asciiletter",[],b),
Self::Chars {text} => variant(s,"NodeKind","Chars",[put_text(text,s,c,b)?],b),
Self::Except {text} => variant(s,"NodeKind","Except",[put_text(text,s,c,b)?],b),
Self::Range {lo,hi} => variant(s,"NodeKind","Range",[put_text(lo,s,c,b)?,put_text(hi,s,c,b)?],b),
Self::Visit {child} => variant(s,"NodeKind","Visit",[put_name(child,s,c,b)?],b),
Self::Import {child} => variant(s,"NodeKind","Import",[put_name(child,s,c,b)?],b),
Self::Propagate {child} => variant(s,"NodeKind","Propagate",[put_name(child,s,c,b)?],b),
Self::Scope {plans} => variant(s,"NodeKind","Scope",[plans.put(s,c,b)?],b),
Self::Group {plans} => variant(s,"NodeKind","Group",[plans.put(s,c,b)?],b),
Self::Bind {namespace,field} => variant(s,"NodeKind","Bind",[put_name(namespace,s,c,b)?,put_name(field,s,c,b)?],b),
Self::Reference {namespace,field} => variant(s,"NodeKind","Reference",[put_name(namespace,s,c,b)?,put_name(field,s,c,b)?],b),
Self::Export {namespace,field} => variant(s,"NodeKind","Export",[put_name(namespace,s,c,b)?,put_name(field,s,c,b)?],b),
Self::Sequential {declarations,body} => variant(s,"NodeKind","Sequential",[put_name(declarations,s,c,b)?,put_name(body,s,c,b)?],b),
Self::Recursive {declarations,body} => variant(s,"NodeKind","Recursive",[put_name(declarations,s,c,b)?,put_name(body,s,c,b)?],b),
Self::None => variant(s,"NodeKind","None",[],b),
Self::Custom {provider} => variant(s,"NodeKind","Custom",[put_text(provider,s,c,b)?],b),
Self::Style {selector,class} => variant(s,"NodeKind","Style",[selector.put(s,c,b)?,put_text(class,s,c,b)?],b),
Self::Selection {selector,priority} => variant(s,"NodeKind","Selection",[selector.put(s,c,b)?,put_nat(priority,s,c,b)?],b),
Self::Head => variant(s,"NodeKind","Head",[],b),
Self::SelfSelector => variant(s,"NodeKind","Self",[],b),
Self::FieldSelector {name} => variant(s,"NodeKind","FieldSelector",[put_name(name,s,c,b)?],b),
Self::CaptureSelector {name} => variant(s,"NodeKind","CaptureSelector",[put_name(name,s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"NodeKind")?;
match (tag,f.len()) {
("Language",4)=>Ok(Self::Language {name:read_name(&f[0],s,c,b)?,revision:read_nat(&f[1],s,c,b)?,root:read_name(&f[2],s,c,b)?,declarations:Value::read(&f[3],s,c,b)?}),
("Category",2)=>Ok(Self::Category {name:read_name(&f[0],s,c,b)?,mode:read_name(&f[1],s,c,b)?}),
("Namespace",2)=>Ok(Self::Namespace {name:read_name(&f[0],s,c,b)?,policy:Value::read(&f[1],s,c,b)?}),
("Reader",2)=>Ok(Self::Reader {name:read_name(&f[0],s,c,b)?,expression:Value::read(&f[1],s,c,b)?}),
("Mode",2)=>Ok(Self::Mode {name:read_name(&f[0],s,c,b)?,rules:Value::read(&f[1],s,c,b)?}),
("Form",6)=>Ok(Self::Form {kind:read_name(&f[0],s,c,b)?,category:read_name(&f[1],s,c,b)?,spelling:read_text(&f[2],s,c,b)?,fields:Value::read(&f[3],s,c,b)?,bindings:Value::read(&f[4],s,c,b)?,styles:Value::read(&f[5],s,c,b)?}),
("Leaf",5)=>Ok(Self::Leaf {kind:read_name(&f[0],s,c,b)?,category:read_name(&f[1],s,c,b)?,token:read_name(&f[2],s,c,b)?,bindings:Value::read(&f[3],s,c,b)?,styles:Value::read(&f[4],s,c,b)?}),
("Extension",3)=>Ok(Self::Extension {alias:read_name(&f[0],s,c,b)?,provider:read_text(&f[1],s,c,b)?,signature:read_text(&f[2],s,c,b)?}),
("Lexical",0)=>Ok(Self::Lexical),
("Global",0)=>Ok(Self::Global),
("Open",0)=>Ok(Self::Open),
("FieldDeclaration",2)=>Ok(Self::FieldDeclaration {name:read_name(&f[0],s,c,b)?,read:Value::read(&f[1],s,c,b)?}),
("Local",1)=>Ok(Self::Local {category:read_name(&f[0],s,c,b)?}),
("Foreign",2)=>Ok(Self::Foreign {alias:read_name(&f[0],s,c,b)?,category:read_name(&f[1],s,c,b)?}),
("ListOf",1)=>Ok(Self::ListOf {element:Value::read(&f[0],s,c,b)?}),
("WithMode",2)=>Ok(Self::WithMode {mode:read_name(&f[0],s,c,b)?,read:Value::read(&f[1],s,c,b)?}),
("Builtin",1)=>Ok(Self::Builtin {reader:read_name(&f[0],s,c,b)?}),
("Take",2)=>Ok(Self::Take {kind:read_name(&f[0],s,c,b)?,reader:read_name(&f[1],s,c,b)?}),
("Skip",1)=>Ok(Self::Skip {reader:read_name(&f[0],s,c,b)?}),
("Literal",1)=>Ok(Self::Literal {text:read_text(&f[0],s,c,b)?}),
("Scalar",1)=>Ok(Self::Scalar {class:Value::read(&f[0],s,c,b)?}),
("Seq",1)=>Ok(Self::Seq {parts:Value::read(&f[0],s,c,b)?}),
("Choice",1)=>Ok(Self::Choice {alternatives:Value::read(&f[0],s,c,b)?}),
("Many",1)=>Ok(Self::Many {body:Value::read(&f[0],s,c,b)?}),
("Some",1)=>Ok(Self::Some {body:Value::read(&f[0],s,c,b)?}),
("Optional",1)=>Ok(Self::Optional {body:Value::read(&f[0],s,c,b)?}),
("Repeat",3)=>Ok(Self::Repeat {min:read_nat(&f[0],s,c,b)?,max:read_nat(&f[1],s,c,b)?,body:Value::read(&f[2],s,c,b)?}),
("Look",1)=>Ok(Self::Look {body:Value::read(&f[0],s,c,b)?}),
("Not",1)=>Ok(Self::Not {body:Value::read(&f[0],s,c,b)?}),
("Commit",1)=>Ok(Self::Commit {body:Value::read(&f[0],s,c,b)?}),
("Capture",2)=>Ok(Self::Capture {name:read_name(&f[0],s,c,b)?,body:Value::read(&f[1],s,c,b)?}),
("Region",2)=>Ok(Self::Region {role:read_name(&f[0],s,c,b)?,body:Value::read(&f[1],s,c,b)?}),
("Node",2)=>Ok(Self::Node {kind:read_name(&f[0],s,c,b)?,body:Value::read(&f[1],s,c,b)?}),
("Discard",1)=>Ok(Self::Discard {body:Value::read(&f[0],s,c,b)?}),
("Ref",1)=>Ok(Self::Ref {name:read_name(&f[0],s,c,b)?}),
("Decode",2)=>Ok(Self::Decode {decoder:read_text(&f[0],s,c,b)?,body:Value::read(&f[1],s,c,b)?}),
("Map",2)=>Ok(Self::Map {provider:read_text(&f[0],s,c,b)?,body:Value::read(&f[1],s,c,b)?}),
("Then",2)=>Ok(Self::Then {first:Value::read(&f[0],s,c,b)?,provider:read_text(&f[1],s,c,b)?}),
("Call",1)=>Ok(Self::Call {provider:read_text(&f[0],s,c,b)?}),
("Eof",0)=>Ok(Self::Eof),
("Takecount",1)=>Ok(Self::Takecount {count:read_nat(&f[0],s,c,b)?}),
("Until",1)=>Ok(Self::Until {delimiter:read_text(&f[0],s,c,b)?}),
("Any",0)=>Ok(Self::Any),
("Whitespace",0)=>Ok(Self::Whitespace),
("Identifierstart",0)=>Ok(Self::Identifierstart),
("Identifiercontinue",0)=>Ok(Self::Identifiercontinue),
("Digit",0)=>Ok(Self::Digit),
("Asciiletter",0)=>Ok(Self::Asciiletter),
("Chars",1)=>Ok(Self::Chars {text:read_text(&f[0],s,c,b)?}),
("Except",1)=>Ok(Self::Except {text:read_text(&f[0],s,c,b)?}),
("Range",2)=>Ok(Self::Range {lo:read_text(&f[0],s,c,b)?,hi:read_text(&f[1],s,c,b)?}),
("Visit",1)=>Ok(Self::Visit {child:read_name(&f[0],s,c,b)?}),
("Import",1)=>Ok(Self::Import {child:read_name(&f[0],s,c,b)?}),
("Propagate",1)=>Ok(Self::Propagate {child:read_name(&f[0],s,c,b)?}),
("Scope",1)=>Ok(Self::Scope {plans:Value::read(&f[0],s,c,b)?}),
("Group",1)=>Ok(Self::Group {plans:Value::read(&f[0],s,c,b)?}),
("Bind",2)=>Ok(Self::Bind {namespace:read_name(&f[0],s,c,b)?,field:read_name(&f[1],s,c,b)?}),
("Reference",2)=>Ok(Self::Reference {namespace:read_name(&f[0],s,c,b)?,field:read_name(&f[1],s,c,b)?}),
("Export",2)=>Ok(Self::Export {namespace:read_name(&f[0],s,c,b)?,field:read_name(&f[1],s,c,b)?}),
("Sequential",2)=>Ok(Self::Sequential {declarations:read_name(&f[0],s,c,b)?,body:read_name(&f[1],s,c,b)?}),
("Recursive",2)=>Ok(Self::Recursive {declarations:read_name(&f[0],s,c,b)?,body:read_name(&f[1],s,c,b)?}),
("None",0)=>Ok(Self::None),
("Custom",1)=>Ok(Self::Custom {provider:read_text(&f[0],s,c,b)?}),
("Style",2)=>Ok(Self::Style {selector:Value::read(&f[0],s,c,b)?,class:read_text(&f[1],s,c,b)?}),
("Selection",2)=>Ok(Self::Selection {selector:Value::read(&f[0],s,c,b)?,priority:read_nat(&f[1],s,c,b)?}),
("Head",0)=>Ok(Self::Head),
("Self",0)=>Ok(Self::SelfSelector),
("FieldSelector",1)=>Ok(Self::FieldSelector {name:read_name(&f[0],s,c,b)?}),
("CaptureSelector",1)=>Ok(Self::CaptureSelector {name:read_name(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
}
