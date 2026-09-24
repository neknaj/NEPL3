use super::{Error, boundary};
use crate::model::*;
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    value::{NdfValue, Record, SchemaRef, TypedValue, Variant},
    value_codec::FoundationValueCodec,
};

pub(super) fn reserve<T, E>(count: usize, b: &mut Budget) -> Result<Vec<T>, Error<E>> {
    b.charge(Resource::Work, count as u64)?;
    b.charge(
        Resource::AllocationUnits,
        (count as u64).saturating_mul(core::mem::size_of::<T>() as u64),
    )?;
    Ok(Vec::with_capacity(count))
}
fn text<E>(s: &str, b: &mut Budget) -> Result<String, Error<E>> {
    b.charge(Resource::Work, s.len() as u64)?;
    b.charge(Resource::AllocationUnits, s.len() as u64)?;
    Ok(s.into())
}
pub(super) fn record<E, const N: usize>(
    s: &SchemaRef,
    kind: &str,
    fields: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, Error<E>> {
    let package = text(&s.package, b)?;
    let kind = text(kind, b)?;
    let mut out = reserve(N, b)?;
    out.extend(fields);
    Ok(NdfValue::Record(Record {
        schema: SchemaRef {
            package,
            revision: s.revision,
            digest: s.digest,
        },
        kind,
        fields: out,
    }))
}
fn variant<E, const N: usize>(
    s: &SchemaRef,
    ty: &str,
    case: &str,
    fields: [NdfValue; N],
    b: &mut Budget,
) -> Result<NdfValue, Error<E>> {
    let package = text(&s.package, b)?;
    let type_name = text(ty, b)?;
    let variant = text(case, b)?;
    let mut out = reserve(N, b)?;
    out.extend(fields);
    Ok(NdfValue::Variant(Variant {
        schema: SchemaRef {
            package,
            revision: s.revision,
            digest: s.digest,
        },
        type_name,
        variant,
        fields: out,
    }))
}
fn reference<E>(
    s: &SchemaRef,
    kind: &str,
    index: u64,
    b: &mut Budget,
) -> Result<NdfValue, Error<E>> {
    record(s, kind, [NdfValue::U64(index)], b)
}
fn inline<E>(s: &SchemaRef, index: InlineRef, b: &mut Budget) -> Result<NdfValue, Error<E>> {
    reference(s, "InlineRef", index.0, b)
}
fn inlines<E>(s: &SchemaRef, refs: &[InlineRef], b: &mut Budget) -> Result<NdfValue, Error<E>> {
    let mut out = reserve(refs.len(), b)?;
    for id in refs {
        out.push(inline(s, *id, b)?);
    }
    Ok(NdfValue::List(out))
}
pub(super) fn encode<C: FoundationValueCodec>(
    v: &SentenceValue,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, Error<C::Error>> {
    let (tag, kind, index) = match v.root {
        Root::Sentence(r) => ("Sentence", "SentenceRef", r.0),
        Root::Inline(r) => ("Inline", "InlineRef", r.0),
    };
    let root = variant(s, "SentenceRoot", tag, [reference(s, kind, index, b)?], b)?;
    let mut nodes = reserve(v.nodes.len(), b)?;
    for node in &v.nodes {
        nodes.push(encode_node(node, s, b)?);
    }
    let mut embeds = reserve(v.embeds.len(), b)?;
    for closure in &v.embeds {
        let (case, value) = match closure {
            InlineContent::Syntax { closure } => (
                "Syntax",
                c.encode_foreign_closure(closure, b).map_err(boundary)?,
            ),
            InlineContent::Value { value } => (
                "Value",
                match value.clone_with_budget(b)? {
                    TypedValue::Record(value) => NdfValue::Record(value),
                    TypedValue::Variant(value) => NdfValue::Variant(value),
                },
            ),
        };
        embeds.push(variant(s, "InlineContent", case, [value], b)?);
    }
    record(
        s,
        "SentenceValue",
        [root, NdfValue::List(nodes), NdfValue::List(embeds)],
        b,
    )
}
fn encode_node<E>(v: &Kind, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, Error<E>> {
    b.charge(Resource::Work, 1)?;
    match v {
        Kind::Sentence { inlines: refs } => {
            variant(s, "SentenceKind", "Sentence", [inlines(s, refs, b)?], b)
        }
        Kind::Concat { inlines: refs } => {
            variant(s, "SentenceKind", "Concat", [inlines(s, refs, b)?], b)
        }
        Kind::Text { text: t } => {
            variant(s, "SentenceKind", "Text", [NdfValue::Text(text(t, b)?)], b)
        }
        Kind::Code { text: t } => {
            variant(s, "SentenceKind", "Code", [NdfValue::Text(text(t, b)?)], b)
        }
        Kind::Ruby { base, reading } => variant(
            s,
            "SentenceKind",
            "Ruby",
            [inline(s, *base, b)?, inline(s, *reading, b)?],
            b,
        ),
        Kind::InlineAnno { base, notes } => variant(
            s,
            "SentenceKind",
            "InlineAnno",
            [inline(s, *base, b)?, inlines(s, notes, b)?],
            b,
        ),
        Kind::Emphasis { inline: r } => {
            variant(s, "SentenceKind", "Emphasis", [inline(s, *r, b)?], b)
        }
        Kind::Strong { inline: r } => variant(s, "SentenceKind", "Strong", [inline(s, *r, b)?], b),
        Kind::Break => variant(s, "SentenceKind", "Break", [], b),
        Kind::ExternalLink { uri, label } => variant(
            s,
            "SentenceKind",
            "ExternalLink",
            [NdfValue::Text(text(uri, b)?), inline(s, *label, b)?],
            b,
        ),
        Kind::ForeignInline { syntax } => variant(
            s,
            "SentenceKind",
            "ForeignInline",
            [reference(s, "EmbedRef", syntax.0, b)?],
            b,
        ),
    }
}
pub(super) fn fields<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    kind: &str,
) -> Result<&'a [NdfValue], Error<E>> {
    match v {
        NdfValue::Record(r) if &r.schema == s && r.kind == kind => Ok(&r.fields),
        _ => Err(Error::Shape),
    }
}
fn case<'a, E>(
    v: &'a NdfValue,
    s: &SchemaRef,
    ty: &str,
) -> Result<(&'a str, &'a [NdfValue]), Error<E>> {
    match v {
        NdfValue::Variant(r) if &r.schema == s && r.type_name == ty => Ok((&r.variant, &r.fields)),
        _ => Err(Error::Shape),
    }
}
fn index<E>(v: &NdfValue, s: &SchemaRef, kind: &str, b: &mut Budget) -> Result<u64, Error<E>> {
    b.charge(Resource::Work, 1)?;
    match fields(v, s, kind)? {
        [NdfValue::U64(index)] => Ok(*index),
        _ => Err(Error::Shape),
    }
}
fn read_inlines<E>(
    v: &NdfValue,
    s: &SchemaRef,
    b: &mut Budget,
) -> Result<Vec<InlineRef>, Error<E>> {
    let NdfValue::List(values) = v else {
        return Err(Error::Shape);
    };
    let mut out = reserve(values.len(), b)?;
    for v in values {
        out.push(InlineRef(index(v, s, "InlineRef", b)?));
    }
    Ok(out)
}
pub(super) fn decode<C: FoundationValueCodec>(
    v: &NdfValue,
    s: &SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<SentenceValue, Error<C::Error>> {
    let [root, NdfValue::List(raw_nodes), NdfValue::List(raw_embeds)] =
        fields(v, s, "SentenceValue")?
    else {
        return Err(Error::Shape);
    };
    let root = match case(root, s, "SentenceRoot")? {
        ("Sentence", [r]) => Root::Sentence(SentenceRef(index(r, s, "SentenceRef", b)?)),
        ("Inline", [r]) => Root::Inline(InlineRef(index(r, s, "InlineRef", b)?)),
        _ => return Err(Error::Shape),
    };
    let mut nodes = reserve(raw_nodes.len(), b)?;
    for raw in raw_nodes {
        nodes.push(decode_node(raw, s, b)?);
    }
    let mut embeds = reserve(raw_embeds.len(), b)?;
    for raw in raw_embeds {
        embeds.push(match case(raw, s, "InlineContent")? {
            ("Syntax", [closure]) => {
                let closure = c.decode_foreign_closure(closure, b).map_err(boundary)?;
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of_val(&closure) as u64,
                )?;
                InlineContent::from(closure)
            }
            ("Value", [value]) => {
                value.charge_clone(b)?;
                InlineContent::Value {
                    value: match value {
                        NdfValue::Record(value) => TypedValue::Record(value.clone()),
                        NdfValue::Variant(value) => TypedValue::Variant(value.clone()),
                        _ => return Err(Error::Shape),
                    },
                }
            }
            _ => return Err(Error::Shape),
        });
    }
    Ok(SentenceValue {
        root,
        nodes,
        embeds,
    })
}
fn decode_node<E>(v: &NdfValue, s: &SchemaRef, b: &mut Budget) -> Result<Kind, Error<E>> {
    b.charge(Resource::Work, 1)?;
    Ok(match case(v, s, "SentenceKind")? {
        ("Sentence", [refs]) => Kind::Sentence {
            inlines: read_inlines(refs, s, b)?,
        },
        ("Concat", [refs]) => Kind::Concat {
            inlines: read_inlines(refs, s, b)?,
        },
        ("Text", [NdfValue::Text(t)]) => Kind::Text { text: text(t, b)? },
        ("Code", [NdfValue::Text(t)]) => Kind::Code { text: text(t, b)? },
        ("Ruby", [base, reading]) => Kind::Ruby {
            base: InlineRef(index(base, s, "InlineRef", b)?),
            reading: InlineRef(index(reading, s, "InlineRef", b)?),
        },
        ("InlineAnno", [base, notes]) => Kind::InlineAnno {
            base: InlineRef(index(base, s, "InlineRef", b)?),
            notes: read_inlines(notes, s, b)?,
        },
        ("Emphasis", [r]) => Kind::Emphasis {
            inline: InlineRef(index(r, s, "InlineRef", b)?),
        },
        ("Strong", [r]) => Kind::Strong {
            inline: InlineRef(index(r, s, "InlineRef", b)?),
        },
        ("Break", []) => Kind::Break,
        ("ExternalLink", [NdfValue::Text(uri), label]) => Kind::ExternalLink {
            uri: text(uri, b)?,
            label: InlineRef(index(label, s, "InlineRef", b)?),
        },
        ("ForeignInline", [syntax]) => Kind::ForeignInline {
            syntax: EmbedRef(index(syntax, s, "EmbedRef", b)?),
        },
        _ => return Err(Error::Shape),
    })
}
