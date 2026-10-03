//! Candidate-side Field Reference correspondence in one complete native execution.
//! This does not establish cross-revision declaration identity or accept an edit.
use super::*;
use crate::{
    analysis::{probe::read, trace::BoundReferenceTrace},
    binding::{
        BindingError, BindingOutcome,
        trace::{FinalReference, ReferenceIssuance, TraceAccessError},
    },
    facts::FactsError,
    package::{Binding, NameSelector},
    profile::ProfileError,
    selection::ShapeSelection,
};
use nepl3_core::{budget::Resource, syntax::FieldValue};

#[derive(Debug)]
pub enum ReferenceError {
    Access(BindingAccessError),
    Read(read::ReadError),
    Facts(FactsError),
    Profile(ProfileError),
    Trace(TraceAccessError),
    Stopped(StopReason),
    ProofMismatch,
    Structure,
}
impl From<StopReason> for ReferenceError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
/// A privately issued association retains the checked insertion and this exact trace.
pub struct MatchedReference<'a, 'tree, 'p> {
    insertion: &'a name::CheckedNameInsertion<'a, 'tree, 'p>,
    trace: &'a BoundReferenceTrace<'a, 'p>,
    index: usize,
    final_reference: FinalReference<'a>,
}
impl<'a, 'tree, 'p> MatchedReference<'a, 'tree, 'p> {
    pub fn insertion(&self) -> &'a name::CheckedNameInsertion<'a, 'tree, 'p> {
        self.insertion
    }
    pub fn trace(&self) -> &'a BoundReferenceTrace<'a, 'p> {
        self.trace
    }
    pub fn issuance(&self) -> &ReferenceIssuance {
        &self.trace.trace().rows()[self.index]
    }
    pub fn final_reference(&self) -> &FinalReference<'a> {
        &self.final_reference
    }
}
pub enum ReferenceOutcome<'a, 'tree, 'p> {
    Unique(MatchedReference<'a, 'tree, 'p>),
    NoMatch,
    Multiple,
    Invalid(&'a BindingError),
    Stopped(StopReason),
}
/// Exhaust both structural reachability and action matches before issuing uniqueness.
pub fn correlate<'a, 'tree, 'p>(
    insertion: &'a name::CheckedNameInsertion<'a, 'tree, 'p>,
    prepared: &PreparedBindingRequest<'_, 'p>,
    trace: &'a BoundReferenceTrace<'a, 'p>,
    b: &mut Budget,
) -> Result<ReferenceOutcome<'a, 'tree, 'p>, ReferenceError> {
    let checked = insertion.checked();
    if b.limits() != checked.limits() || b.limits() != prepared.limits {
        return Err(ReferenceError::Access(BindingAccessError::LimitsMismatch));
    }
    let native = trace
        .for_key(&checked.keys().1, b)
        .map_err(ReferenceError::Access)?;
    b.charge(Resource::Work, 129)?;
    if prepared.key() != checked.keys().1 {
        return Err(ReferenceError::Access(BindingAccessError::StaleAnalysis));
    }
    let tree = checked.candidate().execution().tree();
    let profile = checked.candidate().seed().profile();
    if !core::ptr::eq(tree, native.tree())
        || !core::ptr::eq(tree, prepared.tree.tree())
        || !core::ptr::eq(profile, native.profile())
        || !core::ptr::eq(profile, prepared.profile)
    {
        return Err(ReferenceError::ProofMismatch);
    }
    match &native.reply().outcome {
        BindingOutcome::Invalid { error, .. } => return Ok(ReferenceOutcome::Invalid(error)),
        BindingOutcome::Stopped { reason, .. } => return Ok(ReferenceOutcome::Stopped(*reason)),
        BindingOutcome::Complete(_) => {}
    }
    b.with_depth(|b| {
        let ExpectedReadOrigin::Field {
            field,
            owner: original_package,
            ..
        } = &insertion.choice().read().expected().origin
        else {
            return Err(ReferenceError::Structure);
        };
        let mut bundle = &tree.bundle;
        let mut node = bundle.root;
        let mut parent = None;
        for (depth, step) in checked.path().iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            b.charge(Resource::Nodes, 1)?;
            b.observe_depth(depth as u64 + 1)?;
            let index = match step {
                ExpectedReadStep::Child { field } | ExpectedReadStep::Foreign { field } => *field,
            };
            let value = bundle
                .nodes
                .get(usize::try_from(node.0).map_err(|_| ReferenceError::Structure)?)
                .and_then(|v| usize::try_from(index).ok().and_then(|i| v.fields.get(i)))
                .ok_or(ReferenceError::Structure)?;
            parent = Some((bundle, node, index));
            match (step, value) {
                (ExpectedReadStep::Child { .. }, FieldValue::Child(child)) => node = *child,
                (ExpectedReadStep::Foreign { .. }, FieldValue::Foreign(foreign)) => {
                    bundle = &foreign.bundle;
                    node = bundle.root;
                }
                _ => return Err(ReferenceError::Structure),
            }
        }
        let (owner_bundle, owner_node, index) = parent.ok_or(ReferenceError::Structure)?;
        if index != *field {
            return Err(ReferenceError::Structure);
        }
        read::unique_path(&tree.bundle, bundle, node, b).map_err(ReferenceError::Read)?;
        let (selected, owner_path) =
            read::context(prepared, owner_bundle, owner_node, b).map_err(ReferenceError::Read)?;
        let (_, target_path) =
            read::context(prepared, bundle, node, b).map_err(ReferenceError::Read)?;
        b.charge(
            Resource::Work,
            selected.entry.package.schema.package.len() as u64
                + original_package.schema.package.len() as u64
                + 80,
        )?;
        if &selected.entry.package != original_package {
            return Err(ReferenceError::ProofMismatch);
        }
        let package = profile
            .language(&selected.entry.alias, b)
            .map_err(ReferenceError::Profile)?;
        let binding_id = insertion.choice().read().hit().site().binding;
        let binding = package
            .bindings
            .get(usize::try_from(binding_id.0).map_err(|_| ReferenceError::Structure)?)
            .ok_or(ReferenceError::Structure)?;
        let Binding::Reference {
            name: NameSelector::Field(name),
            ..
        } = binding
        else {
            return Err(ReferenceError::Structure);
        };
        let fields = match &selected.shape {
            ShapeSelection::Form { index } => {
                &package
                    .forms
                    .get(usize::try_from(*index).map_err(|_| ReferenceError::Structure)?)
                    .ok_or(ReferenceError::Structure)?
                    .fields
            }
            ShapeSelection::Dynamic { shape, .. } => &shape.fields,
            _ => return Err(ReferenceError::Structure),
        };
        let actual = fields
            .get(usize::try_from(index).map_err(|_| ReferenceError::Structure)?)
            .ok_or(ReferenceError::Structure)?;
        b.charge(
            Resource::Work,
            actual.name.len() as u64 + name.len() as u64 + 1,
        )?;
        if actual.name != *name {
            return Err(ReferenceError::Structure);
        }
        let owner =
            crate::facts::phase::target(tree, owner_path, owner_node, profile.registry(), b)
                .map_err(ReferenceError::Facts)?;
        let target = crate::facts::phase::target(tree, target_path, node, profile.registry(), b)
            .map_err(ReferenceError::Facts)?;
        let mut found = None;
        let mut multiple = false;
        for (index, row) in native.rows().iter().enumerate() {
            b.charge(Resource::Nodes, 1)?;
            b.charge(
                Resource::Work,
                row.package.schema.package.len() as u64
                    + selected.entry.package.schema.package.len() as u64
                    + 120,
            )?;
            if row.binding == binding_id
                && row.package == selected.entry.package
                && row.execution_digest == selected.execution_digest
                && read::target_equal(&row.owner, &owner, b).map_err(ReferenceError::Read)?
                && read::target_equal(&row.name_target, &target, b).map_err(ReferenceError::Read)?
            {
                if found.is_some() {
                    multiple = true;
                } else {
                    found = Some(index);
                }
            }
        }
        if multiple {
            return Ok(ReferenceOutcome::Multiple);
        }
        let Some(index) = found else {
            return Ok(ReferenceOutcome::NoMatch);
        };
        let final_reference = native
            .final_reference(index, b)
            .map_err(ReferenceError::Trace)?;
        Ok(ReferenceOutcome::Unique(MatchedReference {
            insertion,
            trace,
            index,
            final_reference,
        }))
    })
}

impl ReferenceError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(reason)
            | Self::Access(BindingAccessError::Stopped(reason))
            | Self::Profile(ProfileError::Stopped(reason))
            | Self::Trace(TraceAccessError::Stopped(reason)) => Some(*reason),
            Self::Read(error) => error.stop_reason(),
            Self::Facts(error) => error.stop_reason(),
            _ => None,
        }
    }
}
