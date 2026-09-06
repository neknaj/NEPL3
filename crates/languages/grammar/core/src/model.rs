//! Grammar's complete typed constructor arena. This is independent of Rust enum ABI.
//! Source provenance is retained by literals, lists and constructor covers.
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    source::{SourceAdmission, SourceError, SourceSnapshot, SourceStore, Span},
    value::Integer,
};

mod generated;
pub use generated::{Category, NodeKind};
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NodeId(pub u64);
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Located<T> {
    pub value: T,
    pub span: Span,
}
pub type NameLiteral = Located<String>;
pub type TextLiteral = Located<String>;
pub type LangLiteral = Located<String>;
pub type NatLiteral = Located<Integer>;
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NodeList {
    pub items: Vec<NodeId>,
    pub span: Span,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Node {
    pub kind: NodeKind,
    pub span: Span,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub sources: Vec<SourceSnapshot>,
    pub nodes: Vec<Node>,
    pub root: NodeId,
}
#[derive(Debug, Eq, PartialEq)]
pub enum ModelError {
    Stopped(StopReason),
    Source(SourceError),
    MissingNode,
    Category,
    Cycle,
    Span,
    Natural,
    DuplicateSource,
    Unreachable,
}
impl From<StopReason> for ModelError {
    fn from(v: StopReason) -> Self {
        Self::Stopped(v)
    }
}
impl From<SourceError> for ModelError {
    fn from(v: SourceError) -> Self {
        match v {
            SourceError::Stopped(r) => Self::Stopped(r),
            v => Self::Source(v),
        }
    }
}
/// Proves typed constructor references and source geometry, not that an arbitrary
/// provider produced these values or that source bytes were parsed by a given reader.
pub struct CheckedDocument<'a> {
    document: &'a Document,
}
impl CheckedDocument<'_> {
    pub fn document(&self) -> &Document {
        self.document
    }
}
enum LiteralRef<'a> {
    Name(&'a NameLiteral),
    Text(&'a TextLiteral),
    Nat(&'a NatLiteral),
}
trait Visitor {
    fn node(&mut self, id: NodeId, category: Category) -> Result<(), ModelError>;
    fn list(&mut self, list: &NodeList, category: Category) -> Result<(), ModelError>;
    fn literal(&mut self, value: LiteralRef<'_>) -> Result<(), ModelError>;
}
struct Check<'a, 'b> {
    document: &'a Document,
    source: &'a SourceStore,
    parent: &'a Span,
    pending: &'b mut Vec<(NodeId, bool, u64)>,
    depth: u64,
    budget: &'b mut Budget,
}
impl Check<'_, '_> {
    fn span(&mut self, span: &Span) -> Result<(), ModelError> {
        self.budget.charge(
            Resource::Work,
            span.snapshot_ref().source.0.len() as u64 + 1,
        )?;
        if !self.parent.contains(span) {
            return Err(ModelError::Span);
        }
        self.source
            .get_ref(span.snapshot_ref())
            .ok_or(SourceError::MissingSnapshot)?
            .slice(span)?;
        Ok(())
    }
}
impl Visitor for Check<'_, '_> {
    fn node(&mut self, id: NodeId, category: Category) -> Result<(), ModelError> {
        self.budget.charge(Resource::Work, 1)?;
        let node = self.document.node(id)?;
        if node.kind.category() != category {
            return Err(ModelError::Category);
        }
        self.span(&node.span)?;
        self.budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(NodeId, bool, u64)>() as u64,
        )?;
        self.pending.push((
            id,
            false,
            self.depth.checked_add(1).ok_or(StopReason::DepthLimit)?,
        ));
        Ok(())
    }
    fn list(&mut self, list: &NodeList, category: Category) -> Result<(), ModelError> {
        self.span(&list.span)?;
        let mut end = list.span.start();
        for id in &list.items {
            let span = &self.document.node(*id)?.span;
            if !list.span.contains(span) || span.start() < end {
                return Err(ModelError::Span);
            }
            end = span.end();
            self.node(*id, category)?;
        }
        Ok(())
    }
    fn literal(&mut self, value: LiteralRef<'_>) -> Result<(), ModelError> {
        let (span, bytes) = match value {
            LiteralRef::Name(v) | LiteralRef::Text(v) => (&v.span, v.value.len() as u64),
            LiteralRef::Nat(v) => {
                if v.value.is_negative() {
                    return Err(ModelError::Natural);
                }
                (&v.span, v.value.as_bigint().bits().div_ceil(8))
            }
        };
        self.budget.charge(Resource::Work, bytes + 1)?;
        self.span(span)
    }
}
impl Document {
    pub fn node(&self, id: NodeId) -> Result<&Node, ModelError> {
        usize::try_from(id.0)
            .ok()
            .and_then(|i| self.nodes.get(i))
            .ok_or(ModelError::MissingNode)
    }
    pub fn validate<'a>(
        &'a self,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<CheckedDocument<'a>, ModelError> {
        budget.charge(Resource::Work, 1)?;
        if self.node(self.root)?.kind.category() != Category::Root {
            return Err(ModelError::Category);
        }
        let mut sources = SourceStore::default();
        for source in &self.sources {
            admission.admit_existing(source, budget)?;
            if sources.get_ref(source.identity()).is_some() {
                return Err(ModelError::DuplicateSource);
            }
            sources.insert(source.clone_with_budget(budget)?)?;
        }
        budget.charge(Resource::AllocationUnits, self.nodes.len() as u64)?;
        let mut color = alloc::vec![0u8;self.nodes.len()];
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(NodeId, bool, u64)>() as u64,
        )?;
        let mut pending = alloc::vec![(self.root, false, 1u64)];
        while let Some((id, exit, depth)) = pending.pop() {
            budget.charge(Resource::Work, 1)?;
            let i = usize::try_from(id.0).map_err(|_| ModelError::MissingNode)?;
            let state = color.get_mut(i).ok_or(ModelError::MissingNode)?;
            if exit {
                *state = 2;
                continue;
            }
            budget.observe_depth(depth)?;
            if *state == 1 {
                return Err(ModelError::Cycle);
            }
            if *state == 0 {
                budget.charge(Resource::Nodes, 1)?;
            }
            // Revisit a shared DAG subtree at its current depth. Skipping an earlier
            // visit would undercount a later, longer path; repeated work remains bounded.
            *state = 1;
            let node = self.node(id)?;
            sources
                .get_ref(node.span.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?
                .slice(&node.span)?;
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<(NodeId, bool, u64)>() as u64,
            )?;
            pending.push((id, true, depth));
            node.kind.visit(&mut Check {
                document: self,
                source: &sources,
                parent: &node.span,
                pending: &mut pending,
                depth,
                budget,
            })?;
        }
        if color.contains(&0) {
            return Err(ModelError::Unreachable);
        }
        Ok(CheckedDocument { document: self })
    }
}
