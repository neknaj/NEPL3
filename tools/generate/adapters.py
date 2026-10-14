"""Typed field-layout projection for native Value adapter generators.

Only field names, order, and record/variant layout affect these adapters. The
existing Rust schema compiler owns descriptor type and constraint validation;
generated Value calls delegate to native types or explicit nominal field codecs.
"""

from collections.abc import Callable, Mapping
from dataclasses import dataclass, field
from enum import Enum
from pathlib import Path
from types import MappingProxyType

from tools.serialization.json import JsonValue, array, decode, object_value, string


@dataclass(frozen=True, slots=True)
class Record:
    fields: tuple[str, ...]


@dataclass(frozen=True, slots=True)
class Variant:
    cases: Mapping[str, tuple[str, ...]]


type Shape = Record | Variant


class Codec(Enum):
    LOCAL = "local"
    REGISTRY = "registry"


@dataclass(frozen=True, slots=True)
class Options:
    excluded: frozenset[str]
    rename: Mapping[str, str]
    tuple_case: Callable[[str, str], bool]
    codec: Codec = Codec.LOCAL
    tuple_records: frozenset[str] = frozenset()
    case_names: Mapping[str, str] = field(default_factory=lambda: MappingProxyType({}))
    field_codecs: Mapping[str, tuple[str, str]] = field(default_factory=lambda: MappingProxyType({}))


def field_names(value: JsonValue) -> tuple[str, ...]:
    names: list[str] = []
    for item in array(value):
        pair = array(item)
        if len(pair) != 2:
            raise ValueError("schema field requires a name and a type")
        names.append(string(pair[0]))
    return tuple(names)


def shapes(value: JsonValue, excluded: frozenset[str]) -> Mapping[str, Shape]:
    result: dict[str, Shape] = {}
    for name, item in object_value(object_value(value)["types"]).items():
        if name in excluded or name.startswith("View:"):
            continue
        shape = object_value(item)
        if "record" in shape and "variant" not in shape:
            result[name] = Record(field_names(shape["record"]))
        elif "variant" in shape and "record" not in shape:
            result[name] = Variant(MappingProxyType({
                case: field_names(fields) for case, fields in object_value(shape["variant"]).items()
            }))
        else:
            raise ValueError(f"adapter requires exactly one record or variant layout: {name}")
    return MappingProxyType(result)


def render(types: Mapping[str, Shape], header: str, options: Options) -> str:
    out = [header, "use super::*;", "#[rustfmt::skip]", "mod adapters {", "use super::*;"]
    registry = options.codec is Codec.REGISTRY
    parameters = "s:&SchemaRef,r:&SchemaRegistry," if registry else "s:&SchemaRef,"
    arguments = "s,r,c,b" if registry else "s,c,b"
    rename = options.rename
    def put_field(owner: str, member: str, expression: str, borrowed: str) -> str:
        codec = options.field_codecs.get(f"{owner}.{member}")
        return (f"{codec[0]}({borrowed},{arguments})?" if codec
                else f"{expression}.put({arguments})?")
    def read_field(owner: str, member: str, index: int) -> str:
        codec = options.field_codecs.get(f"{owner}.{member}")
        reader = codec[1] if codec else "Value::read"
        return f"{reader}(&f[{index}],{arguments})?"
    for name, shape in types.items():
        tuple_record = name in options.tuple_records or (name.endswith("Ref") and name != "AssetRef")
        if isinstance(shape, Record) and tuple_record and len(shape.fields) != 1:
            raise ValueError(f"native tuple record requires one field: {name}")
        out.append(f"impl Value for {name} {{")
        codec_name = "_c" if isinstance(shape, Variant) and not any(shape.cases.values()) else "c"
        out.append("fn put<C:FoundationValueCodec>(&self," + parameters + codec_name
                   + ":&mut C,b:&mut Budget)->Result<NdfValue,PortableError<C::Error>> {")
        if isinstance(shape, Record):
            exprs = [put_field(name, member,
                               "self.0" if tuple_record else "self." + rename.get(member, member),
                               "&self.0" if tuple_record else "&self." + rename.get(member, member))
                     for member in shape.fields]
            out.append(f'record(s,"{name}",[{",".join(exprs)}],b)')
        else:
            out.append("match self {")
            for case, members in shape.cases.items():
                native_case = options.case_names.get(f"{name}.{case}", case)
                fields = [rename.get(field, field) for field in members]
                if options.tuple_case(name, case):
                    if len(fields) != 1:
                        raise ValueError(f"native tuple case requires one field: {name}::{case}")
                    pattern = f"Self::{native_case}(" + fields[0] + ")"
                else:
                    pattern = f"Self::{native_case}" + (" {" + ",".join(fields) + "}" if fields else "")
                exprs = [put_field(f"{name}.{case}", member, field, field)
                         for member, field in zip(members, fields)]
                out.append(pattern + f' => variant(s,"{name}","{case}",[{",".join(exprs)}],b),')
            out.append("}")
        out.append("}")
        out.append("fn read<C:FoundationValueCodec>(v:&NdfValue," + parameters + codec_name
                   + ":&mut C,b:&mut Budget)->Result<Self,PortableError<C::Error>> {")
        out.append(f"b.charge(Resource::Work,{len(name) + 32})?;")
        if isinstance(shape, Record):
            out.append(f'let f=fields(v,s,"{name}",{len(shape.fields)})?;')
            if tuple_record:
                out.append("Ok(Self(" + read_field(name, shape.fields[0], 0) + "))")
            else:
                exprs = [rename.get(member, member) + ":" + read_field(name, member, index)
                         for index, member in enumerate(shape.fields)]
                out.append("Ok(Self {" + ",".join(exprs) + "})")
        else:
            out.append(f'let (tag,f)=case(v,s,"{name}")?;')
            out.append("match (tag,f.len()) {")
            for case, members in shape.cases.items():
                native_case = options.case_names.get(f"{name}.{case}", case)
                fields = [rename.get(field, field) for field in members]
                exprs = [field + ":" + read_field(f"{name}.{case}", member, index)
                         for index, (member, field) in enumerate(zip(members, fields))]
                body = ("(" + read_field(f"{name}.{case}", members[0], 0) + ")" if options.tuple_case(name, case)
                        else " {" + ",".join(exprs) + "}" if fields else "")
                out.append(f'("{case}",{len(members)})=>Ok(Self::{native_case}{body}),')
            out.append("_=>Err(PortableError::Shape),}")
        out.extend(["}", "}"])
    return "\n".join(out + ["}"]) + "\n"


def generate(root: Path, interface: str, script: str, options: Options) -> str:
    source = (root / "interfaces" / f"{interface}.json").read_text(encoding="utf-8")
    header = f"// Generated from interfaces/{interface}.json by tools/generate/{script}.py. Do not edit."
    return render(shapes(decode(source), options.excluded), header, options)
