"""Generate explicit native Markup adapters from the normative operation schema."""
import argparse
import json
from pathlib import Path
ROOT=Path(__file__).resolve().parents[2]
OUTPUT=ROOT/"crates/output/markup/src/portable/value/generated.rs"

def generate():
    types=json.loads((ROOT/"interfaces/markup.json").read_text(encoding="utf-8"))["types"]
    out=["// Generated from interfaces/markup.json by tools/generate/markup.py. Do not edit.","use super::*;","#[rustfmt::skip]","mod adapters {", "use super::*;"]
    rename={"languageHint":"language_hint","sourceMaps":"source_maps","documentDigest":"document_digest","guestDigest":"guest_digest"}
    for name,shape in types.items():
        if name in ("MarkupSyntax",) or name.startswith("View:"): continue
        out.append(f"impl Value for {name} {{")
        unused_c = "variant" in shape and not any(shape["variant"].values())
        out.append("fn put<C:FoundationValueCodec>(&self,s:&SchemaRef,"+("_c" if unused_c else "c")+":&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {")
        if "record" in shape:
            fs=shape["record"]
            exprs=[("self.0" if name.endswith("Ref") and name!="AssetRef" else "self."+rename.get(f[0],f[0]))+".put(s,c,b)?" for f in fs]
            out.append(f'record(s,"{name}",[{",".join(exprs)}],b)')
        else:
            out.append("match self {")
            for case,fs in shape["variant"].items():
                fields=[rename.get(f[0],f[0]) for f in fs]
                pat=f"Self::{case}"+("("+fields[0]+")" if name=="MarkupRoot" else " {"+",".join(fields)+"}" if fields else "")
                exprs=[f+".put(s,c,b)?" for f in fields]
                out.append(pat+f' => variant(s,"{name}","{case}",[{",".join(exprs)}],b),')
            out.append("}")
        out.append("}")
        out.append("fn read<C:FoundationValueCodec>(v:&NdfValue,s:&SchemaRef,"+("_c" if unused_c else "c")+":&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {")
        out.append(f"b.charge(Resource::Work,{len(name)+32})?;")
        if "record" in shape:
            fs=shape["record"]
            out.append(f'let f=fields(v,s,"{name}",{len(fs)})?;')
            if name.endswith("Ref") and name!="AssetRef": out.append("Ok(Self(Value::read(&f[0],s,c,b)?))")
            else: out.append("Ok(Self {"+",".join(rename.get(f[0],f[0])+f":Value::read(&f[{i}],s,c,b)?" for i,f in enumerate(fs))+"})")
        else:
            out.append(f'let (tag,f)=case(v,s,"{name}")?;')
            out.append("match (tag,f.len()) {")
            for case,fs in shape["variant"].items():
                fields=[rename.get(f[0],f[0]) for f in fs]
                exprs=[f+f":Value::read(&f[{i}],s,c,b)?" for i,f in enumerate(fields)]
                body="(Value::read(&f[0],s,c,b)?)" if name=="MarkupRoot" else " {"+",".join(exprs)+"}" if fields else ""
                out.append(f'("{case}",{len(fs)})=>Ok(Self::{case}{body}),')
            out.append("_=>Err(PortableError::Shape),}")
        out.extend(["}","}"])
    return "\n".join(out+["}"])+"\n"
def main():
    parser=argparse.ArgumentParser();parser.add_argument("--write",action="store_true");args=parser.parse_args()
    for output,text in [(OUTPUT,generate())]:
        if args.write:
            output.parent.mkdir(parents=True,exist_ok=True);output.write_text(text,encoding="utf-8",newline="\n")
        elif not output.exists() or output.read_bytes()!=text.encode("utf-8"): raise SystemExit(f"{output.relative_to(ROOT)} is stale; run python tools/generate/markup.py --write")
if __name__=="__main__":main()
