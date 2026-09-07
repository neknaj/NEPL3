//! Recursive header claims are structural on receive; only the binding
//! machine's private memo establishes which header was actually accepted.
use super::*;
use crate::{binding::CanonicalBindingTarget, profile::ResolvedParseProfile};
use nepl3_core::{
    budget::{Budget, Resource},
    facts::OccurrenceRole,
    schema::SchemaRegistry,
    syntax::canonical::{BundleMappings, CanonicalError},
};
fn canonical_error(error: CanonicalError) -> FactsError {
    match error {
        CanonicalError::Stopped(reason) => FactsError::Stopped(reason),
        _ => FactsError::Target,
    }
}
pub(crate) fn target(
    tree: &ParseTree,
    path: &[ForeignStep],
    node: NodeRef,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<CanonicalBindingTarget, FactsError> {
    let mappings = BundleMappings::new(&tree.bundle, b).map_err(canonical_error)?;
    let mapped = |owner: &nepl3_core::syntax::SyntaxBundle, node, b: &mut Budget| {
        for entry in mappings.entries() {
            b.charge(Resource::Work, 1)?;
            if core::ptr::eq(entry.bundle(), owner) {
                return entry.mapped(node).map_err(canonical_error);
            }
        }
        Err(FactsError::Target)
    };
    let mut owner = &tree.bundle;
    let mut result = Vec::new();
    for step in path {
        b.charge(Resource::Work, step.field.len() as u64 + 1)?;
        b.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<ForeignStep>() + step.field.len()) as u64,
        )?;
        result.push(ForeignStep {
            node: mapped(owner, step.node, b)?,
            field: step.field.clone(),
        });
        owner = crate::tree::path(owner, core::slice::from_ref(step), registry, b)?;
    }
    Ok(CanonicalBindingTarget {
        path: result,
        node: mapped(owner, node, b)?,
    })
}
pub(crate) fn validate(
    request: FactsRequestView<'_>,
    profile: &ResolvedParseProfile<'_>,
    b: &mut Budget,
) -> Result<(), FactsError> {
    let (group, header) = match request.phase {
        FactsPhase::Ordinary => return Ok(()),
        FactsPhase::Header { group } => (*group, None),
        FactsPhase::Body { header } => (header.group, Some(header.as_ref())),
    };
    b.charge(Resource::Work, request.existing.scopes.len() as u64 + 1)?;
    if !request.existing.scopes.iter().any(|s| s.id == group) {
        return Err(FactsError::Phase);
    }
    let mut current = Some(request.authority.current_scope);
    let mut belongs = false;
    // The existing FactSet has already proved acyclic scope ancestry.
    while let Some(id) = current {
        b.charge(Resource::Work, request.existing.scopes.len() as u64 + 1)?;
        if id == group {
            belongs = true;
            break;
        }
        current = request
            .existing
            .scopes
            .iter()
            .find(|s| s.id == id)
            .ok_or(FactsError::Phase)?
            .parent;
    }
    if !belongs {
        return Err(FactsError::Phase);
    }
    if let Some(header) = header {
        let provider = profile
            .provider(&header.provider.operation, b)
            .map_err(crate::tree::TreeError::from)?;
        b.charge(
            Resource::Work,
            (header.provider.provider.len()
                + header.provider.operation.name.len()
                + header.provider.operation.schema.package.len()) as u64
                + 80,
        )?;
        if provider != &header.provider {
            return Err(FactsError::Phase);
        }
        let descriptor = profile
            .registry()
            .descriptor(&provider.operation.schema)
            .ok_or(FactsError::Phase)?;
        b.charge(
            Resource::Work,
            (descriptor.operations.len() as u64)
                .saturating_mul(provider.operation.name.len() as u64 + 1),
        )?;
        let operation = descriptor
            .operations
            .iter()
            .find(|v| v.name == provider.operation.name)
            .ok_or(FactsError::Phase)?;
        if !super::signature(&operation.input, &operation.output, operation.pure) {
            return Err(FactsError::Phase);
        }
        let actual = target(
            request.tree,
            request.path,
            request.node,
            profile.registry(),
            b,
        )?;
        for step in &header.target.path {
            b.charge(Resource::Work, step.field.len() as u64 + 2)?;
        }
        if actual != header.target {
            return Err(FactsError::Phase);
        }
        for (index, id) in header.entities.iter().enumerate() {
            b.charge(
                Resource::Work,
                (index + request.existing.entities.len() + 1) as u64,
            )?;
            if header.entities[..index].contains(id)
                || !request.existing.entities.iter().any(|e| e.id == *id)
            {
                return Err(FactsError::Phase);
            }
        }
        for (index, id) in header.exports.iter().enumerate() {
            b.charge(Resource::Work, (index + header.entities.len() + 1) as u64)?;
            if header.exports[..index].contains(id) || !header.entities.contains(id) {
                return Err(FactsError::Phase);
            }
        }
    }
    Ok(())
}
/// Header collection may declare names but cannot resolve references before
/// the enclosing recursive group has collected all declarations.
pub(crate) fn delta(
    phase: &FactsPhase,
    delta: &FactDelta,
    b: &mut Budget,
) -> Result<(), FactsError> {
    if matches!(phase, FactsPhase::Header { .. }) {
        b.charge(Resource::Work, delta.occurrences.len() as u64 + 1)?;
        if !delta.resolutions.is_empty()
            || delta
                .occurrences
                .iter()
                .any(|o| o.role == OccurrenceRole::Reference)
        {
            return Err(FactsError::Phase);
        }
        // Every exported header declaration is newly accepted in this header.
        for occurrence in &delta.occurrences {
            if occurrence.role == OccurrenceRole::Export {
                b.charge(Resource::Work, delta.entities.len() as u64 + 1)?;
                if !matches!(occurrence.resolution,
                    nepl3_core::facts::ReferenceResolution::Resolved(id)
                    if delta.entities.iter().any(|e| e.id == id))
                {
                    return Err(FactsError::Phase);
                }
            }
        }
    }
    Ok(())
}
