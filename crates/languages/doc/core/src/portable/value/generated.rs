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
Self::Guest(root) => variant(s,"DocRoot","Guest",[root.put(s,c,b)?],b),
Self::MathGuest(root) => variant(s,"DocRoot","MathGuest",[root.put(s,c,b)?],b),
Self::CircuitGuest(root) => variant(s,"DocRoot","CircuitGuest",[root.put(s,c,b)?],b),
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
("Guest",1)=>Ok(Self::Guest(Value::read(&f[0],s,c,b)?)),
("MathGuest",1)=>Ok(Self::MathGuest(Value::read(&f[0],s,c,b)?)),
("CircuitGuest",1)=>Ok(Self::CircuitGuest(Value::read(&f[0],s,c,b)?)),
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
Self::Guest => variant(s,"EmbedKind","Guest",[],b),
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
("Guest",0)=>Ok(Self::Guest),
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
Self::Guest {language,syntax} => variant(s,"DocKind","Guest",[language.put(s,c,b)?,syntax.put(s,c,b)?],b),
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
("Guest",2)=>Ok(Self::Guest {language:Value::read(&f[0],s,c,b)?,syntax:Value::read(&f[1],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for DocNode {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocNode",[self.kind.put(s,c,b)?,self.origin.put(s,c,b)?,self.span.put(s,c,b)?,self.locations.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let f=fields(v,s,"DocNode",4)?;
Ok(Self {kind:Value::read(&f[0],s,c,b)?,origin:Value::read(&f[1],s,c,b)?,span:Value::read(&f[2],s,c,b)?,locations:Value::read(&f[3],s,c,b)?})
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
impl Value for DocField {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::SectionId => variant(s,"DocField","SectionId",[],b),
Self::AnchorId => variant(s,"DocField","AnchorId",[],b),
Self::ReferenceTarget => variant(s,"DocField","ReferenceTarget",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"DocField")?;
match (tag,f.len()) {
("SectionId",0)=>Ok(Self::SectionId),
("AnchorId",0)=>Ok(Self::AnchorId),
("ReferenceTarget",0)=>Ok(Self::ReferenceTarget),
_=>Err(PortableError::Shape),}
}
}
impl Value for DocFieldLocation {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocFieldLocation",[self.field.put(s,c,b)?,self.origin.put(s,c,b)?,self.span.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let f=fields(v,s,"DocFieldLocation",3)?;
Ok(Self {field:Value::read(&f[0],s,c,b)?,origin:Value::read(&f[1],s,c,b)?,span:Value::read(&f[2],s,c,b)?})
}
}
impl Value for DocPathStep {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocPathStep",[self.owner.put(s,c,b)?,self.child.put(s,c,b)?,self.target.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,43)?;
let f=fields(v,s,"DocPathStep",3)?;
Ok(Self {owner:Value::read(&f[0],s,c,b)?,child:Value::read(&f[1],s,c,b)?,target:Value::read(&f[2],s,c,b)?})
}
}
impl Value for LabelOccurrencePaths {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"LabelOccurrencePaths",[self.first.put(s,c,b)?,self.second.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,52)?;
let f=fields(v,s,"LabelOccurrencePaths",2)?;
Ok(Self {first:Value::read(&f[0],s,c,b)?,second:Value::read(&f[1],s,c,b)?})
}
}
impl Value for LabelDiagnosticArguments {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"LabelDiagnosticArguments",[self.name.put(s,c,b)?,self.paths.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,56)?;
let f=fields(v,s,"LabelDiagnosticArguments",2)?;
Ok(Self {name:Value::read(&f[0],s,c,b)?,paths:Value::read(&f[1],s,c,b)?})
}
}
impl Value for AnnotationPolicy {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::BaseOnly => variant(s,"AnnotationPolicy","BaseOnly",[],b),
Self::WithReadings => variant(s,"AnnotationPolicy","WithReadings",[],b),
Self::WithAllNotes => variant(s,"AnnotationPolicy","WithAllNotes",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let (tag,f)=case(v,s,"AnnotationPolicy")?;
match (tag,f.len()) {
("BaseOnly",0)=>Ok(Self::BaseOnly),
("WithReadings",0)=>Ok(Self::WithReadings),
("WithAllNotes",0)=>Ok(Self::WithAllNotes),
_=>Err(PortableError::Shape),}
}
}
impl Value for InlineTextTarget {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"InlineTextTarget",[self.embed.put(s,c,b)?,self.guest_digest.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let f=fields(v,s,"InlineTextTarget",2)?;
Ok(Self {embed:Value::read(&f[0],s,c,b)?,guest_digest:Value::read(&f[1],s,c,b)?})
}
}
impl Value for TextIdentity {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"TextIdentity",[self.document_digest.put(s,c,b)?,self.embeds.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"TextIdentity",2)?;
Ok(Self {document_digest:Value::read(&f[0],s,c,b)?,embeds:Value::read(&f[1],s,c,b)?})
}
}
impl Value for ResolvedInlineText {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"ResolvedInlineText",[self.document_digest.put(s,c,b)?,self.embed.put(s,c,b)?,self.guest_digest.put(s,c,b)?,self.text.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,50)?;
let f=fields(v,s,"ResolvedInlineText",4)?;
Ok(Self {document_digest:Value::read(&f[0],s,c,b)?,embed:Value::read(&f[1],s,c,b)?,guest_digest:Value::read(&f[2],s,c,b)?,text:Value::read(&f[3],s,c,b)?})
}
}
impl Value for ResolutionMismatch {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Document => variant(s,"ResolutionMismatch","Document",[],b),
Self::Guest => variant(s,"ResolutionMismatch","Guest",[],b),
Self::Embed => variant(s,"ResolutionMismatch","Embed",[],b),
Self::Duplicate => variant(s,"ResolutionMismatch","Duplicate",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,50)?;
let (tag,f)=case(v,s,"ResolutionMismatch")?;
match (tag,f.len()) {
("Document",0)=>Ok(Self::Document),
("Guest",0)=>Ok(Self::Guest),
("Embed",0)=>Ok(Self::Embed),
("Duplicate",0)=>Ok(Self::Duplicate),
_=>Err(PortableError::Shape),}
}
}
impl Value for PlainTextFailure {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::ExpectedSentence {node} => variant(s,"PlainTextFailure","ExpectedSentence",[node.put(s,c,b)?],b),
Self::InvalidResolution {entry,reason} => variant(s,"PlainTextFailure","InvalidResolution",[entry.put(s,c,b)?,reason.put(s,c,b)?],b),
Self::UnresolvedEmbed {embed} => variant(s,"PlainTextFailure","UnresolvedEmbed",[embed.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let (tag,f)=case(v,s,"PlainTextFailure")?;
match (tag,f.len()) {
("ExpectedSentence",1)=>Ok(Self::ExpectedSentence {node:Value::read(&f[0],s,c,b)?}),
("InvalidResolution",2)=>Ok(Self::InvalidResolution {entry:Value::read(&f[0],s,c,b)?,reason:Value::read(&f[1],s,c,b)?}),
("UnresolvedEmbed",1)=>Ok(Self::UnresolvedEmbed {embed:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for PlainTextOutcome {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Complete {text} => variant(s,"PlainTextOutcome","Complete",[text.put(s,c,b)?],b),
Self::Invalid {error} => variant(s,"PlainTextOutcome","Invalid",[error.put(s,c,b)?],b),
Self::Stopped {reason} => variant(s,"PlainTextOutcome","Stopped",[reason.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let (tag,f)=case(v,s,"PlainTextOutcome")?;
match (tag,f.len()) {
("Complete",1)=>Ok(Self::Complete {text:Value::read(&f[0],s,c,b)?}),
("Invalid",1)=>Ok(Self::Invalid {error:Value::read(&f[0],s,c,b)?}),
("Stopped",1)=>Ok(Self::Stopped {reason:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for PlainTextReply {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PlainTextReply",[self.outcome.put(s,c,b)?,self.report.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,46)?;
let f=fields(v,s,"PlainTextReply",2)?;
Ok(Self {outcome:Value::read(&f[0],s,c,b)?,report:Value::read(&f[1],s,c,b)?})
}
}
impl Value for GuestRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"GuestRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"GuestRef",1)?;
Ok(Self(Value::read(&f[0],s,c,b)?))
}
}
impl Value for GuestLanguage {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Math => variant(s,"GuestLanguage","Math",[],b),
Self::Circuit => variant(s,"GuestLanguage","Circuit",[],b),
Self::Grammar => variant(s,"GuestLanguage","Grammar",[],b),
Self::Doc => variant(s,"GuestLanguage","Doc",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let (tag,f)=case(v,s,"GuestLanguage")?;
match (tag,f.len()) {
("Math",0)=>Ok(Self::Math),
("Circuit",0)=>Ok(Self::Circuit),
("Grammar",0)=>Ok(Self::Grammar),
("Doc",0)=>Ok(Self::Doc),
_=>Err(PortableError::Shape),}
}
}
impl Value for PrintMode {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Prefix => variant(s,"PrintMode","Prefix",[],b),
Self::Compact => variant(s,"PrintMode","Compact",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let (tag,f)=case(v,s,"PrintMode")?;
match (tag,f.len()) {
("Prefix",0)=>Ok(Self::Prefix),
("Compact",0)=>Ok(Self::Compact),
_=>Err(PortableError::Shape),}
}
}
impl Value for GuestBinding {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"GuestBinding",[self.schema.put(s,c,b)?,self.category.put(s,c,b)?,self.language.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"GuestBinding",3)?;
Ok(Self {schema:Value::read(&f[0],s,c,b)?,category:Value::read(&f[1],s,c,b)?,language:Value::read(&f[2],s,c,b)?})
}
}
impl Value for PrintedGuest {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PrintedGuest",[self.document_digest.put(s,c,b)?,self.embed.put(s,c,b)?,self.guest_digest.put(s,c,b)?,self.text.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"PrintedGuest",4)?;
Ok(Self {document_digest:Value::read(&f[0],s,c,b)?,embed:Value::read(&f[1],s,c,b)?,guest_digest:Value::read(&f[2],s,c,b)?,text:Value::read(&f[3],s,c,b)?})
}
}
impl Value for PrintGuestTarget {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PrintGuestTarget",[self.embed.put(s,c,b)?,self.guest_digest.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let f=fields(v,s,"PrintGuestTarget",2)?;
Ok(Self {embed:Value::read(&f[0],s,c,b)?,guest_digest:Value::read(&f[1],s,c,b)?})
}
}
impl Value for PrintIdentity {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PrintIdentity",[self.document_digest.put(s,c,b)?,self.guests.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let f=fields(v,s,"PrintIdentity",2)?;
Ok(Self {document_digest:Value::read(&f[0],s,c,b)?,guests:Value::read(&f[1],s,c,b)?})
}
}
impl Value for PrintEntry {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Article => variant(s,"PrintEntry","Article",[],b),
Self::Body => variant(s,"PrintEntry","Body",[],b),
Self::Block => variant(s,"PrintEntry","Block",[],b),
Self::Flow => variant(s,"PrintEntry","Flow",[],b),
Self::Sentence => variant(s,"PrintEntry","Sentence",[],b),
Self::Inline => variant(s,"PrintEntry","Inline",[],b),
Self::Variant => variant(s,"PrintEntry","Variant",[],b),
Self::Row => variant(s,"PrintEntry","Row",[],b),
Self::ListItem => variant(s,"PrintEntry","ListItem",[],b),
Self::Alignment => variant(s,"PrintEntry","Alignment",[],b),
Self::ListStyle => variant(s,"PrintEntry","ListStyle",[],b),
Self::Check => variant(s,"PrintEntry","Check",[],b),
Self::LinkTarget => variant(s,"PrintEntry","LinkTarget",[],b),
Self::Asset => variant(s,"PrintEntry","Asset",[],b),
Self::OptionalRow => variant(s,"PrintEntry","OptionalRow",[],b),
Self::OptionalSentence => variant(s,"PrintEntry","OptionalSentence",[],b),
Self::OptionalText => variant(s,"PrintEntry","OptionalText",[],b),
Self::MathGuest => variant(s,"PrintEntry","MathGuest",[],b),
Self::CircuitGuest => variant(s,"PrintEntry","CircuitGuest",[],b),
Self::Guest => variant(s,"PrintEntry","Guest",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,42)?;
let (tag,f)=case(v,s,"PrintEntry")?;
match (tag,f.len()) {
("Article",0)=>Ok(Self::Article),
("Body",0)=>Ok(Self::Body),
("Block",0)=>Ok(Self::Block),
("Flow",0)=>Ok(Self::Flow),
("Sentence",0)=>Ok(Self::Sentence),
("Inline",0)=>Ok(Self::Inline),
("Variant",0)=>Ok(Self::Variant),
("Row",0)=>Ok(Self::Row),
("ListItem",0)=>Ok(Self::ListItem),
("Alignment",0)=>Ok(Self::Alignment),
("ListStyle",0)=>Ok(Self::ListStyle),
("Check",0)=>Ok(Self::Check),
("LinkTarget",0)=>Ok(Self::LinkTarget),
("Asset",0)=>Ok(Self::Asset),
("OptionalRow",0)=>Ok(Self::OptionalRow),
("OptionalSentence",0)=>Ok(Self::OptionalSentence),
("OptionalText",0)=>Ok(Self::OptionalText),
("MathGuest",0)=>Ok(Self::MathGuest),
("CircuitGuest",0)=>Ok(Self::CircuitGuest),
("Guest",0)=>Ok(Self::Guest),
_=>Err(PortableError::Shape),}
}
}
impl Value for SourceArtifact {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"SourceArtifact",[self.text.put(s,c,b)?,self.entry.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,46)?;
let f=fields(v,s,"SourceArtifact",2)?;
Ok(Self {text:Value::read(&f[0],s,c,b)?,entry:Value::read(&f[1],s,c,b)?})
}
}
impl Value for PrintMismatch {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Document => variant(s,"PrintMismatch","Document",[],b),
Self::Guest => variant(s,"PrintMismatch","Guest",[],b),
Self::Embed => variant(s,"PrintMismatch","Embed",[],b),
Self::Duplicate => variant(s,"PrintMismatch","Duplicate",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let (tag,f)=case(v,s,"PrintMismatch")?;
match (tag,f.len()) {
("Document",0)=>Ok(Self::Document),
("Guest",0)=>Ok(Self::Guest),
("Embed",0)=>Ok(Self::Embed),
("Duplicate",0)=>Ok(Self::Duplicate),
_=>Err(PortableError::Shape),}
}
}
impl Value for PrintFailure {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::InvalidBinding {binding} => variant(s,"PrintFailure","InvalidBinding",[binding.put(s,c,b)?],b),
Self::ConflictingBinding {binding} => variant(s,"PrintFailure","ConflictingBinding",[binding.put(s,c,b)?],b),
Self::MissingBinding {embed} => variant(s,"PrintFailure","MissingBinding",[embed.put(s,c,b)?],b),
Self::GuestCategory {embed} => variant(s,"PrintFailure","GuestCategory",[embed.put(s,c,b)?],b),
Self::InvalidGuest {entry,reason} => variant(s,"PrintFailure","InvalidGuest",[entry.put(s,c,b)?,reason.put(s,c,b)?],b),
Self::UnresolvedGuest {embed} => variant(s,"PrintFailure","UnresolvedGuest",[embed.put(s,c,b)?],b),
Self::UnprintableName {node,field} => variant(s,"PrintFailure","UnprintableName",[node.put(s,c,b)?,field.put(s,c,b)?],b),
Self::UnprintableLanguage {node} => variant(s,"PrintFailure","UnprintableLanguage",[node.put(s,c,b)?],b),
Self::UnprintableLiteral {node} => variant(s,"PrintFailure","UnprintableLiteral",[node.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let (tag,f)=case(v,s,"PrintFailure")?;
match (tag,f.len()) {
("InvalidBinding",1)=>Ok(Self::InvalidBinding {binding:Value::read(&f[0],s,c,b)?}),
("ConflictingBinding",1)=>Ok(Self::ConflictingBinding {binding:Value::read(&f[0],s,c,b)?}),
("MissingBinding",1)=>Ok(Self::MissingBinding {embed:Value::read(&f[0],s,c,b)?}),
("GuestCategory",1)=>Ok(Self::GuestCategory {embed:Value::read(&f[0],s,c,b)?}),
("InvalidGuest",2)=>Ok(Self::InvalidGuest {entry:Value::read(&f[0],s,c,b)?,reason:Value::read(&f[1],s,c,b)?}),
("UnresolvedGuest",1)=>Ok(Self::UnresolvedGuest {embed:Value::read(&f[0],s,c,b)?}),
("UnprintableName",2)=>Ok(Self::UnprintableName {node:Value::read(&f[0],s,c,b)?,field:Value::read(&f[1],s,c,b)?}),
("UnprintableLanguage",1)=>Ok(Self::UnprintableLanguage {node:Value::read(&f[0],s,c,b)?}),
("UnprintableLiteral",1)=>Ok(Self::UnprintableLiteral {node:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for PrintOutcome {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Complete {artifact} => variant(s,"PrintOutcome","Complete",[artifact.put(s,c,b)?],b),
Self::Invalid {error} => variant(s,"PrintOutcome","Invalid",[error.put(s,c,b)?],b),
Self::Stopped {reason} => variant(s,"PrintOutcome","Stopped",[reason.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let (tag,f)=case(v,s,"PrintOutcome")?;
match (tag,f.len()) {
("Complete",1)=>Ok(Self::Complete {artifact:Value::read(&f[0],s,c,b)?}),
("Invalid",1)=>Ok(Self::Invalid {error:Value::read(&f[0],s,c,b)?}),
("Stopped",1)=>Ok(Self::Stopped {reason:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for PrintReply {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PrintReply",[self.outcome.put(s,c,b)?,self.report.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,42)?;
let f=fields(v,s,"PrintReply",2)?;
Ok(Self {outcome:Value::read(&f[0],s,c,b)?,report:Value::read(&f[1],s,c,b)?})
}
}
impl Value for DocRequirement {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Link {node,target} => variant(s,"DocRequirement","Link",[node.put(s,c,b)?,target.put(s,c,b)?],b),
Self::Asset {node,asset} => variant(s,"DocRequirement","Asset",[node.put(s,c,b)?,asset.put(s,c,b)?],b),
Self::Foreign {embed,kind,guest_digest} => variant(s,"DocRequirement","Foreign",[embed.put(s,c,b)?,kind.put(s,c,b)?,guest_digest.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,46)?;
let (tag,f)=case(v,s,"DocRequirement")?;
match (tag,f.len()) {
("Link",2)=>Ok(Self::Link {node:Value::read(&f[0],s,c,b)?,target:Value::read(&f[1],s,c,b)?}),
("Asset",2)=>Ok(Self::Asset {node:Value::read(&f[0],s,c,b)?,asset:Value::read(&f[1],s,c,b)?}),
("Foreign",3)=>Ok(Self::Foreign {embed:Value::read(&f[0],s,c,b)?,kind:Value::read(&f[1],s,c,b)?,guest_digest:Value::read(&f[2],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for DocPreparationPlan {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocPreparationPlan",[self.document_digest.put(s,c,b)?,self.requirements.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,50)?;
let f=fields(v,s,"DocPreparationPlan",2)?;
Ok(Self {document_digest:Value::read(&f[0],s,c,b)?,requirements:Value::read(&f[1],s,c,b)?})
}
}
impl Value for PageRegistration {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PageRegistration",[self.id.put(s,c,b)?,self.source.put(s,c,b)?,self.route.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,48)?;
let f=fields(v,s,"PageRegistration",3)?;
Ok(Self {id:Value::read(&f[0],s,c,b)?,source:Value::read(&f[1],s,c,b)?,route:Value::read(&f[2],s,c,b)?})
}
}
impl Value for PageLink {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PageLink",[self.page.put(s,c,b)?,self.node.put(s,c,b)?,self.target.put(s,c,b)?,self.fragment.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"PageLink",4)?;
Ok(Self {page:Value::read(&f[0],s,c,b)?,node:Value::read(&f[1],s,c,b)?,target:Value::read(&f[2],s,c,b)?,fragment:Value::read(&f[3],s,c,b)?})
}
}
impl Value for PageRequirement {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PageRequirement",[self.page.put(s,c,b)?,self.requirement.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,47)?;
let f=fields(v,s,"PageRequirement",2)?;
Ok(Self {page:Value::read(&f[0],s,c,b)?,requirement:Value::read(&f[1],s,c,b)?})
}
}
impl Value for PageLinkPlan {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"PageLinkPlan",[self.identity.put(s,c,b)?,self.links.put(s,c,b)?,self.remaining.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"PageLinkPlan",3)?;
Ok(Self {identity:Value::read(&f[0],s,c,b)?,links:Value::read(&f[1],s,c,b)?,remaining:Value::read(&f[2],s,c,b)?})
}
}
}
