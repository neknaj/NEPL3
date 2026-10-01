use super::*;
use crate::{recovery::RecoveryKind, selection::NodeSelection, tree::TreeError};
use nepl3_core::syntax::{FieldValue, NodeRef, SyntaxBundle, canonical::BundleMappings};

fn tree_error(error: TreeError) -> ExpectedReadError {
    match error {
        TreeError::Stopped(reason) => reason.into(),
        _ => ExpectedReadError::Owner,
    }
}
fn canonical_error(error: nepl3_core::syntax::canonical::CanonicalError) -> ExpectedReadError {
    match error {
        nepl3_core::syntax::canonical::CanonicalError::Stopped(reason) => reason.into(),
        _ => ExpectedReadError::Owner,
    }
}
fn selection<'a>(
    input: &'a PreparedBindingRequest<'_, '_>,
    bundle: &SyntaxBundle,
    node: NodeRef,
    b: &mut Budget,
) -> Result<&'a NodeSelection, ExpectedReadError> {
    for context in &input.tree.tree().contexts {
        b.charge(Resource::Work, 1)?;
        let found = crate::tree::path(
            &input.tree.tree().bundle,
            &context.path,
            input.profile.registry(),
            b,
        )
        .map_err(tree_error)?;
        if core::ptr::eq(found, bundle) {
            for item in &context.nodes {
                b.charge(Resource::Work, 1)?;
                if item.node == node {
                    return Ok(item);
                }
            }
            return Err(ExpectedReadError::Owner);
        }
    }
    Err(ExpectedReadError::Owner)
}
fn missing<'a>(
    input: &'a PreparedBindingRequest<'_, '_>,
    bundle: &SyntaxBundle,
    node: NodeRef,
    request: &ExpectedReadRequest,
    b: &mut Budget,
) -> Result<Option<&'a EntryContext>, ExpectedReadError> {
    for recovery in &input.tree.tree().recovery {
        b.charge(Resource::Work, 1)?;
        let found = crate::tree::path(
            &input.tree.tree().bundle,
            &recovery.path,
            input.profile.registry(),
            b,
        )
        .map_err(tree_error)?;
        if !core::ptr::eq(found, bundle) {
            continue;
        }
        for item in &recovery.entries {
            b.charge(Resource::Work, 1)?;
            if item.node != node {
                continue;
            }
            if let RecoveryKind::Missing { expected, anchor } = &item.kind {
                b.charge(
                    Resource::Work,
                    (anchor.snapshot_ref().source.0.len() as u64)
                        .saturating_add(request.source.source_id.0.len() as u64)
                        .saturating_add(56),
                )?;
                if anchor.start() == request.offset
                    && anchor.end() == request.offset
                    && anchor.snapshot_ref().source == request.source.source_id
                    && anchor.snapshot_ref().revision == request.source.revision
                    && anchor.snapshot_ref().digest == request.source.digest
                {
                    return Ok(Some(expected));
                }
            }
            return Ok(None);
        }
    }
    Ok(None)
}
struct Pending {
    owner: usize,
    node: NodeRef,
    path: Vec<ExpectedReadStep>,
    incoming: Option<(usize, NodeRef, usize)>,
}
fn path_copy(
    path: &[ExpectedReadStep],
    step: ExpectedReadStep,
    b: &mut Budget,
) -> Result<Vec<ExpectedReadStep>, ExpectedReadError> {
    b.charge(Resource::Work, path.len() as u64 + 1)?;
    b.charge(
        Resource::AllocationUnits,
        (path.len() as u64).saturating_mul(core::mem::size_of::<ExpectedReadStep>() as u64),
    )?;
    let mut out = path.to_vec();
    push(&mut out, step, b)?;
    Ok(out)
}
pub(super) fn run(
    input: &PreparedBindingRequest<'_, '_>,
    request: &ExpectedReadRequest,
    sources: &mut Vec<SourceSnapshot>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Option<Box<ExpectedRead>>, ExpectedReadError> {
    b.charge(Resource::Work, 128)?;
    if b.limits() != input.limits {
        return Err(ExpectedReadError::Access(
            BindingAccessError::LimitsMismatch,
        ));
    }
    if request.key != input.key {
        return Err(ExpectedReadError::Access(BindingAccessError::StaleAnalysis));
    }
    let maps = BundleMappings::new(&input.tree.tree().bundle, b).map_err(canonical_error)?;
    let mut found = false;
    for mapping in maps.entries() {
        for source in &mapping.bundle().sources {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() as u64)
                    .saturating_add(request.source.source_id.0.len() as u64)
                    .saturating_add(48),
            )?;
            if source.identity().source == request.source.source_id
                && source.identity().revision == request.source.revision
                && source.identity().digest == request.source.digest
            {
                source.check_range(request.offset, request.offset)?;
                found = true;
            }
            // SourceAdmission checks identity digest and locator before deduplication.
            // SourceSnapshot construction already binds bytes to that digest.
            admission.admit_existing(source, b)?;
            let mut present = false;
            for prior in sources.iter() {
                if prior.identity().compare_with_budget(source.identity(), b)?
                    == core::cmp::Ordering::Equal
                {
                    present = true;
                    break;
                }
            }
            if !present {
                push(sources, source.clone_with_budget(b)?, b)?;
            }
        }
    }
    if !found {
        return Err(ExpectedReadError::Access(BindingAccessError::MissingSource));
    }
    for i in 1..sources.len() {
        let mut j = i;
        while j > 0 {
            if sources[j - 1]
                .identity()
                .compare_with_budget(sources[j].identity(), b)?
                != core::cmp::Ordering::Greater
            {
                break;
            }
            sources.swap(j - 1, j);
            j -= 1;
        }
    }
    let mut pending = Vec::new();
    push(
        &mut pending,
        Pending {
            owner: 0,
            node: input.tree.tree().bundle.root,
            path: Vec::new(),
            incoming: None,
        },
        b,
    )?;
    while let Some(current) = pending.pop() {
        b.charge(Resource::Nodes, 1)?;
        b.charge(Resource::Work, 1)?;
        b.observe_depth((current.path.len() as u64).saturating_add(1))?;
        let mapping = maps
            .entries()
            .get(current.owner)
            .ok_or(ExpectedReadError::Owner)?;
        let bundle = mapping.bundle();
        let node = bundle
            .nodes
            .get(usize::try_from(current.node.0).map_err(|_| ExpectedReadError::Owner)?)
            .ok_or(ExpectedReadError::Owner)?;
        if let Some(expected) = missing(input, bundle, current.node, request, b)? {
            let origin = if let Some((owner, parent, field)) = current.incoming {
                let parent_map = maps.entries().get(owner).ok_or(ExpectedReadError::Owner)?;
                let selected = selection(input, parent_map.bundle(), parent, b)?;
                let declared = crate::tree::read::declared(selected, field, input.profile, b)
                    .map_err(tree_error)?;
                let resolved = crate::tree::read::child(selected, field, input.profile, b)
                    .map_err(tree_error)?;
                let bytes = expected.alias.len() as u64
                    + expected.category.len() as u64
                    + expected.mode.len() as u64
                    + expected.package.schema.package.len() as u64;
                b.charge(Resource::Work, bytes.saturating_add(80))?;
                if resolved.entry != *expected {
                    return Err(ExpectedReadError::Owner);
                }
                let package = &selected.entry.package;
                b.charge(
                    Resource::Work,
                    (package.schema.package.len() as u64).saturating_add(80),
                )?;
                b.charge(
                    Resource::AllocationUnits,
                    package.schema.package.len() as u64,
                )?;
                ExpectedReadOrigin::Field {
                    parent_bundle: owner as u64,
                    parent_node: parent_map.mapped(parent).map_err(canonical_error)?.0,
                    field: field as u64,
                    owner: package.clone(),
                    declared,
                    resolved_read: resolved.read,
                    foreign: resolved.foreign,
                }
            } else {
                ExpectedReadOrigin::Root
            };
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<ExpectedRead>() as u64,
            )?;
            return Ok(Some(Box::new(ExpectedRead {
                bundle: current.owner as u64,
                node: mapping.mapped(current.node).map_err(canonical_error)?.0,
                path: current.path,
                expected: crate::tree::read::entry_copy(expected, b).map_err(tree_error)?,
                origin,
            })));
        }
        // Stack reversal gives field-order DFS, entering a guest immediately.
        // Shared nodes are revisited per incoming occurrence, never deduplicated.
        for (field, value) in node.fields.iter().enumerate().rev() {
            b.charge(Resource::Work, 1)?;
            let (owner, child, step) = match value {
                FieldValue::Child(child) => (
                    current.owner,
                    *child,
                    ExpectedReadStep::Child {
                        field: field as u64,
                    },
                ),
                FieldValue::Foreign(foreign) => {
                    let mut owner = None;
                    for (i, entry) in maps.entries().iter().enumerate() {
                        b.charge(Resource::Work, 1)?;
                        if core::ptr::eq(entry.bundle(), &foreign.bundle) {
                            owner = Some(i);
                            break;
                        }
                    }
                    (
                        owner.ok_or(ExpectedReadError::Owner)?,
                        foreign.bundle.root,
                        ExpectedReadStep::Foreign {
                            field: field as u64,
                        },
                    )
                }
                FieldValue::Atom(_) => continue,
                FieldValue::Children(_) => return Err(ExpectedReadError::Owner),
            };
            let path = path_copy(&current.path, step, b)?;
            push(
                &mut pending,
                Pending {
                    owner,
                    node: child,
                    path,
                    incoming: Some((current.owner, current.node, field)),
                },
                b,
            )?;
        }
    }
    Ok(None)
}
