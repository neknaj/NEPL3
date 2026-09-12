// Generated from interfaces/math.json by tools/generate/math.py. Do not edit.
use super::*;
#[rustfmt::skip]
mod adapters {
use super::*;
impl Value for ExprRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"ExprRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,39)?;
let f=fields(v,s,"ExprRef",1)?;
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
impl Value for DocGuestRef {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"DocGuestRef",[self.0.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,43)?;
let f=fields(v,s,"DocGuestRef",1)?;
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
impl Value for MathRoot {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Expr(value) => variant(s,"MathRoot","Expr",[value.put(s,c,b)?],b),
Self::Row(value) => variant(s,"MathRoot","Row",[value.put(s,c,b)?],b),
Self::DocGuest(value) => variant(s,"MathRoot","DocGuest",[value.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"MathRoot")?;
match (tag,f.len()) {
("Expr",1)=>Ok(Self::Expr(Value::read(&f[0],s,c,b)?)),
("Row",1)=>Ok(Self::Row(Value::read(&f[0],s,c,b)?)),
("DocGuest",1)=>Ok(Self::DocGuest(Value::read(&f[0],s,c,b)?)),
_=>Err(PortableError::Shape),}
}
}
impl Value for MathField {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::SymbolName => variant(s,"MathField","SymbolName",[],b),
Self::LetName => variant(s,"MathField","LetName",[],b),
Self::SumIndex => variant(s,"MathField","SumIndex",[],b),
Self::IntegralIndex => variant(s,"MathField","IntegralIndex",[],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,_c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let (tag,f)=case(v,s,"MathField")?;
match (tag,f.len()) {
("SymbolName",0)=>Ok(Self::SymbolName),
("LetName",0)=>Ok(Self::LetName),
("SumIndex",0)=>Ok(Self::SumIndex),
("IntegralIndex",0)=>Ok(Self::IntegralIndex),
_=>Err(PortableError::Shape),}
}
}
impl Value for MathFieldLocation {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathFieldLocation",[self.field.put(s,c,b)?,self.origin.put(s,c,b)?,self.span.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,49)?;
let f=fields(v,s,"MathFieldLocation",3)?;
Ok(Self {field:Value::read(&f[0],s,c,b)?,origin:Value::read(&f[1],s,c,b)?,span:Value::read(&f[2],s,c,b)?})
}
}
impl Value for MathKind {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
match self {
Self::Add {left,right} => variant(s,"MathKind","Add",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Sub {left,right} => variant(s,"MathKind","Sub",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Mul {left,right} => variant(s,"MathKind","Mul",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Frac {left,right} => variant(s,"MathKind","Frac",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Pow {left,right} => variant(s,"MathKind","Pow",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Equal {left,right} => variant(s,"MathKind","Equal",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Lt {left,right} => variant(s,"MathKind","Lt",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Le {left,right} => variant(s,"MathKind","Le",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Subscript {left,right} => variant(s,"MathKind","Subscript",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Superscript {left,right} => variant(s,"MathKind","Superscript",[left.put(s,c,b)?,right.put(s,c,b)?],b),
Self::Neg {value} => variant(s,"MathKind","Neg",[value.put(s,c,b)?],b),
Self::Sqrt {value} => variant(s,"MathKind","Sqrt",[value.put(s,c,b)?],b),
Self::Transpose {value} => variant(s,"MathKind","Transpose",[value.put(s,c,b)?],b),
Self::Det {value} => variant(s,"MathKind","Det",[value.put(s,c,b)?],b),
Self::Root {degree,radicand} => variant(s,"MathKind","Root",[degree.put(s,c,b)?,radicand.put(s,c,b)?],b),
Self::Scripts {base,sub,sup} => variant(s,"MathKind","Scripts",[base.put(s,c,b)?,sub.put(s,c,b)?,sup.put(s,c,b)?],b),
Self::Fence {open,close,value} => variant(s,"MathKind","Fence",[open.put(s,c,b)?,close.put(s,c,b)?,value.put(s,c,b)?],b),
Self::Sequence {values} => variant(s,"MathKind","Sequence",[values.put(s,c,b)?],b),
Self::Symbol {name} => variant(s,"MathKind","Symbol",[name.put(s,c,b)?],b),
Self::Text {text} => variant(s,"MathKind","Text",[text.put(s,c,b)?],b),
Self::Vector {values} => variant(s,"MathKind","Vector",[values.put(s,c,b)?],b),
Self::Matrix {rows} => variant(s,"MathKind","Matrix",[rows.put(s,c,b)?],b),
Self::Let {name,init,body} => variant(s,"MathKind","Let",[name.put(s,c,b)?,init.put(s,c,b)?,body.put(s,c,b)?],b),
Self::Sum {index,lower,upper,body} => variant(s,"MathKind","Sum",[index.put(s,c,b)?,lower.put(s,c,b)?,upper.put(s,c,b)?,body.put(s,c,b)?],b),
Self::Integral {index,lower,upper,body} => variant(s,"MathKind","Integral",[index.put(s,c,b)?,lower.put(s,c,b)?,upper.put(s,c,b)?,body.put(s,c,b)?],b),
Self::Call {function,arguments} => variant(s,"MathKind","Call",[function.put(s,c,b)?,arguments.put(s,c,b)?],b),
Self::Label {value,annotation} => variant(s,"MathKind","Label",[value.put(s,c,b)?,annotation.put(s,c,b)?],b),
Self::Number {value,spelling} => variant(s,"MathKind","Number",[value.put(s,c,b)?,spelling.put(s,c,b)?],b),
Self::Row {values} => variant(s,"MathKind","Row",[values.put(s,c,b)?],b),
Self::DocGuest {syntax} => variant(s,"MathKind","DocGuest",[syntax.put(s,c,b)?],b),
}
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let (tag,f)=case(v,s,"MathKind")?;
match (tag,f.len()) {
("Add",2)=>Ok(Self::Add {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Sub",2)=>Ok(Self::Sub {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Mul",2)=>Ok(Self::Mul {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Frac",2)=>Ok(Self::Frac {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Pow",2)=>Ok(Self::Pow {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Equal",2)=>Ok(Self::Equal {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Lt",2)=>Ok(Self::Lt {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Le",2)=>Ok(Self::Le {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Subscript",2)=>Ok(Self::Subscript {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Superscript",2)=>Ok(Self::Superscript {left:Value::read(&f[0],s,c,b)?,right:Value::read(&f[1],s,c,b)?}),
("Neg",1)=>Ok(Self::Neg {value:Value::read(&f[0],s,c,b)?}),
("Sqrt",1)=>Ok(Self::Sqrt {value:Value::read(&f[0],s,c,b)?}),
("Transpose",1)=>Ok(Self::Transpose {value:Value::read(&f[0],s,c,b)?}),
("Det",1)=>Ok(Self::Det {value:Value::read(&f[0],s,c,b)?}),
("Root",2)=>Ok(Self::Root {degree:Value::read(&f[0],s,c,b)?,radicand:Value::read(&f[1],s,c,b)?}),
("Scripts",3)=>Ok(Self::Scripts {base:Value::read(&f[0],s,c,b)?,sub:Value::read(&f[1],s,c,b)?,sup:Value::read(&f[2],s,c,b)?}),
("Fence",3)=>Ok(Self::Fence {open:Value::read(&f[0],s,c,b)?,close:Value::read(&f[1],s,c,b)?,value:Value::read(&f[2],s,c,b)?}),
("Sequence",1)=>Ok(Self::Sequence {values:Value::read(&f[0],s,c,b)?}),
("Symbol",1)=>Ok(Self::Symbol {name:Value::read(&f[0],s,c,b)?}),
("Text",1)=>Ok(Self::Text {text:Value::read(&f[0],s,c,b)?}),
("Vector",1)=>Ok(Self::Vector {values:Value::read(&f[0],s,c,b)?}),
("Matrix",1)=>Ok(Self::Matrix {rows:Value::read(&f[0],s,c,b)?}),
("Let",3)=>Ok(Self::Let {name:Value::read(&f[0],s,c,b)?,init:Value::read(&f[1],s,c,b)?,body:Value::read(&f[2],s,c,b)?}),
("Sum",4)=>Ok(Self::Sum {index:Value::read(&f[0],s,c,b)?,lower:Value::read(&f[1],s,c,b)?,upper:Value::read(&f[2],s,c,b)?,body:Value::read(&f[3],s,c,b)?}),
("Integral",4)=>Ok(Self::Integral {index:Value::read(&f[0],s,c,b)?,lower:Value::read(&f[1],s,c,b)?,upper:Value::read(&f[2],s,c,b)?,body:Value::read(&f[3],s,c,b)?}),
("Call",2)=>Ok(Self::Call {function:Value::read(&f[0],s,c,b)?,arguments:Value::read(&f[1],s,c,b)?}),
("Label",2)=>Ok(Self::Label {value:Value::read(&f[0],s,c,b)?,annotation:Value::read(&f[1],s,c,b)?}),
("Number",2)=>Ok(Self::Number {value:Value::read(&f[0],s,c,b)?,spelling:Value::read(&f[1],s,c,b)?}),
("Row",1)=>Ok(Self::Row {values:Value::read(&f[0],s,c,b)?}),
("DocGuest",1)=>Ok(Self::DocGuest {syntax:Value::read(&f[0],s,c,b)?}),
_=>Err(PortableError::Shape),}
}
}
impl Value for MathNode {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathNode",[self.kind.put(s,c,b)?,self.origin.put(s,c,b)?,self.span.put(s,c,b)?,self.locations.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"MathNode",4)?;
Ok(Self {kind:Value::read(&f[0],s,c,b)?,origin:Value::read(&f[1],s,c,b)?,span:Value::read(&f[2],s,c,b)?,locations:Value::read(&f[3],s,c,b)?})
}
}
impl Value for MathValue {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathValue",[self.root.put(s,c,b)?,self.nodes.put(s,c,b)?,self.embeds.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,41)?;
let f=fields(v,s,"MathValue",3)?;
Ok(Self {root:Value::read(&f[0],s,c,b)?,nodes:Value::read(&f[1],s,c,b)?,embeds:Value::read(&f[2],s,c,b)?})
}
}
impl Value for MathView {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathView",[self.head.put(s,c,b)?,self.view.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,40)?;
let f=fields(v,s,"MathView",2)?;
Ok(Self {head:Value::read(&f[0],s,c,b)?,view:Value::read(&f[1],s,c,b)?})
}
}
impl Value for MathBinding {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathBinding",[self.occurrence.put(s,c,b)?,self.node.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,43)?;
let f=fields(v,s,"MathBinding",2)?;
Ok(Self {occurrence:Value::read(&f[0],s,c,b)?,node:Value::read(&f[1],s,c,b)?})
}
}
impl Value for MathSymbolUse {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathSymbolUse",[self.occurrence.put(s,c,b)?,self.node.put(s,c,b)?,self.binding.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,45)?;
let f=fields(v,s,"MathSymbolUse",3)?;
Ok(Self {occurrence:Value::read(&f[0],s,c,b)?,node:Value::read(&f[1],s,c,b)?,binding:Value::read(&f[2],s,c,b)?})
}
}
impl Value for MathBindings {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathBindings",[self.definitions.put(s,c,b)?,self.uses.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,44)?;
let f=fields(v,s,"MathBindings",2)?;
Ok(Self {definitions:Value::read(&f[0],s,c,b)?,uses:Value::read(&f[1],s,c,b)?})
}
}
impl Value for MathFreeSymbol {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathFreeSymbol",[self.name.put(s,c,b)?,self.occurrences.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,46)?;
let f=fields(v,s,"MathFreeSymbol",2)?;
Ok(Self {name:Value::read(&f[0],s,c,b)?,occurrences:Value::read(&f[1],s,c,b)?})
}
}
impl Value for MathFreeSymbols {
fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {
record(s,"MathFreeSymbols",[self.symbols.put(s,c,b)?],b)
}
fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,c:&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {
b.charge(Resource::Work,47)?;
let f=fields(v,s,"MathFreeSymbols",1)?;
Ok(Self {symbols:Value::read(&f[0],s,c,b)?})
}
}
}
