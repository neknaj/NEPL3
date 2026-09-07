// Generated from interfaces/doc-html.json by tools/generate/doc_html.py. Do not edit.
use super::*;
#[rustfmt::skip]
mod adapters {
use super::*;
impl Value for ParallelMode {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Rows => variant(s,"ParallelMode","Rows",[],b),
Self::Columns => variant(s,"ParallelMode","Columns",[],b),
Self::Single {language,fallbacks} => variant(s,"ParallelMode","Single",[language.put(s,r,c,b)?,fallbacks.put(s,r,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let (tag,f)=case(v,s,"ParallelMode")?;
match (tag,f.len()) {
("Rows",0)=>Ok(Self::Rows),
("Columns",0)=>Ok(Self::Columns),
("Single",2)=>Ok(Self::Single {language:Value::read(&f[0],s,r,c,b)?,fallbacks:Value::read(&f[1],s,r,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for RenderOptions {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"RenderOptions",[self.parallel.put(s,r,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let f=fields(v,s,"RenderOptions",1)?;
Ok(Self {parallel:Value::read(&f[0],s,r,c,b)?})
}
}
impl Value for LocalHtmlRequest {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"LocalHtmlRequest",[self.document.put(s,r,c,b)?,self.options.put(s,r,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let f=fields(v,s,"LocalHtmlRequest",2)?;
Ok(Self {document:Value::read(&f[0],s,r,c,b)?,options:Value::read(&f[1],s,r,c,b)?})
}
}
impl Value for ElementOrigin {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"ElementOrigin",[self.element.put(s,r,c,b)?,self.node.put(s,r,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let f=fields(v,s,"ElementOrigin",2)?;
Ok(Self {element:Value::read(&f[0],s,r,c,b)?,node:Value::read(&f[1],s,r,c,b)?})
}
}
impl Value for RenderedFragment {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"RenderedFragment",[self.document_digest.put(s,r,c,b)?,self.options.put(s,r,c,b)?,self.markup.put(s,r,c,b)?,self.origins.put(s,r,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let f=fields(v,s,"RenderedFragment",4)?;
Ok(Self {document_digest:Value::read(&f[0],s,r,c,b)?,options:Value::read(&f[1],s,r,c,b)?,markup:Value::read(&f[2],s,r,c,b)?,origins:Value::read(&f[3],s,r,c,b)?})
}
}
impl Value for PagesHtmlRequest {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PagesHtmlRequest",[self.set.put(s,r,c,b)?,self.options.put(s,r,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let f=fields(v,s,"PagesHtmlRequest",2)?;
Ok(Self {set:Value::read(&f[0],s,r,c,b)?,options:Value::read(&f[1],s,r,c,b)?})
}
}
impl Value for RenderedPages {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"RenderedPages",[self.identity.put(s,r,c,b)?,self.fragments.put(s,r,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,r:&SchemaRegistry,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let f=fields(v,s,"RenderedPages",2)?;
Ok(Self {identity:Value::read(&f[0],s,r,c,b)?,fragments:Value::read(&f[1],s,r,c,b)?})
}
}
}
