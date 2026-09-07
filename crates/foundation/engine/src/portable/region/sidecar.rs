use super::*;
use nepl3_core::{
    source::Span,
    syntax::{NodeRef, SyntaxBundle, canonical::NodeMapping},
    value::SchemaRef,
    view::{Trivia, TriviaKind},
};
use nepl3_reader::model::ReaderFact;
pub(super) fn facts_value<C: FoundationValueCodec>(
    facts: &[ReaderFact],
    reader: &SchemaRef,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut values = Vec::new();
    for fact in facts {
        b.charge(Resource::Work, 1)?;
        let value = match fact {
            ReaderFact::Capture { name, span } => variant(
                reader,
                "ReaderFact",
                "Capture",
                [name.value(s, c, b)?, span.value(s, c, b)?],
                b,
            )?,
            ReaderFact::Presentation { class, span } => variant(
                reader,
                "ReaderFact",
                "Presentation",
                [class.value(s, c, b)?, span.value(s, c, b)?],
                b,
            )?,
            ReaderFact::Relation {
                schema,
                kind,
                from,
                to,
            } => variant(
                reader,
                "ReaderFact",
                "Relation",
                [
                    schema.value(s, c, b)?,
                    kind.value(s, c, b)?,
                    from.value(s, c, b)?,
                    to.value(s, c, b)?,
                ],
                b,
            )?,
        };
        push(&mut values, value, b)?;
    }
    Ok(NdfValue::List(values))
}
pub(super) fn facts_read<C: FoundationValueCodec>(
    value: &NdfValue,
    reader: &SchemaRef,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<ReaderFact>, PortableError<C::Error>> {
    let mut out = Vec::new();
    for value in list(value)? {
        b.charge(Resource::Work, 1)?;
        let (case, f) = parts(value, reader, "ReaderFact")?;
        let fact = match (case, f) {
            ("Capture", [name, span]) => ReaderFact::Capture {
                name: Value::read(name, s, c, b)?,
                span: Span::read(span, s, c, b)?,
            },
            ("Presentation", [class, span]) => ReaderFact::Presentation {
                class: Value::read(class, s, c, b)?,
                span: Span::read(span, s, c, b)?,
            },
            ("Relation", [schema, kind, from, to]) => ReaderFact::Relation {
                schema: Value::read(schema, s, c, b)?,
                kind: Value::read(kind, s, c, b)?,
                from: Span::read(from, s, c, b)?,
                to: Span::read(to, s, c, b)?,
            },
            _ => return Err(PortableError::Shape),
        };
        push(&mut out, fact, b)?;
    }
    Ok(out)
}
pub(super) fn trivia_value<C: FoundationValueCodec>(
    trivia: &[Trivia],
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut out = Vec::new();
    for v in trivia {
        let case = match v.kind {
            TriviaKind::Whitespace => "Whitespace",
            TriviaKind::Comment => "Comment",
            TriviaKind::Bom => "Bom",
            TriviaKind::Skipped => "Skipped",
        };
        push(
            &mut out,
            record(
                s.foundation,
                "Trivia",
                [
                    v.span.value(s, c, b)?,
                    variant(s.foundation, "TriviaKind", case, [], b)?,
                ],
                b,
            )?,
            b,
        )?;
    }
    Ok(NdfValue::List(out))
}
pub(super) fn trivia_read<C: FoundationValueCodec>(
    value: &NdfValue,
    s: &Schemas<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<Vec<Trivia>, PortableError<C::Error>> {
    let mut out = Vec::new();
    for v in list(value)? {
        let f = fields(v, s.foundation, "Trivia", 2)?;
        let (case, args) = parts(&f[1], s.foundation, "TriviaKind")?;
        if !args.is_empty() {
            return Err(PortableError::Shape);
        }
        let kind = match case {
            "Whitespace" => TriviaKind::Whitespace,
            "Comment" => TriviaKind::Comment,
            "Bom" => TriviaKind::Bom,
            "Skipped" => TriviaKind::Skipped,
            _ => return Err(PortableError::Shape),
        };
        push(
            &mut out,
            Trivia {
                span: Span::read(&f[0], s, c, b)?,
                kind,
            },
            b,
        )?;
    }
    Ok(out)
}
pub(super) fn old_node<E>(
    mapping: &NodeMapping<'_>,
    id: NodeRef,
) -> Result<NodeRef, PortableError<E>> {
    Ok(NodeRef(
        *mapping
            .order()
            .get(usize::try_from(id.0).map_err(|_| PortableError::Shape)?)
            .ok_or(PortableError::Shape)? as u64,
    ))
}
pub(super) fn path_read<'a, E>(
    path: &[crate::recovery::ForeignStep],
    mut owner: &'a SyntaxBundle,
    map: &super::super::tree::canonical::Mappings<'_>,
    profile: &crate::profile::ResolvedParseProfile<'_>,
    b: &mut Budget,
) -> Result<(Vec<crate::recovery::ForeignStep>, &'a SyntaxBundle), PortableError<E>> {
    let mut out = Vec::new();
    for step in path {
        b.charge(Resource::Work, step.field.len() as u64 + 1)?;
        b.charge(Resource::AllocationUnits, step.field.len() as u64)?;
        let old = crate::recovery::ForeignStep {
            node: old_node(map.owner(owner, b)?, step.node)?,
            field: step.field.clone(),
        };
        owner = crate::tree::path(owner, core::slice::from_ref(&old), profile.registry(), b)?;
        push(&mut out, old, b)?;
    }
    Ok((out, owner))
}
