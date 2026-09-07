// Generated from interfaces/doc.json by tools/generate/doc.py. Do not edit.
use super::*;
#[rustfmt::skip]
mod adapters {
use super::*;
impl Value for ArticleRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"ArticleRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,42)?;
let f=fields(v,s,"ArticleRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for BodyRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"BodyRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let f=fields(v,s,"BodyRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for BlockRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"BlockRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"BlockRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for FlowRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"FlowRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let f=fields(v,s,"FlowRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for SentenceRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"SentenceRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,43)?;
let f=fields(v,s,"SentenceRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for InlineRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"InlineRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let f=fields(v,s,"InlineRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for VariantRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"VariantRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,42)?;
let f=fields(v,s,"VariantRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for RowRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"RowRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,38)?;
let f=fields(v,s,"RowRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for ListItemRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"ListItemRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,43)?;
let f=fields(v,s,"ListItemRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for EmbedRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"EmbedRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"EmbedRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for DocRoot {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Article(root) => variant(s,"DocRoot","Article",[root.put(s,c,b)?],b),
Self::Body(root) => variant(s,"DocRoot","Body",[root.put(s,c,b)?],b),
Self::Block(root) => variant(s,"DocRoot","Block",[root.put(s,c,b)?],b),
Self::Flow(root) => variant(s,"DocRoot","Flow",[root.put(s,c,b)?],b),
Self::Sentence(root) => variant(s,"DocRoot","Sentence",[root.put(s,c,b)?],b),
Self::Inline(root) => variant(s,"DocRoot","Inline",[root.put(s,c,b)?],b),
Self::Variant(root) => variant(s,"DocRoot","Variant",[root.put(s,c,b)?],b),
Self::Row(root) => variant(s,"DocRoot","Row",[root.put(s,c,b)?],b),
Self::ListItem(root) => variant(s,"DocRoot","ListItem",[root.put(s,c,b)?],b),
Self::Alignment(root) => variant(s,"DocRoot","Alignment",[root.put(s,c,b)?],b),
Self::ListStyle(root) => variant(s,"DocRoot","ListStyle",[root.put(s,c,b)?],b),
Self::Check(root) => variant(s,"DocRoot","Check",[root.put(s,c,b)?],b),
Self::Target(root) => variant(s,"DocRoot","Target",[root.put(s,c,b)?],b),
Self::Asset(root) => variant(s,"DocRoot","Asset",[root.put(s,c,b)?],b),
Self::OptionalRow(root) => variant(s,"DocRoot","OptionalRow",[root.put(s,c,b)?],b),
Self::OptionalSentence(root) => variant(s,"DocRoot","OptionalSentence",[root.put(s,c,b)?],b),
Self::OptionalText(root) => variant(s,"DocRoot","OptionalText",[root.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let (tag,f)=case(v,s,"DocRoot")?;
match (tag,f.len()) {
("Article",1)=>Ok(Self::Article(Value::read(&f[0],s,c,b)?)),
("Body",1)=>Ok(Self::Body(Value::read(&f[0],s,c,b)?)),
("Block",1)=>Ok(Self::Block(Value::read(&f[0],s,c,b)?)),
("Flow",1)=>Ok(Self::Flow(Value::read(&f[0],s,c,b)?)),
("Sentence",1)=>Ok(Self::Sentence(Value::read(&f[0],s,c,b)?)),
("Inline",1)=>Ok(Self::Inline(Value::read(&f[0],s,c,b)?)),
("Variant",1)=>Ok(Self::Variant(Value::read(&f[0],s,c,b)?)),
("Row",1)=>Ok(Self::Row(Value::read(&f[0],s,c,b)?)),
("ListItem",1)=>Ok(Self::ListItem(Value::read(&f[0],s,c,b)?)),
("Alignment",1)=>Ok(Self::Alignment(Value::read(&f[0],s,c,b)?)),
("ListStyle",1)=>Ok(Self::ListStyle(Value::read(&f[0],s,c,b)?)),
("Check",1)=>Ok(Self::Check(Value::read(&f[0],s,c,b)?)),
("Target",1)=>Ok(Self::Target(Value::read(&f[0],s,c,b)?)),
("Asset",1)=>Ok(Self::Asset(Value::read(&f[0],s,c,b)?)),
("OptionalRow",1)=>Ok(Self::OptionalRow(Value::read(&f[0],s,c,b)?)),
("OptionalSentence",1)=>Ok(Self::OptionalSentence(Value::read(&f[0],s,c,b)?)),
("OptionalText",1)=>Ok(Self::OptionalText(Value::read(&f[0],s,c,b)?)),
_=>Err(PortableError::Shape),}
}
}
impl Value for AssetRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"AssetRef",[self.id.put(s,c,b)?,self.digest.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"AssetRef",2)?;
Ok(Self {id:Value::read(&f[0],s,c,b)?,digest:Value::read(&f[1],s,c,b)?})
}
}
impl Value for LinkTarget {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Page {page,fragment} => variant(s,"LinkTarget","Page",[page.put(s,c,b)?,fragment.put(s,c,b)?],b),
Self::Relative {path,fragment} => variant(s,"LinkTarget","Relative",[path.put(s,c,b)?,fragment.put(s,c,b)?],b),
Self::External {uri} => variant(s,"LinkTarget","External",[uri.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,42)?;
let (tag,f)=case(v,s,"LinkTarget")?;
match (tag,f.len()) {
("Page",2)=>Ok(Self::Page {page:Value::read(&f[0],s,c,b)?,fragment:Value::read(&f[1],s,c,b)?}),
("Relative",2)=>Ok(Self::Relative {path:Value::read(&f[0],s,c,b)?,fragment:Value::read(&f[1],s,c,b)?}),
("External",1)=>Ok(Self::External {uri:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for Alignment {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Default => variant(s,"Alignment","Default",[],b),
Self::Left => variant(s,"Alignment","Left",[],b),
Self::Center => variant(s,"Alignment","Center",[],b),
Self::Right => variant(s,"Alignment","Right",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let (tag,f)=case(v,s,"Alignment")?;
match (tag,f.len()) {
("Default",0)=>Ok(Self::Default),
("Left",0)=>Ok(Self::Left),
("Center",0)=>Ok(Self::Center),
("Right",0)=>Ok(Self::Right),
_=>Err(PortableError::Shape),}
}
}
impl Value for ListKind {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Unordered => variant(s,"ListKind","Unordered",[],b),
Self::Ordered {start} => variant(s,"ListKind","Ordered",[start.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"ListKind")?;
match (tag,f.len()) {
("Unordered",0)=>Ok(Self::Unordered),
("Ordered",1)=>Ok(Self::Ordered {start:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for EmbedKind {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::InlineMath => variant(s,"EmbedKind","InlineMath",[],b),
Self::DisplayMath => variant(s,"EmbedKind","DisplayMath",[],b),
Self::CircuitFigure => variant(s,"EmbedKind","CircuitFigure",[],b),
Self::Code => variant(s,"EmbedKind","Code",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let (tag,f)=case(v,s,"EmbedKind")?;
match (tag,f.len()) {
("InlineMath",0)=>Ok(Self::InlineMath),
("DisplayMath",0)=>Ok(Self::DisplayMath),
("CircuitFigure",0)=>Ok(Self::CircuitFigure),
("Code",0)=>Ok(Self::Code),
_=>Err(PortableError::Shape),}
}
}
impl Value for DocKind {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Article {language,title,body} => variant(s,"DocKind","Article",[language.put(s,c,b)?,title.put(s,c,b)?,body.put(s,c,b)?],b),
Self::Body {blocks} => variant(s,"DocKind","Body",[blocks.put(s,c,b)?],b),
Self::Paragraph {items} => variant(s,"DocKind","Paragraph",[items.put(s,c,b)?],b),
Self::Section {id,title,body} => variant(s,"DocKind","Section",[id.put(s,c,b)?,title.put(s,c,b)?,body.put(s,c,b)?],b),
Self::Sentence {inlines} => variant(s,"DocKind","Sentence",[inlines.put(s,c,b)?],b),
Self::Parallel {variants} => variant(s,"DocKind","Parallel",[variants.put(s,c,b)?],b),
Self::Variant {language,sentence} => variant(s,"DocKind","Variant",[language.put(s,c,b)?,sentence.put(s,c,b)?],b),
Self::Text {text} => variant(s,"DocKind","Text",[text.put(s,c,b)?],b),
Self::Concat {inlines} => variant(s,"DocKind","Concat",[inlines.put(s,c,b)?],b),
Self::Ruby {base,reading} => variant(s,"DocKind","Ruby",[base.put(s,c,b)?,reading.put(s,c,b)?],b),
Self::Anno {base,notes} => variant(s,"DocKind","Anno",[base.put(s,c,b)?,notes.put(s,c,b)?],b),
Self::InlineMath {syntax} => variant(s,"DocKind","InlineMath",[syntax.put(s,c,b)?],b),
Self::Anchor {id,label} => variant(s,"DocKind","Anchor",[id.put(s,c,b)?,label.put(s,c,b)?],b),
Self::Reference {target,label} => variant(s,"DocKind","Reference",[target.put(s,c,b)?,label.put(s,c,b)?],b),
Self::Emphasis {inline} => variant(s,"DocKind","Emphasis",[inline.put(s,c,b)?],b),
Self::Strong {inline} => variant(s,"DocKind","Strong",[inline.put(s,c,b)?],b),
Self::Break => variant(s,"DocKind","Break",[],b),
Self::DisplayMath {syntax} => variant(s,"DocKind","DisplayMath",[syntax.put(s,c,b)?],b),
Self::CircuitFigure {caption,syntax} => variant(s,"DocKind","CircuitFigure",[caption.put(s,c,b)?,syntax.put(s,c,b)?],b),
Self::Code {syntax} => variant(s,"DocKind","Code",[syntax.put(s,c,b)?],b),
Self::Table {columns,header,rows} => variant(s,"DocKind","Table",[columns.put(s,c,b)?,header.put(s,c,b)?,rows.put(s,c,b)?],b),
Self::Row {cells} => variant(s,"DocKind","Row",[cells.put(s,c,b)?],b),
Self::List {kind,items} => variant(s,"DocKind","List",[kind.put(s,c,b)?,items.put(s,c,b)?],b),
Self::ListItem {checked,body} => variant(s,"DocKind","ListItem",[checked.put(s,c,b)?,body.put(s,c,b)?],b),
Self::Link {target,label} => variant(s,"DocKind","Link",[target.put(s,c,b)?,label.put(s,c,b)?],b),
Self::InlineCode {text} => variant(s,"DocKind","InlineCode",[text.put(s,c,b)?],b),
Self::RawCode {language_hint,text} => variant(s,"DocKind","RawCode",[language_hint.put(s,c,b)?,text.put(s,c,b)?],b),
Self::Image {asset,alt,caption} => variant(s,"DocKind","Image",[asset.put(s,c,b)?,alt.put(s,c,b)?,caption.put(s,c,b)?],b),
Self::InlineImage {asset,alt} => variant(s,"DocKind","InlineImage",[asset.put(s,c,b)?,alt.put(s,c,b)?],b),
Self::Alignment {alignment} => variant(s,"DocKind","Alignment",[alignment.put(s,c,b)?],b),
Self::ListStyle {style} => variant(s,"DocKind","ListStyle",[style.put(s,c,b)?],b),
Self::Check {checked} => variant(s,"DocKind","Check",[checked.put(s,c,b)?],b),
Self::Target {target} => variant(s,"DocKind","Target",[target.put(s,c,b)?],b),
Self::Asset {asset} => variant(s,"DocKind","Asset",[asset.put(s,c,b)?],b),
Self::OptionalRow {row} => variant(s,"DocKind","OptionalRow",[row.put(s,c,b)?],b),
Self::OptionalSentence {sentence} => variant(s,"DocKind","OptionalSentence",[sentence.put(s,c,b)?],b),
Self::OptionalText {text} => variant(s,"DocKind","OptionalText",[text.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let (tag,f)=case(v,s,"DocKind")?;
match (tag,f.len()) {
("Article",3)=>Ok(Self::Article {language:Value::read(&f[0],s,c,b)?,title:Value::read(&f[1],s,c,b)?,body:Value::read(&f[2],s,c,b)?}),
("Body",1)=>Ok(Self::Body {blocks:Value::read(&f[0],s,c,b)?}),
("Paragraph",1)=>Ok(Self::Paragraph {items:Value::read(&f[0],s,c,b)?}),
("Section",3)=>Ok(Self::Section {id:Value::read(&f[0],s,c,b)?,title:Value::read(&f[1],s,c,b)?,body:Value::read(&f[2],s,c,b)?}),
("Sentence",1)=>Ok(Self::Sentence {inlines:Value::read(&f[0],s,c,b)?}),
("Parallel",1)=>Ok(Self::Parallel {variants:Value::read(&f[0],s,c,b)?}),
("Variant",2)=>Ok(Self::Variant {language:Value::read(&f[0],s,c,b)?,sentence:Value::read(&f[1],s,c,b)?}),
("Text",1)=>Ok(Self::Text {text:Value::read(&f[0],s,c,b)?}),
("Concat",1)=>Ok(Self::Concat {inlines:Value::read(&f[0],s,c,b)?}),
("Ruby",2)=>Ok(Self::Ruby {base:Value::read(&f[0],s,c,b)?,reading:Value::read(&f[1],s,c,b)?}),
("Anno",2)=>Ok(Self::Anno {base:Value::read(&f[0],s,c,b)?,notes:Value::read(&f[1],s,c,b)?}),
("InlineMath",1)=>Ok(Self::InlineMath {syntax:Value::read(&f[0],s,c,b)?}),
("Anchor",2)=>Ok(Self::Anchor {id:Value::read(&f[0],s,c,b)?,label:Value::read(&f[1],s,c,b)?}),
("Reference",2)=>Ok(Self::Reference {target:Value::read(&f[0],s,c,b)?,label:Value::read(&f[1],s,c,b)?}),
("Emphasis",1)=>Ok(Self::Emphasis {inline:Value::read(&f[0],s,c,b)?}),
("Strong",1)=>Ok(Self::Strong {inline:Value::read(&f[0],s,c,b)?}),
("Break",0)=>Ok(Self::Break),
("DisplayMath",1)=>Ok(Self::DisplayMath {syntax:Value::read(&f[0],s,c,b)?}),
("CircuitFigure",2)=>Ok(Self::CircuitFigure {caption:Value::read(&f[0],s,c,b)?,syntax:Value::read(&f[1],s,c,b)?}),
("Code",1)=>Ok(Self::Code {syntax:Value::read(&f[0],s,c,b)?}),
("Table",3)=>Ok(Self::Table {columns:Value::read(&f[0],s,c,b)?,header:Value::read(&f[1],s,c,b)?,rows:Value::read(&f[2],s,c,b)?}),
("Row",1)=>Ok(Self::Row {cells:Value::read(&f[0],s,c,b)?}),
("List",2)=>Ok(Self::List {kind:Value::read(&f[0],s,c,b)?,items:Value::read(&f[1],s,c,b)?}),
("ListItem",2)=>Ok(Self::ListItem {checked:Value::read(&f[0],s,c,b)?,body:Value::read(&f[1],s,c,b)?}),
("Link",2)=>Ok(Self::Link {target:Value::read(&f[0],s,c,b)?,label:Value::read(&f[1],s,c,b)?}),
("InlineCode",1)=>Ok(Self::InlineCode {text:Value::read(&f[0],s,c,b)?}),
("RawCode",2)=>Ok(Self::RawCode {language_hint:Value::read(&f[0],s,c,b)?,text:Value::read(&f[1],s,c,b)?}),
("Image",3)=>Ok(Self::Image {asset:Value::read(&f[0],s,c,b)?,alt:Value::read(&f[1],s,c,b)?,caption:Value::read(&f[2],s,c,b)?}),
("InlineImage",2)=>Ok(Self::InlineImage {asset:Value::read(&f[0],s,c,b)?,alt:Value::read(&f[1],s,c,b)?}),
("Alignment",1)=>Ok(Self::Alignment {alignment:Value::read(&f[0],s,c,b)?}),
("ListStyle",1)=>Ok(Self::ListStyle {style:Value::read(&f[0],s,c,b)?}),
("Check",1)=>Ok(Self::Check {checked:Value::read(&f[0],s,c,b)?}),
("Target",1)=>Ok(Self::Target {target:Value::read(&f[0],s,c,b)?}),
("Asset",1)=>Ok(Self::Asset {asset:Value::read(&f[0],s,c,b)?}),
("OptionalRow",1)=>Ok(Self::OptionalRow {row:Value::read(&f[0],s,c,b)?}),
("OptionalSentence",1)=>Ok(Self::OptionalSentence {sentence:Value::read(&f[0],s,c,b)?}),
("OptionalText",1)=>Ok(Self::OptionalText {text:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for DocNode {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocNode",[self.kind.put(s,c,b)?,self.origin.put(s,c,b)?,self.span.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let f=fields(v,s,"DocNode",3)?;
Ok(Self {kind:Value::read(&f[0],s,c,b)?,origin:Value::read(&f[1],s,c,b)?,span:Value::read(&f[2],s,c,b)?})
}
}
impl Value for DocEmbed {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocEmbed",[self.kind.put(s,c,b)?,self.closure.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"DocEmbed",2)?;
Ok(Self {kind:Value::read(&f[0],s,c,b)?,closure:Value::read(&f[1],s,c,b)?})
}
}
impl Value for DocValue {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocValue",[self.root.put(s,c,b)?,self.nodes.put(s,c,b)?,self.embeds.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"DocValue",3)?;
Ok(Self {root:Value::read(&f[0],s,c,b)?,nodes:Value::read(&f[1],s,c,b)?,embeds:Value::read(&f[2],s,c,b)?})
}
}
impl Value for DocView {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocView",[self.head.put(s,c,b)?,self.view.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let f=fields(v,s,"DocView",2)?;
Ok(Self {head:Value::read(&f[0],s,c,b)?,view:Value::read(&f[1],s,c,b)?})
}
}
impl Value for AlignmentRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"AlignmentRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"AlignmentRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for ListStyleRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"ListStyleRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"ListStyleRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for CheckRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"CheckRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"CheckRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for TargetRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"TargetRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let f=fields(v,s,"TargetRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for AssetValueRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"AssetValueRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let f=fields(v,s,"AssetValueRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for OptionalRowRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"OptionalRowRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,46)?;
let f=fields(v,s,"OptionalRowRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for OptionalSentenceRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"OptionalSentenceRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,51)?;
let f=fields(v,s,"OptionalSentenceRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for OptionalTextRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"OptionalTextRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,47)?;
let f=fields(v,s,"OptionalTextRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
}
