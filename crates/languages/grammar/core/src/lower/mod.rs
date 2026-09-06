//! Lower a validated static Grammar surface tree into its complete typed constructor arena.
//! Positions come only from retained tokens and covers; no source search or fabricated span.
mod generated;
use crate::model::*;
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{SourceAdmission, Span},
    syntax::{FieldValue, NodeRef, SyntaxBundle, SyntaxNode},
    value::NdfValue,
};
use nepl3_engine::{
    package::{Form, ReadSpec},
    profile::ResolvedParseProfile,
    selection::{NodeSelection, ShapeSelection},
    tree::ValidatedParseTree,
};
use nepl3_reader::builtin::BuiltinReader;
#[derive(Debug, Eq, PartialEq)]
pub enum LowerError {
    Stopped(StopReason),
    Model(ModelError),
    Profile(nepl3_engine::profile::ProfileError),
    Package(nepl3_engine::package::PackageError),
    Shape,
    Reference,
    Recovery,
    Foreign,
    Tree(nepl3_engine::tree::TreeError),
}
impl From<StopReason> for LowerError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<ModelError> for LowerError {
    fn from(v: ModelError) -> Self {
        Self::Model(v)
    }
}
impl From<nepl3_engine::profile::ProfileError> for LowerError {
    fn from(v: nepl3_engine::profile::ProfileError) -> Self {
        Self::Profile(v)
    }
}
impl From<nepl3_engine::package::PackageError> for LowerError {
    fn from(v: nepl3_engine::package::PackageError) -> Self {
        Self::Package(v)
    }
}
struct Adapter<'a> {
    bundle: &'a SyntaxBundle,
    profile: &'a ResolvedParseProfile<'a>,
    selections: Vec<Option<&'a NodeSelection>>,
    mapping: Vec<Option<NodeId>>,
    nodes: Vec<Node>,
    budget: &'a mut Budget,
}
fn span(v: &Span, b: &mut Budget) -> Result<Span, StopReason> {
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Span>() as u64 + v.snapshot_ref().source.0.len() as u64,
    )?;
    b.charge(Resource::Work, v.snapshot_ref().source.0.len() as u64)?;
    Ok(v.clone())
}
impl<'a> Adapter<'a> {
    fn index(id: NodeRef) -> Result<usize, LowerError> {
        usize::try_from(id.0).map_err(|_| LowerError::Reference)
    }
    fn selected(&self, id: NodeRef) -> Result<&'a NodeSelection, LowerError> {
        self.selections
            .get(Self::index(id)?)
            .and_then(|v| *v)
            .ok_or(LowerError::Reference)
    }
    fn syntax(&self, id: NodeRef) -> Result<&'a SyntaxNode, LowerError> {
        self.bundle
            .nodes
            .get(Self::index(id)?)
            .ok_or(LowerError::Reference)
    }
    fn field(&mut self, node: &SyntaxNode, form: &Form, name: &str) -> Result<NodeRef, LowerError> {
        self.budget.charge(
            Resource::Work,
            form.fields.len() as u64 * (name.len() as u64 + 1),
        )?;
        let index = form
            .fields
            .iter()
            .position(|v| v.name == name)
            .ok_or(LowerError::Shape)?;
        match node.fields.get(index) {
            Some(FieldValue::Child(id)) => Ok(*id),
            Some(FieldValue::Foreign(_)) => Err(LowerError::Foreign),
            _ => Err(LowerError::Shape),
        }
    }
    fn node(
        &mut self,
        node: &SyntaxNode,
        form: &Form,
        name: &str,
        category: Category,
    ) -> Result<NodeId, LowerError> {
        let id = self.field(node, form, name)?;
        let mapped = self
            .mapping
            .get(Self::index(id)?)
            .and_then(|v| *v)
            .ok_or(LowerError::Reference)?;
        if self
            .nodes
            .get(mapped.0 as usize)
            .ok_or(LowerError::Reference)?
            .kind
            .category()
            != category
        {
            return Err(LowerError::Shape);
        }
        Ok(mapped)
    }
    fn list(
        &mut self,
        node: &SyntaxNode,
        form: &Form,
        name: &str,
        category: Category,
    ) -> Result<NodeList, LowerError> {
        let mut current = self.field(node, form, name)?;
        let root = self.syntax(current)?;
        let cover = span(root.cover.as_ref().ok_or(LowerError::Shape)?, self.budget)?;
        let mut items = Vec::new();
        loop {
            self.budget.charge(Resource::Work, 1)?;
            let selected = self.selected(current)?;
            let ShapeSelection::List { cons, .. } = selected.shape else {
                return Err(LowerError::Shape);
            };
            let node = self.syntax(current)?;
            if !cons {
                if !node.fields.is_empty() {
                    return Err(LowerError::Shape);
                }
                break;
            }
            let [FieldValue::Child(head), FieldValue::Child(tail)] = node.fields.as_slice() else {
                return Err(LowerError::Shape);
            };
            let mapped = self
                .mapping
                .get(Self::index(*head)?)
                .and_then(|v| *v)
                .ok_or(LowerError::Reference)?;
            if self
                .nodes
                .get(mapped.0 as usize)
                .ok_or(LowerError::Reference)?
                .kind
                .category()
                != category
            {
                return Err(LowerError::Shape);
            }
            self.budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<NodeId>() as u64,
            )?;
            items.push(mapped);
            current = *tail;
        }
        Ok(NodeList { items, span: cover })
    }
    fn literal(
        &mut self,
        node: &SyntaxNode,
        form: &Form,
        name: &str,
        expected: BuiltinReader,
    ) -> Result<(&'a NdfValue, &'a Span), LowerError> {
        let id = self.field(node, form, name)?;
        let selected = self.selected(id)?;
        let ShapeSelection::Builtin { read } = selected.shape else {
            return Err(LowerError::Shape);
        };
        let package = self.profile.language(&selected.entry.alias, self.budget)?;
        if !matches!(package.read(read)?,ReadSpec::Builtin{reader,..}if *reader==expected) {
            return Err(LowerError::Shape);
        }
        let child = self.syntax(id)?;
        let token = child
            .token
            .and_then(|id| usize::try_from(id.0).ok())
            .and_then(|id| self.bundle.tokens.get(id))
            .ok_or(LowerError::Reference)?;
        Ok((&token.payload, &token.head))
    }
    fn string(
        &mut self,
        node: &SyntaxNode,
        form: &Form,
        name: &str,
        kind: BuiltinReader,
    ) -> Result<Located<alloc::string::String>, LowerError> {
        let (value, at) = self.literal(node, form, name, kind)?;
        let NdfValue::Text(value) = value else {
            return Err(LowerError::Shape);
        };
        self.budget
            .charge(Resource::AllocationUnits, value.len() as u64)?;
        self.budget.charge(Resource::Work, value.len() as u64)?;
        Ok(Located {
            value: value.clone(),
            span: span(at, self.budget)?,
        })
    }
    fn name(
        &mut self,
        node: &SyntaxNode,
        form: &Form,
        name: &str,
    ) -> Result<NameLiteral, LowerError> {
        self.string(node, form, name, BuiltinReader::Name)
    }
    fn text(
        &mut self,
        node: &SyntaxNode,
        form: &Form,
        name: &str,
    ) -> Result<TextLiteral, LowerError> {
        self.string(node, form, name, BuiltinReader::Text)
    }
    fn nat(
        &mut self,
        node: &SyntaxNode,
        form: &Form,
        name: &str,
    ) -> Result<NatLiteral, LowerError> {
        let (value, at) = self.literal(node, form, name, BuiltinReader::Nat)?;
        let NdfValue::Integer(value) = value else {
            return Err(LowerError::Shape);
        };
        if value.is_negative() {
            return Err(LowerError::Shape);
        }
        self.budget.charge(
            Resource::AllocationUnits,
            value.as_bigint().bits().div_ceil(8),
        )?;
        self.budget
            .charge(Resource::Work, value.as_bigint().bits().div_ceil(8))?;
        Ok(Located {
            value: value.clone(),
            span: span(at, self.budget)?,
        })
    }
}
/// Convert only static constructor selections resolved through their original profile.
/// The returned document is checked for category, source, list and graph invariants.
pub fn lower(
    tree: &ValidatedParseTree<'_>,
    profile: &ResolvedParseProfile<'_>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Document, LowerError> {
    let tree = tree.tree();
    tree.validate(profile, budget, admission)
        .map_err(LowerError::Tree)?;
    if tree.profile_digest != profile.digest() {
        return Err(LowerError::Shape);
    }
    if tree.recovery.iter().any(|v| !v.entries.is_empty()) {
        return Err(LowerError::Recovery);
    }
    if tree.contexts.len() != 1 || !tree.contexts[0].path.is_empty() {
        return Err(LowerError::Foreign);
    }
    let count = tree.bundle.nodes.len();
    budget.charge(
        Resource::AllocationUnits,
        count as u64
            * (core::mem::size_of::<Option<NodeId>>()
                + core::mem::size_of::<Option<&NodeSelection>>()) as u64,
    )?;
    let mut selections = alloc::vec![None;count];
    for selected in &tree.contexts[0].nodes {
        let index = Adapter::index(selected.node)?;
        let slot = selections.get_mut(index).ok_or(LowerError::Reference)?;
        if slot.replace(selected).is_some() {
            return Err(LowerError::Reference);
        }
    }
    let mut adapter = Adapter {
        bundle: &tree.bundle,
        profile,
        selections,
        mapping: alloc::vec![None;count],
        nodes: Vec::new(),
        budget,
    };
    let mut pending = Vec::new();
    push(&mut pending, (tree.bundle.root, false, 1), adapter.budget)?;
    while let Some((id, exit, depth)) = pending.pop() {
        adapter.budget.observe_depth(depth)?;
        adapter.budget.charge(Resource::Work, 1)?;
        let node = adapter.syntax(id)?;
        if !exit {
            push(&mut pending, (id, true, depth), adapter.budget)?;
            for field in node.fields.iter().rev() {
                let FieldValue::Child(child) = field else {
                    return Err(LowerError::Foreign);
                };
                push(
                    &mut pending,
                    (
                        *child,
                        false,
                        depth
                            .checked_add(1)
                            .ok_or_else(|| adapter.budget.stop(StopReason::DepthLimit))?,
                    ),
                    adapter.budget,
                )?;
            }
            continue;
        }
        let selected = adapter.selected(id)?;
        if let ShapeSelection::Form { index } = selected.shape {
            let package = profile.language(&selected.entry.alias, adapter.budget)?;
            let form = usize::try_from(index)
                .ok()
                .and_then(|i| package.forms.get(i))
                .ok_or(LowerError::Reference)?;
            let kind = adapter.constructor(node, form)?;
            let at = span(
                node.cover.as_ref().ok_or(LowerError::Shape)?,
                adapter.budget,
            )?;
            adapter.budget.charge(Resource::Nodes, 1)?;
            adapter.budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<Node>() as u64,
            )?;
            let index = adapter.nodes.len() as u64;
            adapter.nodes.push(Node { kind, span: at });
            adapter.mapping[Adapter::index(id)?] = Some(NodeId(index));
        }
    }
    let root = adapter
        .mapping
        .get(Adapter::index(tree.bundle.root)?)
        .and_then(|v| *v)
        .ok_or(LowerError::Reference)?;
    let nodes = adapter.nodes;
    let mut sources = Vec::new();
    for source in &tree.bundle.sources {
        sources.push(source.clone_with_budget(budget)?);
    }
    let document = Document {
        sources,
        nodes,
        root,
    };
    document.validate(budget, admission)?;
    Ok(document)
}
fn push(
    stack: &mut Vec<(NodeRef, bool, u64)>,
    item: (NodeRef, bool, u64),
    b: &mut Budget,
) -> Result<(), StopReason> {
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<(NodeRef, bool, u64)>() as u64,
    )?;
    stack.push(item);
    Ok(())
}
