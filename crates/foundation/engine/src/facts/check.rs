use super::*;
use crate::profile::ResolvedParseProfile;
use nepl3_core::{
    budget::{Budget, Resource},
    diagnostic::validation::ReportValidationError,
    facts::FactError,
    origin::{Mapping, OriginError, SourceMap},
    source::{SourceAdmission, SourceError, SourceSnapshot, SourceStore},
    syntax::FieldValue,
};

#[derive(Debug)]
pub enum FactsError {
    Stopped(StopReason),
    Tree(crate::tree::TreeError),
    Fact(FactError),
    Source(SourceError),
    Origin(OriginError),
    Report(ReportValidationError),
    Target,
}
macro_rules! error_from {
    ($ty:ty,$case:ident) => {
        impl From<$ty> for FactsError {
            fn from(v: $ty) -> Self {
                Self::$case(v)
            }
        }
    };
}
error_from!(StopReason, Stopped);
error_from!(crate::tree::TreeError, Tree);
error_from!(FactError, Fact);
error_from!(SourceError, Source);
error_from!(OriginError, Origin);
error_from!(ReportValidationError, Report);

/// A trusted host issues this proof for the exact request it selected. Public
/// transport decoding cannot manufacture it from a received authority field.
/// The borrowed profile fixes both semantic and concrete execution identities.
pub struct CheckedFactsRequest<'a, 'profile> {
    request: &'a FactsRequest,
    profile: &'a ResolvedParseProfile<'profile>,
}
impl<'a, 'profile> CheckedFactsRequest<'a, 'profile> {
    pub fn request(&self) -> &'a FactsRequest {
        self.request
    }
    pub fn profile(&self) -> &'a ResolvedParseProfile<'profile> {
        self.profile
    }
}
impl FactsRequest {
    /// Host issuance boundary, not authentication of provider-supplied grants.
    /// A receiver must authenticate its transport and authorize the operation
    /// before issuing a proof for a decoded request. Validation alone is not
    /// sufficient authority to execute a provider.
    pub fn issue<'a, 'profile>(
        &'a self,
        profile: &'a ResolvedParseProfile<'profile>,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<CheckedFactsRequest<'a, 'profile>, FactsError> {
        self.validate(profile, b, admission)?;
        Ok(CheckedFactsRequest {
            request: self,
            profile,
        })
    }
    /// Checks data invariants only; does not authenticate or grant execution.
    pub fn validate(
        &self,
        profile: &ResolvedParseProfile<'_>,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), FactsError> {
        let result = (|| {
            self.tree.validate(profile, b, admission)?;
            let owner = crate::tree::path(&self.tree.bundle, &self.path, profile.registry(), b)?;
            b.charge(Resource::Work, 1)?;
            if usize::try_from(self.node.0)
                .ok()
                .and_then(|i| owner.nodes.get(i))
                .is_none()
            {
                return Err(FactsError::Target);
            }
            let base = self.existing.validate(profile.registry(), b, admission)?;
            self.authority.validate(&base, b, admission)?;
            // Independently valid tables must also agree on shared identities.
            closure(self, None, &[], &[], b, admission)?;
            Ok(())
        })();
        result.map_err(|e| match b.poll() {
            Err(r) => FactsError::Stopped(r),
            Ok(()) => e,
        })
    }
}
impl FactsReply {
    pub fn validate(
        &self,
        request: &CheckedFactsRequest<'_, '_>,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), FactsError> {
        let result = (|| {
            b.poll()?;
            let (delta, report, sources, maps) = self.parts();
            if report.trace_overflow.is_some() && !matches!(self, Self::Stopped { .. }) {
                return Err(FactsError::Report(ReportValidationError::Usage));
            }
            let base =
                request
                    .request
                    .existing
                    .validate(request.profile.registry(), b, admission)?;
            request.request.authority.validate(&base, b, admission)?;
            if let Some(delta) = delta {
                delta.validate(&base, &request.request.authority, b, admission)?;
            }
            let store = closure(request.request, delta, sources, maps, b, admission)?;
            report.validate(&store, &[], request.profile.registry(), b)?;
            Ok(())
        })();
        result.map_err(|e| match b.poll() {
            Err(r) => FactsError::Stopped(r),
            Ok(()) => e,
        })
    }
    pub(crate) fn parts(&self) -> (Option<&FactDelta>, &Report, &[SourceSnapshot], &[Mapping]) {
        match self {
            Self::Complete {
                delta,
                report,
                sources,
                source_maps,
            } => (Some(delta), report, sources, source_maps),
            Self::Invalid {
                partial,
                report,
                sources,
                source_maps,
            }
            | Self::Stopped {
                partial,
                report,
                sources,
                source_maps,
                ..
            } => (partial.as_ref(), report, sources, source_maps),
        }
    }
}
fn push<T>(v: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), FactsError> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(item);
    Ok(())
}
pub(crate) fn closure(
    request: &FactsRequest,
    delta: Option<&FactDelta>,
    added: &[SourceSnapshot],
    maps: &[Mapping],
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<SourceStore, FactsError> {
    closure_for(
        &request.tree,
        &request.existing,
        delta,
        added,
        maps,
        b,
        admission,
    )
}
pub(crate) fn closure_for(
    tree: &ParseTree,
    existing: &FactSet,
    delta: Option<&FactDelta>,
    added: &[SourceSnapshot],
    maps: &[Mapping],
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<SourceStore, FactsError> {
    // Cross-table sharing is allowed; duplicate entries in this single reply
    // declaration table are not. Match SourceContent's native/wire invariant.
    for (i, source) in added.iter().enumerate() {
        for prior in &added[..i] {
            b.charge(
                Resource::Work,
                (source.identity().source.0.len() as u64)
                    .saturating_add(prior.identity().source.0.len() as u64)
                    .saturating_add(34),
            )?;
            if source.identity().source == prior.identity().source
                && source.identity().revision == prior.identity().revision
            {
                return Err(SourceError::IdentityConflict.into());
            }
        }
    }
    let mut store = SourceStore::default();
    let mut mappings = Vec::new();
    fn add(
        store: &mut SourceStore,
        sources: &[SourceSnapshot],
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<(), FactsError> {
        for source in sources {
            admission.admit_existing(source, b)?;
            b.charge(
                Resource::Work,
                (store.snapshots().len() as u64 + 1)
                    .saturating_mul(source.identity().source.0.len() as u64 + 1),
            )?;
            if store.get_ref(source.identity()).is_none() {
                b.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<SourceSnapshot>() as u64,
                )?;
                store.insert(source.clone_with_budget(b)?)?;
            }
        }
        Ok(())
    }
    let mut pending = Vec::new();
    push(&mut pending, (&tree.bundle, 1u64), b)?;
    while let Some((bundle, depth)) = pending.pop() {
        b.observe_depth(depth)?;
        add(&mut store, &bundle.sources, b, admission)?;
        for map in &bundle.source_maps {
            push(&mut mappings, map.clone_with_budget(b)?, b)?;
        }
        for node in &bundle.nodes {
            for field in &node.fields {
                b.charge(Resource::Work, 1)?;
                if let FieldValue::Foreign(f) = field {
                    push(&mut pending, (&f.bundle, depth.saturating_add(1)), b)?;
                }
            }
        }
    }
    add(&mut store, &existing.sources, b, admission)?;
    for map in &existing.source_maps {
        push(&mut mappings, map.clone_with_budget(b)?, b)?;
    }
    if let Some(delta) = delta {
        add(&mut store, &delta.sources, b, admission)?;
        for map in &delta.source_maps {
            push(&mut mappings, map.clone_with_budget(b)?, b)?;
        }
    }
    add(&mut store, added, b, admission)?;
    for map in maps {
        push(&mut mappings, map.clone_with_budget(b)?, b)?;
    }
    SourceMap::validate_mappings(&mappings, &store, b)?;
    Ok(store)
}
