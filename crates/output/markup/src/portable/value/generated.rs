// Generated from interfaces/markup.json by tools/generate/markup.py. Do not edit.
use super::*;
#[rustfmt::skip]
mod adapters {
use super::*;
impl Value for HtmlTag {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Article => variant(s,"HtmlTag","Article",[],b),
Self::Section => variant(s,"HtmlTag","Section",[],b),
Self::Div => variant(s,"HtmlTag","Div",[],b),
Self::P => variant(s,"HtmlTag","P",[],b),
Self::Span => variant(s,"HtmlTag","Span",[],b),
Self::H1 => variant(s,"HtmlTag","H1",[],b),
Self::H2 => variant(s,"HtmlTag","H2",[],b),
Self::H3 => variant(s,"HtmlTag","H3",[],b),
Self::H4 => variant(s,"HtmlTag","H4",[],b),
Self::H5 => variant(s,"HtmlTag","H5",[],b),
Self::H6 => variant(s,"HtmlTag","H6",[],b),
Self::Ruby => variant(s,"HtmlTag","Ruby",[],b),
Self::Rt => variant(s,"HtmlTag","Rt",[],b),
Self::Rp => variant(s,"HtmlTag","Rp",[],b),
Self::Em => variant(s,"HtmlTag","Em",[],b),
Self::Strong => variant(s,"HtmlTag","Strong",[],b),
Self::Br => variant(s,"HtmlTag","Br",[],b),
Self::Pre => variant(s,"HtmlTag","Pre",[],b),
Self::Code => variant(s,"HtmlTag","Code",[],b),
Self::Figure => variant(s,"HtmlTag","Figure",[],b),
Self::Figcaption => variant(s,"HtmlTag","Figcaption",[],b),
Self::A => variant(s,"HtmlTag","A",[],b),
Self::Ul => variant(s,"HtmlTag","Ul",[],b),
Self::Ol => variant(s,"HtmlTag","Ol",[],b),
Self::Li => variant(s,"HtmlTag","Li",[],b),
Self::Table => variant(s,"HtmlTag","Table",[],b),
Self::Caption => variant(s,"HtmlTag","Caption",[],b),
Self::Thead => variant(s,"HtmlTag","Thead",[],b),
Self::Tbody => variant(s,"HtmlTag","Tbody",[],b),
Self::Tr => variant(s,"HtmlTag","Tr",[],b),
Self::Th => variant(s,"HtmlTag","Th",[],b),
Self::Td => variant(s,"HtmlTag","Td",[],b),
Self::Img => variant(s,"HtmlTag","Img",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let (tag,f)=case(v,s,"HtmlTag")?;
match (tag,f.len()) {
("Article",0)=>Ok(Self::Article),
("Section",0)=>Ok(Self::Section),
("Div",0)=>Ok(Self::Div),
("P",0)=>Ok(Self::P),
("Span",0)=>Ok(Self::Span),
("H1",0)=>Ok(Self::H1),
("H2",0)=>Ok(Self::H2),
("H3",0)=>Ok(Self::H3),
("H4",0)=>Ok(Self::H4),
("H5",0)=>Ok(Self::H5),
("H6",0)=>Ok(Self::H6),
("Ruby",0)=>Ok(Self::Ruby),
("Rt",0)=>Ok(Self::Rt),
("Rp",0)=>Ok(Self::Rp),
("Em",0)=>Ok(Self::Em),
("Strong",0)=>Ok(Self::Strong),
("Br",0)=>Ok(Self::Br),
("Pre",0)=>Ok(Self::Pre),
("Code",0)=>Ok(Self::Code),
("Figure",0)=>Ok(Self::Figure),
("Figcaption",0)=>Ok(Self::Figcaption),
("A",0)=>Ok(Self::A),
("Ul",0)=>Ok(Self::Ul),
("Ol",0)=>Ok(Self::Ol),
("Li",0)=>Ok(Self::Li),
("Table",0)=>Ok(Self::Table),
("Caption",0)=>Ok(Self::Caption),
("Thead",0)=>Ok(Self::Thead),
("Tbody",0)=>Ok(Self::Tbody),
("Tr",0)=>Ok(Self::Tr),
("Th",0)=>Ok(Self::Th),
("Td",0)=>Ok(Self::Td),
("Img",0)=>Ok(Self::Img),
_=>Err(PortableError::Shape),}
}
}
impl Value for HtmlSlot {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Block => variant(s,"HtmlSlot","Block",[],b),
Self::Phrasing => variant(s,"HtmlSlot","Phrasing",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"HtmlSlot")?;
match (tag,f.len()) {
("Block",0)=>Ok(Self::Block),
("Phrasing",0)=>Ok(Self::Phrasing),
_=>Err(PortableError::Shape),}
}
}
impl Value for HtmlRole {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Heading => variant(s,"HtmlRole","Heading",[],b),
Self::Img => variant(s,"HtmlRole","Img",[],b),
Self::Group => variant(s,"HtmlRole","Group",[],b),
Self::Note => variant(s,"HtmlRole","Note",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"HtmlRole")?;
match (tag,f.len()) {
("Heading",0)=>Ok(Self::Heading),
("Img",0)=>Ok(Self::Img),
("Group",0)=>Ok(Self::Group),
("Note",0)=>Ok(Self::Note),
_=>Err(PortableError::Shape),}
}
}
impl Value for CellScope {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Row => variant(s,"CellScope","Row",[],b),
Self::Col => variant(s,"CellScope","Col",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let (tag,f)=case(v,s,"CellScope")?;
match (tag,f.len()) {
("Row",0)=>Ok(Self::Row),
("Col",0)=>Ok(Self::Col),
_=>Err(PortableError::Shape),}
}
}
impl Value for TextContext {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Content => variant(s,"TextContext","Content",[],b),
Self::Attribute => variant(s,"TextContext","Attribute",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,43)?;
let (tag,f)=case(v,s,"TextContext")?;
match (tag,f.len()) {
("Content",0)=>Ok(Self::Content),
("Attribute",0)=>Ok(Self::Attribute),
_=>Err(PortableError::Shape),}
}
}
impl Value for HtmlHref {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Fragment {id} => variant(s,"HtmlHref","Fragment",[id.put(s,c,b)?],b),
Self::Artifact {path,fragment} => variant(s,"HtmlHref","Artifact",[path.put(s,c,b)?,fragment.put(s,c,b)?],b),
Self::External {uri} => variant(s,"HtmlHref","External",[uri.put(s,c,b)?],b),
Self::BetweenArtifacts {source,target,fragment} => variant(s,"HtmlHref","BetweenArtifacts",[source.put(s,c,b)?,target.put(s,c,b)?,fragment.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"HtmlHref")?;
match (tag,f.len()) {
("Fragment",1)=>Ok(Self::Fragment {id:Value::read(&f[0],s,c,b)?}),
("Artifact",2)=>Ok(Self::Artifact {path:Value::read(&f[0],s,c,b)?,fragment:Value::read(&f[1],s,c,b)?}),
("External",1)=>Ok(Self::External {uri:Value::read(&f[0],s,c,b)?}),
("BetweenArtifacts",3)=>Ok(Self::BetweenArtifacts {source:Value::read(&f[0],s,c,b)?,target:Value::read(&f[1],s,c,b)?,fragment:Value::read(&f[2],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for HtmlAttribute {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Id {value} => variant(s,"HtmlAttribute","Id",[value.put(s,c,b)?],b),
Self::Lang {value} => variant(s,"HtmlAttribute","Lang",[value.put(s,c,b)?],b),
Self::AriaLabel {value} => variant(s,"HtmlAttribute","AriaLabel",[value.put(s,c,b)?],b),
Self::DataId {value} => variant(s,"HtmlAttribute","DataId",[value.put(s,c,b)?],b),
Self::DataGroup {value} => variant(s,"HtmlAttribute","DataGroup",[value.put(s,c,b)?],b),
Self::Alt {value} => variant(s,"HtmlAttribute","Alt",[value.put(s,c,b)?],b),
Self::Class {values} => variant(s,"HtmlAttribute","Class",[values.put(s,c,b)?],b),
Self::Role {value} => variant(s,"HtmlAttribute","Role",[value.put(s,c,b)?],b),
Self::Href {value} => variant(s,"HtmlAttribute","Href",[value.put(s,c,b)?],b),
Self::Src {path} => variant(s,"HtmlAttribute","Src",[path.put(s,c,b)?],b),
Self::AriaLevel {value} => variant(s,"HtmlAttribute","AriaLevel",[value.put(s,c,b)?],b),
Self::Width {value} => variant(s,"HtmlAttribute","Width",[value.put(s,c,b)?],b),
Self::Height {value} => variant(s,"HtmlAttribute","Height",[value.put(s,c,b)?],b),
Self::Start {value} => variant(s,"HtmlAttribute","Start",[value.put(s,c,b)?],b),
Self::Scope {value} => variant(s,"HtmlAttribute","Scope",[value.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let (tag,f)=case(v,s,"HtmlAttribute")?;
match (tag,f.len()) {
("Id",1)=>Ok(Self::Id {value:Value::read(&f[0],s,c,b)?}),
("Lang",1)=>Ok(Self::Lang {value:Value::read(&f[0],s,c,b)?}),
("AriaLabel",1)=>Ok(Self::AriaLabel {value:Value::read(&f[0],s,c,b)?}),
("DataId",1)=>Ok(Self::DataId {value:Value::read(&f[0],s,c,b)?}),
("DataGroup",1)=>Ok(Self::DataGroup {value:Value::read(&f[0],s,c,b)?}),
("Alt",1)=>Ok(Self::Alt {value:Value::read(&f[0],s,c,b)?}),
("Class",1)=>Ok(Self::Class {values:Value::read(&f[0],s,c,b)?}),
("Role",1)=>Ok(Self::Role {value:Value::read(&f[0],s,c,b)?}),
("Href",1)=>Ok(Self::Href {value:Value::read(&f[0],s,c,b)?}),
("Src",1)=>Ok(Self::Src {path:Value::read(&f[0],s,c,b)?}),
("AriaLevel",1)=>Ok(Self::AriaLevel {value:Value::read(&f[0],s,c,b)?}),
("Width",1)=>Ok(Self::Width {value:Value::read(&f[0],s,c,b)?}),
("Height",1)=>Ok(Self::Height {value:Value::read(&f[0],s,c,b)?}),
("Start",1)=>Ok(Self::Start {value:Value::read(&f[0],s,c,b)?}),
("Scope",1)=>Ok(Self::Scope {value:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for HtmlNode {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Text {text} => variant(s,"HtmlNode","Text",[text.put(s,c,b)?],b),
Self::Element {tag,attributes,children} => variant(s,"HtmlNode","Element",[tag.put(s,c,b)?,attributes.put(s,c,b)?,children.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"HtmlNode")?;
match (tag,f.len()) {
("Text",1)=>Ok(Self::Text {text:Value::read(&f[0],s,c,b)?}),
("Element",3)=>Ok(Self::Element {tag:Value::read(&f[0],s,c,b)?,attributes:Value::read(&f[1],s,c,b)?,children:Value::read(&f[2],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for HtmlFragment {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"HtmlFragment",[self.root.put(s,c,b)?,self.nodes.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"HtmlFragment",2)?;
Ok(Self {root:Value::read(&f[0],s,c,b)?,nodes:Value::read(&f[1],s,c,b)?})
}
}
impl Value for HtmlPolicy {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"HtmlPolicy",[self.classes.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,42)?;
let f=fields(v,s,"HtmlPolicy",1)?;
Ok(Self {classes:Value::read(&f[0],s,c,b)?})
}
}
impl Value for HtmlRequest {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"HtmlRequest",[self.fragment.put(s,c,b)?,self.slot.put(s,c,b)?,self.policy.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,43)?;
let f=fields(v,s,"HtmlRequest",3)?;
Ok(Self {fragment:Value::read(&f[0],s,c,b)?,slot:Value::read(&f[1],s,c,b)?,policy:Value::read(&f[2],s,c,b)?})
}
}
}
