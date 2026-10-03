//! Direct source preservation under the one authenticated insertion.
use super::*;
use core::cmp::Ordering;
use nepl3_core::{
    budget::Resource,
    origin::Origin,
    source::{SourceSnapshot, Span},
    syntax::SyntaxNode,
};

fn same_span(a: &Span, c: &Span, b: &mut Budget) -> Result<bool, DeclarationError> {
    b.charge(Resource::Work, 2)?;
    Ok(a.start() == c.start()
        && a.end() == c.end()
        && a.snapshot_ref().compare_with_budget(c.snapshot_ref(), b)? == Ordering::Equal)
}
fn snapshot<'a>(
    parsed: &'a RetainedParse<'_>,
    span: &Span,
    b: &mut Budget,
) -> Result<Option<&'a SourceSnapshot>, DeclarationError> {
    snapshot_in_bundle(&parsed.execution().tree().bundle, span, b)
}
fn snapshot_in_bundle<'a>(
    root: &'a nepl3_core::syntax::SyntaxBundle,
    span: &Span,
    b: &mut Budget,
) -> Result<Option<&'a SourceSnapshot>, DeclarationError> {
    // Reader-emitted sources belong to the executed closure, not necessarily the seed store.
    let maps = nepl3_core::syntax::canonical::BundleMappings::new(root, b)
        .map_err(DeclarationError::Canonical)?;
    let mut found: Option<&SourceSnapshot> = None;
    for map in maps.entries() {
        for source in &map.bundle().sources {
            b.charge(Resource::Nodes, 1)?;
            if source
                .identity()
                .compare_with_budget(span.snapshot_ref(), b)?
                != Ordering::Equal
            {
                continue;
            }
            source
                .check_range(span.start(), span.end())
                .map_err(DeclarationError::Source)?;
            if let Some(previous) = found {
                b.charge(
                    Resource::Work,
                    previous.uri().len() as u64 + source.uri().len() as u64 + 1,
                )?;
                if previous.uri() != source.uri() {
                    return Err(DeclarationError::Structure);
                }
            }
            found = Some(source);
        }
    }
    Ok(found)
}
fn mapped_range(start: u64, end: u64, offset: u64, length: u64) -> Option<(u64, u64)> {
    if start > end || (start == end && start == offset) {
        return None;
    }
    if end <= offset {
        Some((start, end))
    } else if start >= offset {
        Some((start.checked_add(length)?, end.checked_add(length)?))
    } else {
        None
    }
}
fn preserved(
    checked: &checked::CheckedInsertion<'_, '_>,
    a: &Span,
    c: &Span,
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    let Some(old) = snapshot(checked.original(), a, b)? else {
        return Ok(false);
    };
    let Some(new) = snapshot(checked.candidate(), c, b)? else {
        return Ok(false);
    };
    preserved_spans(
        checked.edit(),
        (
            checked.original().seed().request().snapshot,
            checked.candidate().seed().request().snapshot,
        ),
        (a, old),
        (c, new),
        b,
    )
}
fn preserved_spans(
    edit: &TextEdit,
    primary: (&SourceSnapshot, &SourceSnapshot),
    old: (&Span, &SourceSnapshot),
    new: (&Span, &SourceSnapshot),
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    let (old_primary, new_primary) = primary;
    let (a, old) = old;
    let (c, new) = new;
    b.charge(
        Resource::Work,
        old.uri().len() as u64 + new.uri().len() as u64 + 6,
    )?;
    if old.uri() != new.uri() {
        return Ok(false);
    }
    if old
        .identity()
        .compare_with_budget(old_primary.identity(), b)?
        == Ordering::Equal
    {
        if new
            .identity()
            .compare_with_budget(new_primary.identity(), b)?
            != Ordering::Equal
        {
            return Ok(false);
        }
        return Ok(mapped_range(
            a.start(),
            a.end(),
            edit.span.start(),
            edit.replacement.len() as u64,
        ) == Some((c.start(), c.end())));
    }
    // Old primary revisions retained in the candidate store cannot become auxiliaries.
    b.charge(
        Resource::Work,
        old.identity().source.0.len() as u64
            + old_primary.identity().source.0.len() as u64
            + new.identity().source.0.len() as u64
            + new_primary.identity().source.0.len() as u64
            + 2,
    )?;
    if old.identity().source == old_primary.identity().source
        || new.identity().source == new_primary.identity().source
    {
        return Ok(false);
    }
    same_span(a, c, b)
}
fn node<'a>(location: &location::Location<'a>) -> Result<&'a SyntaxNode, DeclarationError> {
    usize::try_from(location.node.0)
        .ok()
        .and_then(|i| location.bundle.nodes.get(i))
        .ok_or(DeclarationError::Structure)
}
fn direct(
    location: &location::Location<'_>,
    span: &Span,
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    let node = node(location)?;
    let origin = usize::try_from(node.origin.0)
        .ok()
        .and_then(|i| location.bundle.origins.get(i))
        .ok_or(DeclarationError::Structure)?;
    let Origin::Direct(origin) = origin else {
        return Ok(false);
    };
    b.charge(Resource::Work, 2)?;
    Ok(origin
        .snapshot_ref()
        .compare_with_budget(span.snapshot_ref(), b)?
        == Ordering::Equal
        && origin.start() <= span.start()
        && span.end() <= origin.end())
}
pub(super) struct Evidence<'a, 'tree> {
    pub owner: &'a location::Location<'tree>,
    pub name: &'a location::Location<'tree>,
    pub entity: &'a nepl3_core::facts::Entity,
}
pub(super) fn same(
    checked: &checked::CheckedInsertion<'_, '_>,
    old: Evidence<'_, '_>,
    new: Evidence<'_, '_>,
    b: &mut Budget,
) -> Result<bool, DeclarationError> {
    let Evidence {
        owner: a_owner,
        name: a_name,
        entity: a_entity,
    } = old;
    let Evidence {
        owner: c_owner,
        name: c_name,
        entity: c_entity,
    } = new;
    let an = node(a_name)?;
    let cn = node(c_name)?;
    let at = an
        .token
        .and_then(|t| usize::try_from(t.0).ok())
        .and_then(|i| a_name.bundle.tokens.get(i))
        .ok_or(DeclarationError::Structure)?;
    let ct = cn
        .token
        .and_then(|t| usize::try_from(t.0).ok())
        .and_then(|i| c_name.bundle.tokens.get(i))
        .ok_or(DeclarationError::Structure)?;
    let (nepl3_core::value::NdfValue::Text(a), nepl3_core::value::NdfValue::Text(c)) =
        (&at.payload, &ct.payload)
    else {
        return Err(DeclarationError::Structure);
    };
    b.charge(
        Resource::Work,
        a.len() as u64
            + c.len() as u64
            + a_entity.name.len() as u64
            + c_entity.name.len() as u64
            + 3,
    )?;
    if a != c || a != &a_entity.name || c != &c_entity.name {
        return Ok(false);
    }
    let (Some(as_), Some(cs)) = (&a_entity.selection, &c_entity.selection) else {
        return Err(DeclarationError::Structure);
    };
    if !same_span(as_, &at.head, b)? || !same_span(cs, &ct.head, b)? {
        return Err(DeclarationError::Structure);
    }
    let (Some(ah), Some(ch)) = (&node(a_owner)?.head, &node(c_owner)?.head) else {
        return Ok(false);
    };
    Ok(direct(a_name, &at.head, b)?
        && direct(c_name, &ct.head, b)?
        && direct(a_owner, ah, b)?
        && direct(c_owner, ch, b)?
        && preserved(checked, &at.head, &ct.head, b)?
        && preserved(checked, ah, ch, b)?)
}

#[cfg(test)]
mod tests {
    use super::mapped_range;
    #[test]
    fn insertion_affinity_preserves_both_adjacent_ranges() {
        assert_eq!(mapped_range(1, 4, 4, 3), Some((1, 4)));
        assert_eq!(mapped_range(4, 8, 4, 3), Some((7, 11)));
        assert_eq!(mapped_range(3, 5, 4, 3), None);
        assert_eq!(mapped_range(4, 4, 4, 3), None);
        assert_eq!(mapped_range(3, 3, 4, 3), Some((3, 3)));
        assert_eq!(mapped_range(5, 5, 4, 3), Some((8, 8)));
        assert_eq!(mapped_range(u64::MAX, u64::MAX, 4, 1), None);
    }
}

#[cfg(test)]
mod source_tests {
    use super::*;
    use nepl3_core::{budget::Limits, source::SourceId};
    fn snapshot(
        name: &str,
        revision: u64,
        locator: &str,
        text: &str,
        b: &mut Budget,
    ) -> Result<SourceSnapshot, DeclarationError> {
        SourceSnapshot::new(
            SourceId(name.into()),
            revision,
            locator.into(),
            text.as_bytes().to_vec(),
            b,
        )
        .map_err(DeclarationError::Source)
    }
    #[test]
    fn primary_classification_precedes_auxiliary_and_rejects_expanded_heads()
    -> Result<(), DeclarationError> {
        let mut b = Budget::new(Limits {
            work: 100_000,
            allocation_units: 100_000,
            source_bytes: 100_000,
            ..Limits::default()
        });
        let old = snapshot("main", 0, "memory:main", "name", &mut b)?;
        let new = snapshot("main", 1, "memory:main", "name x", &mut b)?;
        let span = old.span(0, 4).map_err(DeclarationError::Source)?;
        let unchanged = new.span(0, 4).map_err(DeclarationError::Source)?;
        let expanded = new.span(0, 5).map_err(DeclarationError::Source)?;
        let edit = TextEdit {
            span: old.span(4, 4).map_err(DeclarationError::Source)?,
            expected_digest: nepl3_core::source::Digest::of(b""),
            replacement: " x".into(),
        };
        assert!(preserved_spans(
            &edit,
            (&old, &new),
            (&span, &old),
            (&unchanged, &new),
            &mut b
        )?);
        assert!(!preserved_spans(
            &edit,
            (&old, &new),
            (&span, &old),
            (&expanded, &new),
            &mut b
        )?);
        // The old snapshot may remain in the new closure, but is still the edited primary.
        assert!(!preserved_spans(
            &edit,
            (&old, &new),
            (&span, &old),
            (&span, &old),
            &mut b
        )?);
        let aux = snapshot("aux", 0, "memory:aux", "term", &mut b)?;
        let conflict = snapshot("aux", 0, "memory:other", "term", &mut b)?;
        let aux_span = aux.span(0, 4).map_err(DeclarationError::Source)?;
        assert!(preserved_spans(
            &edit,
            (&old, &new),
            (&aux_span, &aux),
            (&aux_span, &aux),
            &mut b
        )?);
        assert!(!preserved_spans(
            &edit,
            (&old, &new),
            (&aux_span, &aux),
            (&aux_span, &conflict),
            &mut b
        )?);
        Ok(())
    }
    #[test]
    fn source_lookup_exhausts_after_an_initial_matching_snapshot() -> Result<(), DeclarationError> {
        use alloc::vec::Vec;
        use nepl3_core::{
            origin::OriginId,
            source::Digest,
            syntax::{NodeRef, SyntaxBundle, SyntaxNode},
            value::SchemaRef,
        };
        let limits = Limits {
            work: 100_000,
            allocation_units: 100_000,
            source_bytes: 100_000,
            nodes: 100,
            depth: 100,
            ..Limits::default()
        };
        let mut setup = Budget::new(limits);
        let first = snapshot("first", 0, "memory:first", "name", &mut setup)?;
        let later = snapshot("later", 0, "memory:later", "other", &mut setup)?;
        let span = first.span(0, 4).map_err(DeclarationError::Source)?;
        // Isolate closure enumeration; this is not a claim of complete syntax validation.
        let root = SyntaxBundle {
            sources: alloc::vec![first, later],
            origins: Vec::new(),
            environments: Vec::new(),
            tokens: Vec::new(),
            source_maps: Vec::new(),
            root: NodeRef(0),
            nodes: alloc::vec![SyntaxNode {
                schema: SchemaRef {
                    package: "test.source".into(),
                    revision: 1,
                    digest: Digest::of(b"source")
                },
                kind: "Node".into(),
                fields: Vec::new(),
                head: None,
                cover: None,
                origin: OriginId(0),
                token: None
            }],
        };
        let mut full = Budget::new(limits);
        assert!(snapshot_in_bundle(&root, &span, &mut full)?.is_some());
        assert_eq!(full.usage().nodes, 2);
        let mut stopped = Budget::new(Limits { nodes: 1, ..limits });
        assert!(matches!(
            snapshot_in_bundle(&root, &span, &mut stopped),
            Err(DeclarationError::Stopped(StopReason::NodeLimit))
        ));
        Ok(())
    }
}
