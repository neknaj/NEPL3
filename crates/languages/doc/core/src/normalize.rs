//! Shared lowering normal form; token-local source views are not rewritten.
use crate::{
    check::{ShapeError, edges},
    model::{DocKind, DocNode, DocValue, InlineRef},
};
/// The literal and prefix paths share this normalization. Source views stay in
/// their original token arenas; merged Text origins retain both input origins.
pub fn document(
    input: &crate::model::DocumentSyntax,
    registry: &nepl3_core::schema::SchemaRegistry,
    b: &mut Budget,
    admission: &mut nepl3_core::source::SourceAdmission,
) -> Result<crate::model::DocumentSyntax, crate::check::StructureError> {
    input.validate_structure(registry, b, admission)?;
    let mut output = input.clone_with_budget(b)?;
    output.value = value(output.value, &mut output.origins, b)?;
    output.validate_structure(registry, b, admission)?;
    Ok(output)
}
use alloc::{string::String, vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    origin::{Origin, OriginId},
};

/// Internal lowering step: the operation owner retains/admitted the source and
/// Origin closure. Input ownership and all intermediate nodes are shallow.
pub(crate) fn value(
    mut input: DocValue,
    origins: &mut Vec<Origin>,
    b: &mut Budget,
) -> Result<DocValue, ShapeError> {
    let order = input.validate_shape(b)?.into_order();
    b.charge(
        Resource::AllocationUnits,
        (input.nodes.len() as u64)
            .saturating_mul((core::mem::size_of::<Option<DocNode>>() + 8) as u64),
    )?;
    let mut old: Vec<_> = core::mem::take(&mut input.nodes)
        .into_iter()
        .map(Some)
        .collect();
    let mut mapping = vec![0u64; old.len()];
    for i in order {
        b.charge(Resource::Work, 1)?;
        let mut node = old[i].take().ok_or(ShapeError::Reference(i as u64))?;
        if matches!(&node.kind,DocKind::Text{text} if text.is_empty()) {
            node.kind = DocKind::Concat {
                inlines: Vec::new(),
            };
        }
        edges::rewrite(&mut node.kind, |id| {
            b.charge(Resource::Work, 1)?;
            mapping
                .get(id as usize)
                .copied()
                .ok_or(ShapeError::Reference(id))
        })?;
        match &mut node.kind {
            DocKind::Sentence { inlines } | DocKind::Concat { inlines } => {
                *inlines = sequence(inlines, &mut input.nodes, origins, b)?
            }
            _ => {}
        }
        if let DocKind::Concat { inlines } = &node.kind
            && inlines.len() == 1
        {
            mapping[i] = inlines[0].0;
            continue;
        }
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<DocNode>() as u64,
        )?;
        mapping[i] = input.nodes.len() as u64;
        input.nodes.push(node);
    }
    edges::rewrite_root(&mut input.root, |id| {
        mapping
            .get(id as usize)
            .copied()
            .ok_or(ShapeError::Reference(id))
    })?;
    compact(input, b)
}
fn sequence(
    items: &[InlineRef],
    nodes: &mut Vec<DocNode>,
    origins: &mut Vec<Origin>,
    b: &mut Budget,
) -> Result<Vec<InlineRef>, ShapeError> {
    b.charge(
        Resource::AllocationUnits,
        (items.len() as u64).saturating_mul(8),
    )?;
    let mut pending: Vec<_> = items.iter().rev().copied().collect();
    let mut output: Vec<InlineRef> = Vec::new();
    while let Some(r) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        let node = nodes.get(r.0 as usize).ok_or(ShapeError::Reference(r.0))?;
        match &node.kind {
            DocKind::Concat { inlines } => {
                b.charge(
                    Resource::AllocationUnits,
                    (inlines.len() as u64).saturating_mul(8),
                )?;
                pending.extend(inlines.iter().rev().copied());
                continue;
            }
            DocKind::Text { text } if text.is_empty() => continue,
            DocKind::Text { text: right } => {
                if let Some(last) = output.last_mut() {
                    let prior = nodes
                        .get(last.0 as usize)
                        .ok_or(ShapeError::Reference(last.0))?;
                    if let DocKind::Text { text: left } = &prior.kind {
                        let len = (left.len() as u64).saturating_add(right.len() as u64);
                        b.charge(Resource::Work, len)?;
                        b.charge(
                            Resource::AllocationUnits,
                            len.saturating_add(core::mem::size_of::<DocNode>() as u64),
                        )?;
                        b.charge(Resource::Nodes, 1)?;
                        let mut text = String::new();
                        text.push_str(left);
                        text.push_str(right);
                        let left_origin = origin(prior, origins, b)?;
                        let right_origin = origin(node, origins, b)?;
                        let origin = match (left_origin, right_origin) {
                            (Some(a), Some(c)) => {
                                b.charge(
                                    Resource::AllocationUnits,
                                    core::mem::size_of::<Origin>() as u64 + 16,
                                )?;
                                let id = OriginId(origins.len() as u64);
                                origins.push(Origin::Composite(vec![a, c]));
                                Some(id)
                            }
                            (None, None) => None,
                            (left, right) => {
                                const REASON: &str =
                                    "Doc normalization of a source-less Text segment";
                                b.charge(
                                    Resource::AllocationUnits,
                                    (core::mem::size_of::<Origin>() as u64)
                                        .saturating_mul(2)
                                        .saturating_add(REASON.len() as u64)
                                        .saturating_add(16),
                                )?;
                                b.charge(Resource::Work, REASON.len() as u64)?;
                                let synthetic = OriginId(origins.len() as u64);
                                origins.push(Origin::Synthetic {
                                    reason: String::from(REASON),
                                    anchor: None,
                                });
                                let joined = OriginId(origins.len() as u64);
                                origins.push(Origin::Composite(vec![
                                    left.unwrap_or(synthetic),
                                    right.unwrap_or(synthetic),
                                ]));
                                Some(joined)
                            }
                        };
                        let id = InlineRef(nodes.len() as u64);
                        nodes.push(DocNode {
                            locations: Vec::new(),
                            kind: DocKind::Text { text },
                            origin,
                            span: None,
                        });
                        *last = id;
                        continue;
                    }
                }
            }
            _ => {}
        }
        b.charge(Resource::AllocationUnits, 8)?;
        output.push(r);
    }
    Ok(output)
}
fn origin(
    node: &DocNode,
    origins: &mut Vec<Origin>,
    b: &mut Budget,
) -> Result<Option<OriginId>, ShapeError> {
    if node.origin.is_some() {
        return Ok(node.origin);
    }
    if let Some(span) = &node.span {
        b.charge(Resource::Work, span.snapshot_ref().source.0.len() as u64)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Origin>() as u64 + span.snapshot_ref().source.0.len() as u64,
        )?;
        let origin = OriginId(origins.len() as u64);
        origins.push(Origin::Direct(span.clone()));
        return Ok(Some(origin));
    }
    Ok(None)
}
pub(crate) fn compact(mut value: DocValue, b: &mut Budget) -> Result<DocValue, ShapeError> {
    let order = value.walk_shape(b, false)?.into_order();
    b.charge(
        Resource::AllocationUnits,
        (value.nodes.len() as u64)
            .saturating_mul((core::mem::size_of::<Option<DocNode>>() + 8) as u64),
    )?;
    let mut nodes: Vec<_> = core::mem::take(&mut value.nodes)
        .into_iter()
        .map(Some)
        .collect();
    let mut mapping = vec![0u64; nodes.len()];
    for (new, old) in order.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        mapping[*old] = new as u64;
    }
    b.charge(
        Resource::AllocationUnits,
        (order.len() as u64).saturating_mul(core::mem::size_of::<DocNode>() as u64),
    )?;
    for old in order {
        let mut node = nodes[old].take().ok_or(ShapeError::Reference(old as u64))?;
        edges::rewrite(&mut node.kind, |id| {
            b.charge(Resource::Work, 1)?;
            mapping
                .get(id as usize)
                .copied()
                .ok_or(ShapeError::Reference(id))
        })?;
        value.nodes.push(node);
    }
    edges::rewrite_root(&mut value.root, |id| {
        mapping
            .get(id as usize)
            .copied()
            .ok_or(ShapeError::Reference(id))
    })?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{DocRoot, SentenceRef};
    use nepl3_core::budget::Limits;
    fn b() -> Budget {
        Budget::new(Limits {
            work: 1_000_000,
            depth: 100,
            nodes: 1000,
            allocation_units: 1_000_000,
            ..Limits::default()
        })
    }
    #[test]
    fn normal_form_flattens_only_sequences_and_retains_explicit_break() -> Result<(), ShapeError> {
        let kinds = vec![
            DocKind::Text { text: "a".into() },
            DocKind::Text {
                text: String::new(),
            },
            DocKind::Text { text: "b".into() },
            DocKind::Concat {
                inlines: vec![InlineRef(0), InlineRef(1), InlineRef(2)],
            },
            DocKind::Break,
            DocKind::Text { text: "c".into() },
            DocKind::Text {
                text: "read".into(),
            },
            DocKind::Ruby {
                base: InlineRef(5),
                reading: InlineRef(6),
            },
            DocKind::Sentence {
                inlines: vec![InlineRef(3), InlineRef(4), InlineRef(7)],
            },
        ];
        let input = DocValue {
            root: DocRoot::Sentence(SentenceRef(8)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    locations: Vec::new(),
                    kind,
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: vec![],
        };
        let output = value(input, &mut Vec::new(), &mut b())?;
        output.validate_shape(&mut b())?;
        assert_eq!(output.nodes.len(), 6);
        assert_eq!(output.nodes[0].kind, DocKind::Text { text: "ab".into() });
        assert_eq!(output.nodes[1].kind, DocKind::Break);
        assert_eq!(
            output.nodes[4].kind,
            DocKind::Ruby {
                base: InlineRef(2),
                reading: InlineRef(3)
            }
        );
        assert_eq!(
            output.nodes[5].kind,
            DocKind::Sentence {
                inlines: vec![InlineRef(0), InlineRef(1), InlineRef(4)]
            }
        );
        let again = value(output.clone(), &mut Vec::new(), &mut b())?;
        assert_eq!(again, output);
        Ok(())
    }
}
