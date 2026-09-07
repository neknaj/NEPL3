//! Map authority follows the executing syntax bundle, never a flattened table.
use super::*;
use crate::binding::BindingAnalysis;
use nepl3_core::{
    facts::{NamespaceRef, ScopeId},
    origin::Mapping,
    syntax::canonical::{BundleMappings, CanonicalError},
};
pub(super) struct Owners {
    rows: Vec<(ScopeId, Vec<Mapping>)>,
}
impl Owners {
    pub(super) fn new(
        prepared: &PreparedBindingRequest<'_, '_>,
        analysis: &BindingAnalysis,
        b: &mut Budget,
    ) -> Result<Self, RenameError> {
        let canonical =
            BundleMappings::new(&prepared.tree.tree().bundle, b).map_err(|e| match e {
                CanonicalError::Stopped(r) => RenameError::Stopped(r),
                _ => RenameError::RequestMismatch,
            })?;
        let ledger = &analysis.result().bundle_scopes;
        if canonical.entries().len() != ledger.len() {
            return Err(RenameError::RequestMismatch);
        }
        let mut rows = Vec::new();
        for (index, entry) in canonical.entries().iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            let owner = &ledger[index];
            if owner.bundle != index as u64 {
                return Err(RenameError::RequestMismatch);
            }
            let mut maps = Vec::new();
            for map in &entry.bundle().source_maps {
                push(&mut maps, map.clone_with_budget(b)?, b)?;
            }
            for index in &owner.custom_source_maps {
                b.charge(Resource::Work, 1)?;
                let map = usize::try_from(*index)
                    .ok()
                    .and_then(|i| analysis.result().source_maps.get(i))
                    .ok_or(RenameError::RequestMismatch)?;
                push(&mut maps, map.clone_with_budget(b)?, b)?;
            }
            push(&mut rows, (owner.scope, maps), b)?;
        }
        Ok(Self { rows })
    }
    pub(super) fn namespace(
        &self,
        facts: &FactSet,
        ns: NamespaceRef,
        b: &mut Budget,
    ) -> Result<&[Mapping], RenameError> {
        b.charge(Resource::Work, self.rows.len() as u64 + 1)?;
        let ns = facts
            .namespaces
            .get(ns.0 as usize)
            .ok_or(RenameError::RequestMismatch)?;
        self.rows
            .iter()
            .find(|(root, _)| *root == ns.root)
            .map(|(_, maps)| maps.as_slice())
            .ok_or(RenameError::RequestMismatch)
    }
    pub(super) fn bundle(&self, index: usize) -> Result<&[Mapping], RenameError> {
        self.rows
            .get(index)
            .map(|(_, maps)| maps.as_slice())
            .ok_or(RenameError::RequestMismatch)
    }
}
