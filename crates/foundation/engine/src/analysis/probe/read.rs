//! Fail-closed correspondence of a native first hit to one structural Missing read.
use super::{BoundProbeReply, ProbeAccessError, ProbePosition};
use crate::{
    analysis::{BindingAccessError, PreparedBindingRequest, expected::*},
    binding::{BindingError, probe::MissingReference},
    facts::FactsError,
    package::{Binding, NameSelector},
    profile::ProfileError,
    recovery::ForeignStep,
    selection::{NodeSelection, ShapeSelection},
    tree::TreeError,
};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    source::SourceAdmission,
    syntax::{
        FieldValue, NodeRef, SyntaxBundle,
        canonical::{BundleMappings, CanonicalError},
    },
};

pub struct ReadCorrespondence<'a, 'tree> {
    reply: &'a BoundProbeReply<'tree>,
    hit: &'a MissingReference<'tree>,
    expected: Box<ExpectedRead>,
}
impl<'a, 'tree> ReadCorrespondence<'a, 'tree> {
    pub fn reply(&self) -> &'a BoundProbeReply<'tree> {
        self.reply
    }
    pub fn limits(&self) -> nepl3_core::budget::Limits {
        self.reply.limits
    }

    pub fn hit(&self) -> &'a MissingReference<'tree> {
        self.hit
    }
    pub fn expected(&self) -> &ExpectedRead {
        &self.expected
    }
}
pub enum ReadOutcome<'a, 'tree> {
    Hit(ReadCorrespondence<'a, 'tree>),
    OtherHit(&'a MissingReference<'tree>),
    NoHit,
    Blocked(&'a BindingError),
    Stopped(StopReason),
}
pub struct ReadReply<'a, 'tree> {
    outcome: ReadOutcome<'a, 'tree>,
    report: Report,
}
impl<'a, 'tree> ReadReply<'a, 'tree> {
    pub fn outcome(&self) -> &ReadOutcome<'a, 'tree> {
        &self.outcome
    }
    pub fn report(&self) -> &Report {
        &self.report
    }
}
#[derive(Debug)]
pub enum ReadError {
    Access(ProbeAccessError),
    Expected(ExpectedReadError),
    Tree(TreeError),
    Facts(FactsError),
    Profile(ProfileError),
    Canonical(CanonicalError),
    Stopped(StopReason),
    ProofMismatch,
    DifferentTarget,
    AmbiguousTarget,
    Structure,
}
impl From<StopReason> for ReadError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
fn tree_error(v: TreeError) -> ReadError {
    match v {
        TreeError::Stopped(s) => s.into(),
        v => ReadError::Tree(v),
    }
}
fn facts_error(v: FactsError) -> ReadError {
    match v.stop_reason() {
        Some(reason) => reason.into(),
        None => ReadError::Facts(v),
    }
}
fn canonical_error(v: CanonicalError) -> ReadError {
    match v {
        CanonicalError::Stopped(s) => s.into(),
        v => ReadError::Canonical(v),
    }
}
fn profile_error(v: ProfileError) -> ReadError {
    match v {
        ProfileError::Stopped(s) => s.into(),
        v => ReadError::Profile(v),
    }
}

/// This proves only one shared syntax target, not accepted insertion semantics.
/// Native repeated actions remain separate from structural occurrence identity.
pub fn correlate<'a, 'tree>(
    reply: &'a BoundProbeReply<'tree>,
    prepared: &PreparedBindingRequest<'_, '_>,
    request: &ExpectedReadRequest,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<ReadReply<'a, 'tree>, ReadError> {
    if b.limits() != prepared.limits || b.limits() != reply.limits {
        return Err(ReadError::Access(ProbeAccessError::Access(
            BindingAccessError::LimitsMismatch,
        )));
    }
    let position = reply
        .for_position(&request.key, &request.source, request.offset, b, admission)
        .map_err(ReadError::Access)?;
    b.charge(Resource::Work, 128)?;
    if prepared.key() != request.key {
        return Err(ReadError::Access(ProbeAccessError::Access(
            BindingAccessError::StaleAnalysis,
        )));
    }
    if !core::ptr::eq(reply.tree, prepared.tree.tree()) {
        return Err(ReadError::ProofMismatch);
    }
    let outcome = match position {
        ProbePosition::HitAtPosition(hit) => {
            let expected = match expected_read(prepared, request, b, admission).outcome {
                ExpectedReadOutcome::Complete(Some(value)) => value,
                ExpectedReadOutcome::Complete(None) => return Err(ReadError::DifferentTarget),
                ExpectedReadOutcome::Invalid(error) => return Err(ReadError::Expected(error)),
                ExpectedReadOutcome::Stopped(reason) => return Err(reason.into()),
            };
            b.with_depth(|b| check_target(prepared, hit, &expected, b))?;
            ReadOutcome::Hit(ReadCorrespondence {
                reply,
                hit,
                expected,
            })
        }
        ProbePosition::OtherHit(hit) => ReadOutcome::OtherHit(hit),
        ProbePosition::NoHit => ReadOutcome::NoHit,
        ProbePosition::Blocked(error) => ReadOutcome::Blocked(error),
        ProbePosition::Stopped(reason) => ReadOutcome::Stopped(reason),
    };
    Ok(ReadReply {
        outcome,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
    })
}
fn local<'a>(
    maps: &BundleMappings<'a>,
    bundle: u64,
    node: u64,
) -> Result<(&'a SyntaxBundle, NodeRef), ReadError> {
    let map = maps
        .entries()
        .get(usize::try_from(bundle).map_err(|_| ReadError::Structure)?)
        .ok_or(ReadError::Structure)?;
    let local = *map
        .order()
        .get(usize::try_from(node).map_err(|_| ReadError::Structure)?)
        .ok_or(ReadError::Structure)?;
    Ok((map.bundle(), NodeRef(local as u64)))
}
pub(crate) fn context<'a>(
    prepared: &'a PreparedBindingRequest<'_, '_>,
    bundle: &SyntaxBundle,
    node: NodeRef,
    b: &mut Budget,
) -> Result<(&'a NodeSelection, &'a [ForeignStep]), ReadError> {
    for context in &prepared.tree.tree().contexts {
        b.charge(Resource::Work, 1)?;
        let owner = crate::tree::path(
            &prepared.tree.tree().bundle,
            &context.path,
            prepared.profile.registry(),
            b,
        )
        .map_err(tree_error)?;
        if core::ptr::eq(owner, bundle) {
            for selected in &context.nodes {
                b.charge(Resource::Work, 1)?;
                if selected.node == node {
                    return Ok((selected, &context.path));
                }
            }
            break;
        }
    }
    Err(ReadError::Structure)
}
fn check_target(
    prepared: &PreparedBindingRequest<'_, '_>,
    hit: &MissingReference<'_>,
    expected: &ExpectedRead,
    b: &mut Budget,
) -> Result<(), ReadError> {
    let tree = prepared.tree.tree();
    let maps = BundleMappings::new(&tree.bundle, b).map_err(canonical_error)?;
    let (target_bundle, target_node) = local(&maps, expected.bundle, expected.node)?;
    let (_, target_path) = context(prepared, target_bundle, target_node, b)?;
    let target = crate::facts::phase::target(
        tree,
        target_path,
        target_node,
        prepared.profile.registry(),
        b,
    )
    .map_err(facts_error)?;
    if !target_equal(&target, &hit.site().name_target, b)? {
        return Err(ReadError::DifferentTarget);
    }
    let ExpectedReadOrigin::Field {
        parent_bundle,
        parent_node,
        field,
        owner,
        declared,
        resolved_read,
        foreign,
    } = &expected.origin
    else {
        return Err(ReadError::DifferentTarget);
    };
    let (owner_bundle, owner_node) = local(&maps, *parent_bundle, *parent_node)?;
    let (selected, owner_path) = context(prepared, owner_bundle, owner_node, b)?;
    let actual_owner =
        crate::facts::phase::target(tree, owner_path, owner_node, prepared.profile.registry(), b)
            .map_err(facts_error)?;
    b.charge(
        Resource::Work,
        (selected.entry.package.schema.package.len() as u64)
            .saturating_add(owner.schema.package.len() as u64)
            .saturating_add(80),
    )?;
    if !target_equal(&actual_owner, &hit.site().owner, b)? || &selected.entry.package != owner {
        return Err(ReadError::DifferentTarget);
    }
    let package = prepared
        .profile
        .language(&selected.entry.alias, b)
        .map_err(profile_error)?;
    let binding = package
        .bindings
        .get(usize::try_from(hit.site().binding.0).map_err(|_| ReadError::Structure)?)
        .ok_or(ReadError::Structure)?;
    let Binding::Reference {
        name: NameSelector::Field(name),
        ..
    } = binding
    else {
        return Err(ReadError::DifferentTarget);
    };
    let fields = match &selected.shape {
        ShapeSelection::Form { index } => {
            &package
                .forms
                .get(usize::try_from(*index).map_err(|_| ReadError::Structure)?)
                .ok_or(ReadError::Structure)?
                .fields
        }
        ShapeSelection::Dynamic { shape, .. } => &shape.fields,
        _ => return Err(ReadError::DifferentTarget),
    };
    let field_index = usize::try_from(*field).map_err(|_| ReadError::Structure)?;
    let selected_field = fields.get(field_index).ok_or(ReadError::Structure)?;
    b.charge(
        Resource::Work,
        name.len() as u64 + selected_field.name.len() as u64 + 1,
    )?;
    if selected_field.name != *name {
        return Err(ReadError::DifferentTarget);
    }
    let actual_declared = crate::tree::read::declared(selected, field_index, prepared.profile, b)
        .map_err(tree_error)?;
    let actual =
        crate::tree::read::child(selected, field_index, prepared.profile, b).map_err(tree_error)?;
    if actual_declared != *declared
        || actual.read != *resolved_read
        || actual.foreign != *foreign
        || !crate::selection::entry_equal(&actual.entry, &expected.expected, b)?
    {
        return Err(ReadError::DifferentTarget);
    }
    let node = owner_bundle
        .nodes
        .get(owner_node.0 as usize)
        .ok_or(ReadError::Structure)?;
    let same = match node.fields.get(field_index).ok_or(ReadError::Structure)? {
        FieldValue::Child(node) => {
            !*foreign && core::ptr::eq(owner_bundle, target_bundle) && *node == target_node
        }
        FieldValue::Foreign(value) => {
            *foreign
                && core::ptr::eq(&value.bundle, target_bundle)
                && value.bundle.root == target_node
        }
        _ => false,
    };
    if !same {
        return Err(ReadError::DifferentTarget);
    }
    unique_path(&tree.bundle, target_bundle, target_node, b)
}
pub(crate) fn unique_path(
    root: &SyntaxBundle,
    target_bundle: &SyntaxBundle,
    target: NodeRef,
    b: &mut Budget,
) -> Result<(), ReadError> {
    struct Pending<'a> {
        bundle: &'a SyntaxBundle,
        node: NodeRef,
        depth: u64,
    }
    fn push<'a>(
        pending: &mut Vec<Pending<'a>>,
        value: Pending<'a>,
        b: &mut Budget,
    ) -> Result<(), ReadError> {
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Pending>() as u64,
        )?;
        pending.push(value);
        Ok(())
    }
    let mut pending = Vec::new();
    push(
        &mut pending,
        Pending {
            bundle: root,
            node: root.root,
            depth: 1,
        },
        b,
    )?;
    let mut found = false;
    while let Some(current) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        b.charge(Resource::Nodes, 1)?;
        b.observe_depth(current.depth)?;
        if core::ptr::eq(current.bundle, target_bundle) && current.node == target {
            if found {
                return Err(ReadError::AmbiguousTarget);
            }
            found = true;
        }
        let node = current
            .bundle
            .nodes
            .get(usize::try_from(current.node.0).map_err(|_| ReadError::Structure)?)
            .ok_or(ReadError::Structure)?;
        for field in &node.fields {
            b.charge(Resource::Work, 1)?;
            let (bundle, node) = match field {
                FieldValue::Child(child) => (current.bundle, *child),
                FieldValue::Foreign(value) => (&value.bundle, value.bundle.root),
                FieldValue::Atom(_) => continue,
                FieldValue::Children(_) => return Err(ReadError::Structure),
            };
            push(
                &mut pending,
                Pending {
                    bundle,
                    node,
                    depth: current.depth.saturating_add(1),
                },
                b,
            )?;
        }
    }
    if found {
        Ok(())
    } else {
        Err(ReadError::DifferentTarget)
    }
}

pub(crate) fn target_equal(
    a: &crate::binding::CanonicalBindingTarget,
    c: &crate::binding::CanonicalBindingTarget,
    b: &mut Budget,
) -> Result<bool, ReadError> {
    b.charge(Resource::Work, 1)?;
    if a.node != c.node || a.path.len() != c.path.len() {
        return Ok(false);
    }
    for (a, c) in a.path.iter().zip(&c.path) {
        b.charge(
            Resource::Work,
            (a.field.len() as u64)
                .saturating_add(c.field.len() as u64)
                .saturating_add(9),
        )?;
        if a != c {
            return Ok(false);
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn target_comparison_accounts_for_long_fields_and_nested_stops() {
        let target = crate::binding::CanonicalBindingTarget {
            path: alloc::vec![ForeignStep {
                node: NodeRef(0),
                field: "x".repeat(2048)
            }],
            node: NodeRef(1),
        };
        let mut limits = nepl3_core::budget::Limits {
            work: 1 + 2048 * 2 + 9,
            ..Default::default()
        };
        assert!(matches!(
            target_equal(&target, &target, &mut Budget::new(limits)),
            Ok(true)
        ));
        limits.work -= 1;
        assert!(matches!(
            target_equal(&target, &target, &mut Budget::new(limits)),
            Err(ReadError::Stopped(StopReason::WorkLimit))
        ));
        assert!(matches!(
            facts_error(FactsError::Tree(TreeError::Stopped(StopReason::WorkLimit))),
            ReadError::Stopped(StopReason::WorkLimit)
        ));
    }
}

impl ReadError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Stopped(reason)
            | Self::Access(ProbeAccessError::Access(BindingAccessError::Stopped(reason))) => {
                Some(*reason)
            }
            Self::Expected(error) => error.stop_reason(),
            _ => None,
        }
    }
}

#[cfg(test)]
mod traversal_tests {
    use super::*;
    fn node(children: &[u64]) -> nepl3_core::syntax::SyntaxNode {
        nepl3_core::syntax::SyntaxNode {
            schema: nepl3_core::value::SchemaRef {
                package: "test.counter".into(),
                revision: 1,
                digest: nepl3_core::source::Digest::of(b"counter"),
            },
            kind: "Node".into(),
            fields: children
                .iter()
                .map(|v| FieldValue::Child(NodeRef(*v)))
                .collect(),
            head: None,
            cover: None,
            origin: nepl3_core::origin::OriginId(0),
            token: None,
        }
    }
    #[test]
    fn structural_counter_rejects_shared_owners_ancestors_and_name_targets() {
        // Algorithm fixtures deliberately isolate reachability from actual action count.
        // Owner 2, its ancestor 1, or just target 3 can be shared independently.
        for nodes in [
            alloc::vec![node(&[1, 1]), node(&[2]), node(&[3]), node(&[])],
            alloc::vec![node(&[1, 2]), node(&[3]), node(&[3]), node(&[])],
            alloc::vec![node(&[1, 2]), node(&[2]), node(&[3]), node(&[])],
        ] {
            let bundle = SyntaxBundle {
                sources: Vec::new(),
                nodes,
                origins: Vec::new(),
                root: NodeRef(0),
                environments: Vec::new(),
                tokens: Vec::new(),
                source_maps: Vec::new(),
            };
            let limits = nepl3_core::budget::Limits {
                work: 100_000,
                nodes: 100_000,
                allocation_units: 100_000,
                depth: 64,
                ..Default::default()
            };
            assert!(matches!(
                unique_path(&bundle, &bundle, NodeRef(3), &mut Budget::new(limits)),
                Err(ReadError::AmbiguousTarget)
            ));
        }
    }
    #[test]
    fn structural_counter_ignores_unrelated_sharing_and_checks_after_first_match() {
        // This is an algorithm-only DAG fixture, not a prepared parse proof.
        let bundle = SyntaxBundle {
            sources: Vec::new(),
            nodes: alloc::vec![node(&[1, 1, 2]), node(&[]), node(&[])],
            origins: Vec::new(),
            root: NodeRef(0),
            environments: Vec::new(),
            tokens: Vec::new(),
            source_maps: Vec::new(),
        };
        let limits = nepl3_core::budget::Limits {
            work: 100_000,
            nodes: 100_000,
            allocation_units: 100_000,
            depth: 64,
            ..Default::default()
        };
        let mut b = Budget::new(limits);
        assert!(unique_path(&bundle, &bundle, NodeRef(2), &mut b).is_ok());
        let usage = b.usage();
        // LIFO visits target 2 first; exhaustion later must not certify uniqueness.
        let mut limited = Budget::new(limits);
        assert!(
            limited
                .charge(Resource::Nodes, limits.nodes - usage.nodes + 1)
                .is_ok()
        );
        assert!(matches!(
            unique_path(&bundle, &bundle, NodeRef(2), &mut limited),
            Err(ReadError::Stopped(StopReason::NodeLimit))
        ));
        assert!(matches!(
            unique_path(&bundle, &bundle, NodeRef(1), &mut Budget::new(limits)),
            Err(ReadError::AmbiguousTarget)
        ));
    }
}
