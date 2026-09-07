//! Only the completed roots supplied by the parser are traversed. Bundle source,
//! environment and origin tables never become part of the operation payload.
use super::{
    HeadError, ProjectedFieldValue, ProjectedNode, ProjectedNodeRef, ProjectedSpan,
    ProjectedSyntax, ProjectedToken, SourceWindow,
};
use alloc::{string::String, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource},
    source::{SourceAdmission, SourceSnapshot, Span},
    syntax::{FieldValue, NodeRef, SyntaxNode},
    value::{NdfScalar, SchemaRef},
    view::Token,
};

#[derive(Clone, Copy)]
struct Tables<'a> {
    nodes: &'a [SyntaxNode],
    tokens: &'a [Token],
    sources: &'a [SourceSnapshot],
}
#[derive(Clone, Copy)]
struct Task<'a> {
    tables: Tables<'a>,
    node: &'a SyntaxNode,
    depth: u64,
}

fn slot<T>(b: &mut Budget) -> Result<(), HeadError> {
    b.charge(Resource::AllocationUnits, core::mem::size_of::<T>() as u64)?;
    Ok(())
}
fn text(v: &str, b: &mut Budget) -> Result<String, HeadError> {
    b.charge(Resource::Work, v.len() as u64)?;
    b.charge(Resource::AllocationUnits, v.len() as u64)?;
    Ok(v.into())
}
fn schema(v: &SchemaRef, b: &mut Budget) -> Result<SchemaRef, HeadError> {
    Ok(SchemaRef {
        package: text(&v.package, b)?,
        revision: v.revision,
        digest: v.digest,
    })
}
pub(super) fn charge_scalar(v: &NdfScalar, b: &mut Budget) -> Result<(), HeadError> {
    let bytes = match v {
        NdfScalar::Text(v) => v.len() as u64,
        NdfScalar::Bytes(v) => v.len() as u64,
        NdfScalar::Integer(v) => v.as_bigint().bits().div_ceil(8),
        NdfScalar::Rational(v) => v
            .numerator()
            .as_bigint()
            .bits()
            .div_ceil(8)
            .saturating_add(v.denominator().bits().div_ceil(8)),
        _ => 0,
    };
    b.charge(Resource::Work, bytes.saturating_add(1))?;
    b.charge(
        Resource::AllocationUnits,
        bytes.saturating_add(core::mem::size_of::<NdfScalar>() as u64),
    )?;
    Ok(())
}
fn scalar(v: &NdfScalar, b: &mut Budget) -> Result<NdfScalar, HeadError> {
    charge_scalar(v, b)?;
    Ok(v.clone())
}
fn source<'a>(
    tables: Tables<'a>,
    span: &Span,
    b: &mut Budget,
) -> Result<&'a SourceSnapshot, HeadError> {
    for source in tables.sources {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() as u64)
                .saturating_add(span.snapshot_ref().source.0.len() as u64)
                .saturating_add(34),
        )?;
        if source.identity() == span.snapshot_ref() {
            return Ok(source);
        }
    }
    Err(nepl3_core::source::SourceError::MissingSnapshot.into())
}
fn capture_span(
    tables: Tables<'_>,
    span: &Span,
    windows: &mut Vec<SourceWindow>,
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<ProjectedSpan, HeadError> {
    let source = source(tables, span, b)?;
    for window in windows.iter() {
        b.charge(
            Resource::Work,
            (window.span.source.source_id.0.len() as u64)
                .saturating_add(span.snapshot_ref().source.0.len() as u64)
                .saturating_add(34),
        )?;
        if window.span.source.source_id == span.snapshot_ref().source
            && window.span.source.revision == span.snapshot_ref().revision
            && window.span.source.digest == span.snapshot_ref().digest
            && window.span.start == span.start()
            && window.span.end == span.end()
        {
            return ProjectedSpan::capture(span, b);
        }
    }
    let window = SourceWindow::capture(source, span.start(), span.end(), b, a)?;
    windows.push(window);
    ProjectedSpan::capture(span, b)
}
fn enqueue<'a>(
    tables: Tables<'a>,
    id: NodeRef,
    depth: u64,
    tasks: &mut Vec<Task<'a>>,
    b: &mut Budget,
) -> Result<ProjectedNodeRef, HeadError> {
    b.observe_depth(depth)?;
    let index = usize::try_from(id.0).map_err(|_| HeadError::Projection)?;
    let node = tables.nodes.get(index).ok_or(HeadError::Projection)?;
    for (index, task) in tasks.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        // Local host address is only a deduplication key; no address enters the
        // projection, operation identity or portable representation.
        if core::ptr::eq(task.node, node) {
            return Ok(ProjectedNodeRef(index as u64));
        }
    }
    slot::<Task<'a>>(b)?;
    let id = ProjectedNodeRef(tasks.len() as u64);
    tasks.push(Task {
        tables,
        node,
        depth,
    });
    Ok(id)
}

pub(crate) fn capture_completed(
    nodes: &[SyntaxNode],
    tokens: &[Token],
    sources: &[SourceSnapshot],
    completed: &[FieldValue],
    b: &mut Budget,
    a: &mut SourceAdmission,
) -> Result<ProjectedSyntax, HeadError> {
    let tables = Tables {
        nodes,
        tokens,
        sources,
    };
    let mut tasks = Vec::new();
    let mut out = ProjectedSyntax {
        nodes: Vec::new(),
        roots: Vec::new(),
        windows: Vec::new(),
    };
    for field in completed {
        let root = match field {
            FieldValue::Child(id) => enqueue(tables, *id, 1, &mut tasks, b)?,
            FieldValue::Foreign(v) => enqueue(
                Tables {
                    nodes: &v.bundle.nodes,
                    tokens: &v.bundle.tokens,
                    sources: &v.bundle.sources,
                },
                v.root,
                1,
                &mut tasks,
                b,
            )?,
            // HeadShape child slots are NodeRef or ForeignSyntax, not scalar or
            // variadic schema fields. Do not invent a root for a wrong slot.
            _ => return Err(HeadError::Projection),
        };
        slot::<ProjectedNodeRef>(b)?;
        out.roots.push(root);
    }
    let mut next = 0;
    while let Some(task) = tasks.get(next).copied() {
        b.charge(Resource::Work, 1)?;
        b.observe_depth(task.depth)?;
        slot::<ProjectedNode>(b)?;
        let mut fields = Vec::new();
        for field in &task.node.fields {
            slot::<ProjectedFieldValue>(b)?;
            let depth = task
                .depth
                .checked_add(1)
                .ok_or(nepl3_core::budget::StopReason::DepthLimit)?;
            fields.push(match field {
                FieldValue::Atom(v) => ProjectedFieldValue::Atom(scalar(v, b)?),
                FieldValue::Child(id) => {
                    ProjectedFieldValue::Child(enqueue(task.tables, *id, depth, &mut tasks, b)?)
                }
                FieldValue::Children(ids) => {
                    let mut children = Vec::new();
                    for id in ids {
                        slot::<ProjectedNodeRef>(b)?;
                        children.push(enqueue(task.tables, *id, depth, &mut tasks, b)?);
                    }
                    ProjectedFieldValue::Children(children)
                }
                FieldValue::Foreign(v) => ProjectedFieldValue::Foreign {
                    schema: schema(&v.schema, b)?,
                    category: text(&v.category, b)?,
                    root: enqueue(
                        Tables {
                            nodes: &v.bundle.nodes,
                            tokens: &v.bundle.tokens,
                            sources: &v.bundle.sources,
                        },
                        v.root,
                        depth,
                        &mut tasks,
                        b,
                    )?,
                    environment: v.environment.clone(),
                },
            });
        }
        let head = task
            .node
            .head
            .as_ref()
            .map(|span| capture_span(task.tables, span, &mut out.windows, b, a))
            .transpose()?;
        let cover = task
            .node
            .cover
            .as_ref()
            .map(|span| capture_span(task.tables, span, &mut out.windows, b, a))
            .transpose()?;
        let token = if let Some(id) = task.node.token {
            let token = task
                .tables
                .tokens
                .get(usize::try_from(id.0).map_err(|_| HeadError::Projection)?)
                .ok_or(HeadError::Projection)?;
            let source = source(task.tables, &token.head, b)?;
            // Capture its explicit head too, including token geometry in the
            // projection's independently checked window closure.
            capture_span(task.tables, &token.head, &mut out.windows, b, a)?;
            Some(ProjectedToken::capture(token, source, b)?)
        } else {
            None
        };
        out.nodes.push(ProjectedNode {
            schema: schema(&task.node.schema, b)?,
            kind: text(&task.node.kind, b)?,
            head,
            cover,
            token,
            fields,
        });
        next += 1;
    }
    Ok(out)
}
