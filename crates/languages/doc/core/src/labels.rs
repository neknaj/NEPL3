//! Article-local labels. This proof does not check foreign meanings, resolve
//! pages/assets, or grant the ability to render an unprepared document.
use crate::{
    check::{StructureError, edges},
    model::*,
};
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, Span},
};
mod diagnostic;
mod occurrences;
pub use diagnostic::LabelDiagnosticError;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocLabelId(pub u64);
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct LabelSite<'a> {
    pub node: u64,
    pub name: &'a str,
    pub selection: Option<&'a Span>,
    pub range: Option<&'a Span>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResolvedLabel<'a> {
    pub reference: LabelSite<'a>,
    pub target: DocLabelId,
}
#[derive(Debug, Eq, PartialEq)]
pub enum LabelError<'a> {
    Stopped(StopReason),
    Structure(StructureError),
    ExpectedArticle,
    DuplicateOccurrence {
        definition: LabelSite<'a>,
        paths: LabelOccurrencePaths,
    },
    Duplicate {
        definition: LabelSite<'a>,
        previous: LabelSite<'a>,
    },
    Unresolved {
        reference: LabelSite<'a>,
    },
}
impl From<StopReason> for LabelError<'_> {
    fn from(s: StopReason) -> Self {
        Self::Stopped(s)
    }
}
impl From<StructureError> for LabelError<'_> {
    fn from(e: StructureError) -> Self {
        match e {
            StructureError::Stopped(s) => Self::Stopped(s),
            e => Self::Structure(e),
        }
    }
}
pub struct CheckedLabels<'a> {
    document: &'a DocumentSyntax,
    definitions: Vec<LabelSite<'a>>,
    references: Vec<ResolvedLabel<'a>>,
}
impl<'a> CheckedLabels<'a> {
    pub fn document(&self) -> &'a DocumentSyntax {
        self.document
    }
    pub fn definitions(&self) -> &[LabelSite<'a>] {
        &self.definitions
    }
    pub fn references(&self) -> &[ResolvedLabel<'a>] {
        &self.references
    }
}
fn site<'a>(node: &'a DocNode, index: usize, name: &'a str) -> LabelSite<'a> {
    LabelSite {
        node: index as u64,
        name,
        selection: node.locations.first().and_then(|f| f.span.as_ref()),
        range: node.span.as_ref(),
    }
}
fn push<T>(items: &mut Vec<T>, item: T, b: &mut Budget) -> Result<(), StopReason> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    items.push(item);
    Ok(())
}
/// Collect declarations before resolving references, so a forward reference
/// reaches the same definition identity as a later one. Only this Article's
/// semantic arena is visited; embedded ForeignClosure bundles are not entered.
pub fn check<'a>(
    document: &'a DocumentSyntax,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<CheckedLabels<'a>, LabelError<'a>> {
    let structure = document.validate_structure(registry, b, admission)?;
    let DocRoot::Article(root) = document.value.root else {
        return Err(LabelError::ExpectedArticle);
    };
    occurrences::check(document, structure.shape().postorder(), root, b)?;
    let mut definitions: Vec<LabelSite<'a>> = Vec::new();
    let mut unresolved = Vec::new();
    b.charge(Resource::AllocationUnits, document.value.nodes.len() as u64)?;
    let mut visited = vec![false; document.value.nodes.len()];
    let mut pending = Vec::new();
    push(&mut pending, root.0 as usize, b)?;
    while let Some(index) = pending.pop() {
        b.charge(Resource::Work, 1)?;
        if visited[index] {
            continue;
        }
        visited[index] = true;
        let node = &document.value.nodes[index];
        match &node.kind {
            DocKind::Section { id, .. } | DocKind::Anchor { id, .. } => {
                let current = site(node, index, id);
                for previous in &definitions {
                    b.charge(Resource::Work, (previous.name.len() + id.len()) as u64 + 1)?;
                    if previous.name == id {
                        return Err(LabelError::Duplicate {
                            definition: current,
                            previous: *previous,
                        });
                    }
                }
                push(&mut definitions, current, b)?;
            }
            DocKind::Reference { target, .. } => {
                push(&mut unresolved, site(node, index, target), b)?
            }
            _ => {}
        }
        // Preserve constructor/child order, not arena storage order. Charge
        // each queued child before allocation even for a very wide paragraph.
        let mut children = Vec::new();
        let mut next = 0;
        while let Some((child, _)) = edges::edge(&node.kind, next) {
            b.charge(Resource::Work, 1)?;
            push(&mut children, child as usize, b)?;
            next += 1;
        }
        for child in children.into_iter().rev() {
            push(&mut pending, child, b)?;
        }
    }
    let mut references = Vec::new();
    for reference in unresolved {
        let mut target = None;
        for (index, definition) in definitions.iter().enumerate() {
            b.charge(
                Resource::Work,
                (reference.name.len() + definition.name.len()) as u64 + 1,
            )?;
            if reference.name == definition.name {
                target = Some(DocLabelId(index as u64));
                break;
            }
        }
        let target = target.ok_or(LabelError::Unresolved { reference })?;
        push(&mut references, ResolvedLabel { reference, target }, b)?;
    }
    Ok(CheckedLabels {
        document,
        definitions,
        references,
    })
}
