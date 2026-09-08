//! Validate persistent selections and recovery indices against their concrete owners.
use crate::{
    package::{LanguagePackage, PackageError, ReadSpec},
    profile::{ProfileError, ResolvedParseProfile, ResolvedRead},
    recovery::*,
    selection::*,
};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::{SchemaRegistry, TypeShape},
    source::SourceAdmission,
    syntax::{FieldValue, NodeRef, SyntaxBundle, SyntaxError},
    value::{KindRef, NdfValue},
};
use nepl3_reader::builtin::BuiltinReader;
#[derive(Debug, Eq, PartialEq)]
pub enum TreeError {
    Stopped(StopReason),
    Syntax(SyntaxError),
    Package(PackageError),
    Profile(ProfileError),
    Path,
    Duplicate,
    Selection,
    ExecutionIdentity,
    Recovery,
    Unreachable,
    /// The static validator cannot certify the HeadProvider protocol until its
    /// projected request/reply signatures are implemented and selected.
    UnvalidatedDynamic,
}
impl From<StopReason> for TreeError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<SyntaxError> for TreeError {
    fn from(v: SyntaxError) -> Self {
        match v.stop_reason() {
            Some(reason) => Self::Stopped(reason),
            None => Self::Syntax(v),
        }
    }
}
impl From<PackageError> for TreeError {
    fn from(v: PackageError) -> Self {
        match v {
            PackageError::Stopped(v) => Self::Stopped(v),
            v => Self::Package(v),
        }
    }
}
impl From<ProfileError> for TreeError {
    fn from(v: ProfileError) -> Self {
        match v {
            ProfileError::Stopped(v) => Self::Stopped(v),
            v => Self::Profile(v),
        }
    }
}
pub struct ValidatedParseTree<'a> {
    tree: &'a ParseTree,
    syntax: nepl3_core::syntax::ValidatedSyntaxBundle<'a>,
}
impl ValidatedParseTree<'_> {
    /// Borrow the syntax proof already established for this immutable tree.
    /// This does not certify domain semantics or environment digest equality.
    pub fn syntax(&self) -> &nepl3_core::syntax::ValidatedSyntaxBundle<'_> {
        &self.syntax
    }
    pub fn tree(&self) -> &ParseTree {
        self.tree
    }
    pub fn is_recovered(&self) -> bool {
        self.tree.recovery.iter().any(|b| !b.entries.is_empty())
    }
}
fn push<T>(v: &mut Vec<T>, item: T, budget: &mut Budget) -> Result<(), TreeError> {
    budget.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    v.push(item);
    Ok(())
}
fn node(bundle: &SyntaxBundle, id: NodeRef) -> Result<&nepl3_core::syntax::SyntaxNode, TreeError> {
    usize::try_from(id.0)
        .ok()
        .and_then(|i| bundle.nodes.get(i))
        .ok_or(TreeError::Path)
}
pub(crate) fn path<'a>(
    mut bundle: &'a SyntaxBundle,
    steps: &[ForeignStep],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<&'a SyntaxBundle, TreeError> {
    for (depth, step) in steps.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        budget.observe_depth(depth as u64 + 1)?;
        let node = node(bundle, step.node)?;
        let descriptor = registry.descriptor(&node.schema).ok_or(TreeError::Path)?;
        budget.charge(
            Resource::Work,
            (descriptor.types.len() as u64).saturating_mul(node.kind.len() as u64 + 1),
        )?;
        let ty = descriptor
            .types
            .iter()
            .find(|v| v.name == node.kind)
            .ok_or(TreeError::Path)?;
        let TypeShape::Record { fields } = &ty.shape else {
            return Err(TreeError::Path);
        };
        budget.charge(
            Resource::Work,
            (fields.len() as u64).saturating_mul(step.field.len() as u64 + 1),
        )?;
        let index = fields
            .iter()
            .position(|f| f.name == step.field)
            .ok_or(TreeError::Path)?;
        let Some(FieldValue::Foreign(foreign)) = node.fields.get(index) else {
            return Err(TreeError::Path);
        };
        bundle = &foreign.bundle;
    }
    Ok(bundle)
}
fn same_kind(
    node: &nepl3_core::syntax::SyntaxNode,
    kind: &KindRef,
    registry: &SchemaRegistry,
) -> bool {
    node.schema == kind.schema
        && registry
            .kind_name(&kind.schema, kind.local_kind)
            .is_ok_and(|name| name == node.kind)
}

fn selection<'a>(
    context: &'a BundleContext,
    id: NodeRef,
    budget: &mut Budget,
) -> Result<&'a NodeSelection, TreeError> {
    budget.charge(Resource::Work, context.nodes.len() as u64 + 1)?;
    context
        .nodes
        .iter()
        .find(|v| v.node == id)
        .ok_or(TreeError::Selection)
}
fn token<'a>(
    bundle: &'a SyntaxBundle,
    node: &nepl3_core::syntax::SyntaxNode,
) -> Result<&'a nepl3_core::view::Token, TreeError> {
    node.token
        .and_then(|id| usize::try_from(id.0).ok())
        .and_then(|i| bundle.tokens.get(i))
        .ok_or(TreeError::Selection)
}
fn head_text<'a>(
    bundle: &'a SyntaxBundle,
    node: &nepl3_core::syntax::SyntaxNode,
    budget: &mut Budget,
) -> Result<&'a str, TreeError> {
    let head = &token(bundle, node)?.head;
    budget.charge(
        Resource::Work,
        (bundle.sources.len() as u64)
            .saturating_mul(head.snapshot_ref().source.0.len() as u64 + 41),
    )?;
    let source = bundle
        .sources
        .iter()
        .find(|s| s.identity() == head.snapshot_ref())
        .ok_or(TreeError::Selection)?;
    let raw = source
        .slice(head)
        .map_err(|e| TreeError::from(SyntaxError::from(e)))?;
    budget.charge(Resource::Work, raw.len() as u64 + 1)?;
    Ok(raw)
}
fn spelling(
    bundle: &SyntaxBundle,
    node: &nepl3_core::syntax::SyntaxNode,
    expected: &str,
    budget: &mut Budget,
) -> Result<(), TreeError> {
    if head_text(bundle, node, budget)? != expected {
        return Err(TreeError::Selection);
    }
    Ok(())
}
fn target(
    expected: &ResolvedRead,
    selected: &NodeSelection,
    package: &LanguagePackage,
    budget: &mut Budget,
) -> Result<(), TreeError> {
    budget.charge(
        Resource::Work,
        (selected.entry.alias.len()
            + selected.entry.category.len()
            + selected.entry.mode.len()
            + selected.entry.package.schema.package.len()) as u64
            + 65,
    )?;
    if expected.entry != selected.entry {
        return Err(TreeError::Selection);
    }
    if matches!(selected.shape, ShapeSelection::Recovery) {
        return Ok(());
    }
    let compatible = match expected.read {
        None => matches!(
            selected.shape,
            ShapeSelection::Form { .. }
                | ShapeSelection::Leaf { .. }
                | ShapeSelection::Dynamic { .. }
        ),
        Some(read) => match package.read(read)? {
            ReadSpec::Builtin { .. } => {
                matches!(selected.shape, ShapeSelection::Builtin {read: r} if r == read)
            }
            ReadSpec::ListOf { .. } => {
                matches!(selected.shape, ShapeSelection::List {read: r,..} if r == read)
            }
            _ => false,
        },
    };
    if compatible {
        Ok(())
    } else {
        Err(TreeError::Selection)
    }
}
fn field(
    value: &FieldValue,
    expected: &ResolvedRead,
    bundle: &SyntaxBundle,
    context: &BundleContext,
    contexts: &[(&SyntaxBundle, &BundleContext)],
    package: &LanguagePackage,
    budget: &mut Budget,
) -> Result<(), TreeError> {
    match (expected.foreign, value) {
        (false, FieldValue::Child(id)) => {
            target(expected, selection(context, *id, budget)?, package, budget)
        }
        (true, FieldValue::Foreign(foreign)) => {
            budget.charge(
                Resource::Work,
                contexts.len() as u64
                    + foreign.category.len() as u64
                    + foreign.schema.package.len() as u64
                    + 34,
            )?;
            if foreign.schema != expected.entry.package.schema
                || foreign.category != expected.entry.category
            {
                return Err(TreeError::Selection);
            }
            let context = contexts
                .iter()
                .find(|(b, _)| core::ptr::eq(*b, &foreign.bundle))
                .map(|(_, c)| *c)
                .ok_or(TreeError::Selection)?;
            target(
                expected,
                selection(context, foreign.root, budget)?,
                package,
                budget,
            )
        }
        _ => {
            let _ = bundle;
            Err(TreeError::Selection)
        }
    }
}
fn static_fields(
    bundle: &SyntaxBundle,
    context: &BundleContext,
    contexts: &[(&SyntaxBundle, &BundleContext)],
    selected: &NodeSelection,
    profile: &ResolvedParseProfile<'_>,
    budget: &mut Budget,
) -> Result<(), TreeError> {
    let package = profile.language(&selected.entry.alias, budget)?;
    let node = node(bundle, selected.node)?;
    match selected.shape {
        ShapeSelection::Form { index } => {
            let form = package
                .forms
                .get(usize::try_from(index).map_err(|_| TreeError::Selection)?)
                .ok_or(TreeError::Selection)?;
            spelling(bundle, node, &form.spelling, budget)?;
            for (spec, value) in form.fields.iter().zip(&node.fields) {
                let expected = profile.read_entry(&selected.entry, spec.read, budget)?;
                field(value, &expected, bundle, context, contexts, package, budget)?;
            }
        }
        ShapeSelection::Leaf { index } => {
            let leaf = package
                .leaves
                .get(usize::try_from(index).map_err(|_| TreeError::Selection)?)
                .ok_or(TreeError::Selection)?;
            let token = token(bundle, node)?;
            let raw = head_text(bundle, node, budget)?;
            for form in &package.forms {
                budget.charge(
                    Resource::Work,
                    (selected.entry.category.len() + raw.len()) as u64 + 1,
                )?;
                if form.category == selected.entry.category && form.spelling == raw {
                    return Err(TreeError::Selection);
                }
            }
            budget.charge(Resource::Work, token.kind.schema.package.len() as u64 + 41)?;
            if token.kind != leaf.token_kind {
                return Err(TreeError::Selection);
            }
            profile
                .registry()
                .validate(&leaf.payload, &token.payload, budget)
                .map_err(|e| TreeError::from(SyntaxError::from(e)))?;
        }
        ShapeSelection::Builtin { read } => {
            let ReadSpec::Builtin {
                reader, token_kind, ..
            } = package.read(read)?
            else {
                return Err(TreeError::Selection);
            };
            let token = token(bundle, node)?;
            budget.charge(Resource::Work, token.kind.schema.package.len() as u64 + 41)?;
            if &token.kind != token_kind {
                return Err(TreeError::Selection);
            }
            let valid = match (reader, &token.payload) {
                (
                    BuiltinReader::Name | BuiltinReader::Text | BuiltinReader::Lang,
                    NdfValue::Text(_),
                ) => true,
                (BuiltinReader::Nat, NdfValue::Integer(v)) => !v.is_negative(),
                (BuiltinReader::Number, NdfValue::Rational(_))
                | (BuiltinReader::Trivia, NdfValue::Unit) => true,
                _ => false,
            };
            if !valid {
                return Err(TreeError::Selection);
            }
        }
        ShapeSelection::List { read, cons } => {
            let ReadSpec::ListOf { element, .. } = package.read(read)? else {
                return Err(TreeError::Selection);
            };
            spelling(bundle, node, if cons { "cons" } else { "nil" }, budget)?;
            if cons {
                let expected = profile.read_entry(&selected.entry, *element, budget)?;
                field(
                    &node.fields[0],
                    &expected,
                    bundle,
                    context,
                    contexts,
                    package,
                    budget,
                )?;
                // The tail keeps the resolved spine entry; it does not reapply category defaults.
                budget.charge(
                    Resource::AllocationUnits,
                    (selected.entry.alias.len()
                        + selected.entry.category.len()
                        + selected.entry.mode.len()
                        + selected.entry.package.schema.package.len()) as u64,
                )?;
                let expected = ResolvedRead {
                    entry: selected.entry.clone(),
                    read: Some(read),
                    foreign: false,
                };
                field(
                    &node.fields[1],
                    &expected,
                    bundle,
                    context,
                    contexts,
                    package,
                    budget,
                )?;
            }
        }
        ShapeSelection::Dynamic {
            ref shape,
            ref child_contexts,
            ..
        } => {
            let raw = head_text(bundle, node, budget)?;
            for form in &package.forms {
                budget.charge(
                    Resource::Work,
                    (selected.entry.category.len() + raw.len()) as u64 + 1,
                )?;
                if form.category == selected.entry.category && form.spelling == raw {
                    return Err(TreeError::Selection);
                }
            }
            if child_contexts.len() != shape.fields.len() {
                return Err(TreeError::Selection);
            }
            for ((spec, value), actual) in shape.fields.iter().zip(&node.fields).zip(child_contexts)
            {
                profile.validate_entry(actual, budget)?;
                let mut expected = profile.read_entry(&selected.entry, spec.read, budget)?;
                // A fixed builtin/list slot still names a concrete local shape. A
                // category slot can retarget its category/mode, and a Foreign slot
                // can select a different registered guest without changing the slot.
                if expected.read.is_some() {
                    if actual != &expected.entry {
                        return Err(TreeError::Selection);
                    }
                } else if !expected.foreign && actual.alias != selected.entry.alias {
                    return Err(TreeError::Selection);
                }
                budget.charge(
                    Resource::AllocationUnits,
                    (actual.alias.len()
                        + actual.category.len()
                        + actual.mode.len()
                        + actual.package.schema.package.len()) as u64,
                )?;
                expected.entry = actual.clone();
                field(value, &expected, bundle, context, contexts, package, budget)?;
            }
        }
        _ => {}
    }
    Ok(())
}
impl ParseTree {
    pub fn validate<'a>(
        &'a self,
        profile: &ResolvedParseProfile<'_>,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedParseTree<'a>, TreeError> {
        budget.charge(Resource::Work, 1)?;
        if self.profile_digest != profile.digest() {
            return Err(TreeError::Selection);
        }
        let registry = profile.registry();
        let syntax = self
            .bundle
            .validate_with_sources(registry, budget, admission)?;
        let mut contexts: Vec<(&SyntaxBundle, &BundleContext)> = Vec::new();
        for context in &self.contexts {
            let bundle = path(&self.bundle, &context.path, registry, budget)?;
            budget.charge(Resource::Work, contexts.len() as u64 + 1)?;
            if contexts
                .iter()
                .any(|(prior, _)| core::ptr::eq(*prior, bundle))
            {
                return Err(TreeError::Duplicate);
            }
            push(&mut contexts, (bundle, context), budget)?;
        }
        let mut recoveries = Vec::new();
        for recovery in &self.recovery {
            let bundle = path(&self.bundle, &recovery.path, registry, budget)?;
            budget.charge(Resource::Work, recoveries.len() as u64 + 1)?;
            if recovery.entries.is_empty()
                || recoveries
                    .iter()
                    .any(|(prior, _)| core::ptr::eq(*prior, bundle))
            {
                return Err(TreeError::Duplicate);
            }
            push(&mut recoveries, (bundle, recovery), budget)?;
        }
        let mut pending = Vec::new();
        push(&mut pending, (&self.bundle, 1u64), budget)?;
        while let Some((bundle, depth)) = pending.pop() {
            budget.observe_depth(depth)?;
            budget.charge(
                Resource::Work,
                contexts.len() as u64 + recoveries.len() as u64 + 1,
            )?;
            let context = contexts
                .iter()
                .find(|(b, _)| core::ptr::eq(*b, bundle))
                .map(|(_, c)| *c)
                .ok_or(TreeError::Selection)?;
            let recovery = recoveries
                .iter()
                .find(|(b, _)| core::ptr::eq(*b, bundle))
                .map(|(_, c)| *c);
            budget.charge(Resource::AllocationUnits, bundle.nodes.len() as u64)?;
            let mut reached = alloc::vec![false;bundle.nodes.len()];
            let mut nodes = Vec::new();
            push(&mut nodes, bundle.root, budget)?;
            while let Some(id) = nodes.pop() {
                budget.charge(Resource::Work, 1)?;
                let index = usize::try_from(id.0).map_err(|_| TreeError::Path)?;
                let seen = reached.get_mut(index).ok_or(TreeError::Path)?;
                if *seen {
                    continue;
                }
                *seen = true;
                for field in &node(bundle, id)?.fields {
                    match field {
                        FieldValue::Child(id) => push(&mut nodes, *id, budget)?,
                        FieldValue::Children(ids) => {
                            for id in ids {
                                push(&mut nodes, *id, budget)?;
                            }
                        }
                        FieldValue::Foreign(f) => push(
                            &mut pending,
                            (
                                &f.bundle,
                                depth.checked_add(1).ok_or(StopReason::DepthLimit)?,
                            ),
                            budget,
                        )?,
                        _ => {}
                    }
                }
            }
            if reached.iter().any(|v| !*v) {
                return Err(TreeError::Unreachable);
            }
            if context.nodes.len() != bundle.nodes.len() {
                return Err(TreeError::Selection);
            }
            for (index, selected) in context.nodes.iter().enumerate() {
                budget.charge(Resource::Work, index as u64 + 1)?;
                if context.nodes[..index]
                    .iter()
                    .any(|prior| prior.node == selected.node)
                {
                    return Err(TreeError::Duplicate);
                }
                let node = node(bundle, selected.node)?;
                let checked = profile.validate_entry(&selected.entry, budget)?;
                if profile.execution_digest(&selected.entry.alias, budget)?
                    != selected.execution_digest
                {
                    return Err(TreeError::ExecutionIdentity);
                }
                let package = checked.package();
                let valid = match &selected.shape {
                    ShapeSelection::Form { index } => usize::try_from(*index)
                        .ok()
                        .and_then(|i| package.forms.get(i))
                        .is_some_and(|f| {
                            f.category == selected.entry.category
                                && same_kind(node, &f.kind, registry)
                                && f.fields.len() == node.fields.len()
                        }),
                    ShapeSelection::Leaf { index } => usize::try_from(*index)
                        .ok()
                        .and_then(|i| package.leaves.get(i))
                        .is_some_and(|l| {
                            l.category == selected.entry.category
                                && same_kind(node, &l.kind, registry)
                                && node.fields.is_empty()
                        }),
                    ShapeSelection::Builtin { read } => {
                        matches!(package.read(*read)?,ReadSpec::Builtin{kind,..} if same_kind(node,kind,registry)&&node.fields.is_empty())
                    }
                    ShapeSelection::List { read, cons } => {
                        matches!(package.read(*read)?,ReadSpec::ListOf{cons:c,nil,..} if same_kind(node,if *cons{c}else{nil},registry)&&node.fields.len()==if *cons{2}else{0})
                    }
                    ShapeSelection::Dynamic {
                        provider, shape, ..
                    } => {
                        let registered = profile.head_provider(
                            &selected.entry.alias,
                            &selected.entry.category,
                            budget,
                        )?;
                        budget.charge(
                            Resource::Work,
                            (provider.shape.schema.package.len()
                                + provider.shape.name.len()
                                + provider.child_context.schema.package.len()
                                + provider.child_context.name.len())
                                as u64
                                + 66,
                        )?;
                        if registered != Some(provider) {
                            return Err(TreeError::UnvalidatedDynamic);
                        }
                        profile.provider(&provider.shape, budget)?;
                        profile.provider(&provider.child_context, budget)?;
                        checked.validate_head_shape(shape, budget)?;
                        same_kind(node, &shape.kind, registry)
                            && node.fields.len() == shape.fields.len()
                    }
                    ShapeSelection::Recovery => {
                        let entries = &recovery.ok_or(TreeError::Recovery)?.entries;
                        budget.charge(Resource::Work, entries.len() as u64 + 1)?;
                        let entry = entries
                            .iter()
                            .find(|r| r.node == selected.node)
                            .ok_or(TreeError::Recovery)?;
                        if node.schema.package != "nepl3.engine"
                            || node.schema.revision != 1
                            || !node.fields.is_empty()
                        {
                            return Err(TreeError::Recovery);
                        }
                        match &entry.kind {
                            RecoveryKind::Missing { expected, anchor } => {
                                profile.validate_entry(expected, budget)?;
                                node.kind == "RecoveryMissing"
                                    && expected == &selected.entry
                                    && node.token.is_none()
                                    && node.head.is_none()
                                    && node.cover.as_ref() == Some(anchor)
                                    && anchor.start() == anchor.end()
                            }
                            RecoveryKind::Unparsed { span, .. } => {
                                node.kind == "RecoveryUnparsed"
                                    && node.cover.as_ref() == Some(span)
                                    && match node.token {
                                        Some(_) => {
                                            let token = token(bundle, node)?;
                                            node.head.as_ref() == Some(&token.head)
                                                && span.contains(&token.head)
                                        }
                                        None => node.head.is_none(),
                                    }
                            }
                            RecoveryKind::Unexpected { token } => {
                                let reference = *token;
                                let token = usize::try_from(token.0)
                                    .ok()
                                    .and_then(|i| bundle.tokens.get(i))
                                    .ok_or(TreeError::Recovery)?;
                                node.kind == "RecoveryUnexpected"
                                    && node.token == Some(reference)
                                    && node.head.as_ref() == Some(&token.head)
                                    && node.cover.as_ref() == Some(&token.head)
                            }
                        }
                    }
                };
                if !valid {
                    return Err(TreeError::Selection);
                }
                static_fields(bundle, context, &contexts, selected, profile, budget)?;
                if !matches!(selected.shape, ShapeSelection::Recovery) && node.token.is_none() {
                    return Err(TreeError::Selection);
                }
            }
            if let Some(recovery) = recovery {
                for (i, entry) in recovery.entries.iter().enumerate() {
                    budget.charge(Resource::Work, (context.nodes.len() + i) as u64 + 1)?;
                    if recovery.entries[..i]
                        .iter()
                        .any(|prior| prior.node == entry.node)
                    {
                        return Err(TreeError::Duplicate);
                    }
                    if !context.nodes.iter().any(|v| {
                        v.node == entry.node && matches!(v.shape, ShapeSelection::Recovery)
                    }) {
                        return Err(TreeError::Recovery);
                    }
                }
            }
        }
        Ok(ValidatedParseTree { tree: self, syntax })
    }
}
