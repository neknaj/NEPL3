use super::*;
use crate::{
    selection::{NodeSelection, ShapeSelection},
    tree::TreeError,
};
use nepl3_core::{
    budget::Resource,
    origin::Origin,
    syntax::{FieldValue, NodeRef, SyntaxBundle, SyntaxNode},
};
fn error(value: TreeError) -> InsertionError {
    match crate::parse::ParseError::Tree(value).stop_reason() {
        Some(reason) => InsertionError::Stopped(reason),
        None => InsertionError::OccurrenceMismatch,
    }
}
fn node(bundle: &SyntaxBundle, id: NodeRef) -> Result<&SyntaxNode, InsertionError> {
    usize::try_from(id.0)
        .ok()
        .and_then(|v| bundle.nodes.get(v))
        .ok_or(InsertionError::OccurrenceMismatch)
}
fn selected<'a>(
    input: &'a PreparedBindingRequest<'_, '_>,
    bundle: &SyntaxBundle,
    id: NodeRef,
    b: &mut Budget,
) -> Result<&'a NodeSelection, InsertionError> {
    for context in &input.tree.tree().contexts {
        b.charge(Resource::Work, 1)?;
        let owner = crate::tree::path(
            &input.tree.tree().bundle,
            &context.path,
            input.profile.registry(),
            b,
        )
        .map_err(error)?;
        if core::ptr::eq(owner, bundle) {
            for item in &context.nodes {
                b.charge(Resource::Work, 1)?;
                if item.node == id {
                    return Ok(item);
                }
            }
            break;
        }
    }
    Err(InsertionError::OccurrenceMismatch)
}
pub(super) fn check(
    original: &PreparedBindingRequest<'_, '_>,
    candidate: &PreparedBindingRequest<'_, '_>,
    wanted: &ExpectedRead,
    parsed: &RetainedParse<'_>,
    edit: &TextEdit,
    b: &mut Budget,
) -> Result<Span, InsertionError> {
    let (mut a_bundle, mut c_bundle) =
        (&original.tree.tree().bundle, &candidate.tree.tree().bundle);
    let (mut a_id, mut c_id) = (a_bundle.root, c_bundle.root);
    for (depth, step) in wanted.path.iter().enumerate() {
        b.charge(Resource::Nodes, 1)?;
        b.charge(Resource::Work, 1)?;
        b.observe_depth((depth as u64).saturating_add(1))?;
        let a = selected(original, a_bundle, a_id, b)?;
        let c = selected(candidate, c_bundle, c_id, b)?;
        b.charge(Resource::Work, 32)?;
        if a.execution_digest != c.execution_digest
            || !crate::selection::entry_equal(&a.entry, &c.entry, b)?
            || !a.shape.same_resolved_with_budget(&c.shape, b)?
        {
            return Err(InsertionError::OccurrenceMismatch);
        }
        let field = match step {
            ExpectedReadStep::Child { field } | ExpectedReadStep::Foreign { field } => *field,
        };
        let field = usize::try_from(field).map_err(|_| InsertionError::OccurrenceMismatch)?;
        let a_declared =
            crate::tree::read::declared(a, field, original.profile, b).map_err(error)?;
        let c_declared =
            crate::tree::read::declared(c, field, candidate.profile, b).map_err(error)?;
        let a_read = crate::tree::read::child(a, field, original.profile, b).map_err(error)?;
        let c_read = crate::tree::read::child(c, field, candidate.profile, b).map_err(error)?;
        if a_declared != c_declared
            || a_read.read != c_read.read
            || a_read.foreign != c_read.foreign
            || !crate::selection::entry_equal(&a_read.entry, &c_read.entry, b)?
        {
            return Err(InsertionError::OccurrenceMismatch);
        }
        let a_field = node(a_bundle, a_id)?
            .fields
            .get(field)
            .ok_or(InsertionError::OccurrenceMismatch)?;
        let c_field = node(c_bundle, c_id)?
            .fields
            .get(field)
            .ok_or(InsertionError::OccurrenceMismatch)?;
        match (step, a_field, c_field) {
            (ExpectedReadStep::Child { .. }, FieldValue::Child(a), FieldValue::Child(c)) => {
                a_id = *a;
                c_id = *c;
            }
            (ExpectedReadStep::Foreign { .. }, FieldValue::Foreign(a), FieldValue::Foreign(c)) => {
                b.charge(
                    Resource::Work,
                    (a.schema.package.len() as u64)
                        .saturating_add(c.schema.package.len() as u64)
                        .saturating_add(a.category.len() as u64)
                        .saturating_add(c.category.len() as u64)
                        .saturating_add(81),
                )?;
                if a.category != c.category || a.environment.digest != c.environment.digest {
                    return Err(InsertionError::OccurrenceMismatch);
                }
                if a.schema != c.schema {
                    // A validated foreign wrapper records its actual root schema.
                    // Missing roots use the engine recovery schema, while a filled
                    // guest root uses the guest package schema. Its guest read
                    // identity was compared above through the prepared trees.
                    let a_root = selected(original, &a.bundle, a.bundle.root, b)?;
                    let c_root = selected(candidate, &c.bundle, c.bundle.root, b)?;
                    if !matches!(a_root.shape, ShapeSelection::Recovery)
                        && !matches!(c_root.shape, ShapeSelection::Recovery)
                    {
                        return Err(InsertionError::OccurrenceMismatch);
                    }
                }
                a_bundle = &a.bundle;
                c_bundle = &c.bundle;
                a_id = a_bundle.root;
                c_id = c_bundle.root;
            }
            _ => return Err(InsertionError::OccurrenceMismatch),
        }
    }
    b.charge(Resource::Nodes, 1)?;
    b.observe_depth((wanted.path.len() as u64).saturating_add(1))?;
    let choice = selected(candidate, c_bundle, c_id, b)?;
    if matches!(choice.shape, ShapeSelection::Recovery) {
        return Err(InsertionError::RecoveryTarget);
    }
    if !crate::selection::entry_equal(&wanted.expected, &choice.entry, b)? {
        return Err(InsertionError::OccurrenceMismatch);
    }
    direct_cover(c_bundle, c_id, parsed.seed().request().snapshot, edit, b)
}
fn direct_cover(
    bundle: &SyntaxBundle,
    id: NodeRef,
    source: &nepl3_core::source::SourceSnapshot,
    edit: &TextEdit,
    b: &mut Budget,
) -> Result<Span, InsertionError> {
    let target = node(bundle, id)?;
    let cover = target
        .cover
        .as_ref()
        .ok_or(InsertionError::UnsupportedOrigin)?;
    let origin = usize::try_from(target.origin.0)
        .ok()
        .and_then(|id| bundle.origins.get(id))
        .ok_or(InsertionError::UnsupportedOrigin)?;
    let Origin::Direct(direct) = origin else {
        return Err(InsertionError::UnsupportedOrigin);
    };
    if cover
        .snapshot_ref()
        .compare_with_budget(source.identity(), b)?
        != core::cmp::Ordering::Equal
        || direct
            .snapshot_ref()
            .compare_with_budget(source.identity(), b)?
            != core::cmp::Ordering::Equal
    {
        return Err(InsertionError::UnsupportedOrigin);
    }
    b.charge(Resource::Work, 5)?;
    let end = edit
        .span
        .start()
        .checked_add(edit.replacement.len() as u64)
        .ok_or(InsertionError::EditMismatch)?;
    if cover.start() >= cover.end()
        || cover.start() < edit.span.start()
        || cover.end() > end
        || direct.start() != cover.start()
        || direct.end() != cover.end()
    {
        return Err(InsertionError::UnsupportedOrigin);
    }
    cover.clone_with_budget(b).map_err(InsertionError::Stopped)
}

#[cfg(test)]
mod tests;
