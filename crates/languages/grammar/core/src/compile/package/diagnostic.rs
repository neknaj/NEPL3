//! Resolve checked package indices through the compiler's retained AST maps.
use super::*;
use crate::model::Document;
use nepl3_core::source::Span;

pub(super) struct Sources<'a> {
    pub doc: &'a Document,
    pub reads: &'a [Option<p::ReadSpecId>],
    pub bindings: &'a [Option<p::BindingId>],
}
pub(super) fn locate(
    package: &p::LanguagePackage,
    failure: p::PackageFailure,
    sources: Sources<'_>,
    budget: &mut Budget,
) -> CompileError {
    let error = CompileError::from(failure.error);
    if matches!(error, CompileError::Stopped(_)) {
        return error;
    }
    let Some(subject) = failure.subject else {
        return error;
    };
    let resolved = resolve(subject, &sources, budget);
    let (at, owner) = match resolved {
        Ok(value) => value,
        Err(error) => return error,
    };
    let Some(at) = at else {
        return error;
    };
    let node = match sources.doc.node(at) {
        Ok(node) => node,
        Err(error) => return error.into(),
    };
    let primary = operand(&node.kind, error.cause()).unwrap_or(&node.span);
    let related = match owner {
        Some(p::BindingOwner::Leaf(index)) => {
            let leaf = match declaration(sources.doc, p::PackageSubject::Leaf(index), budget) {
                Ok(v) => v,
                Err(error) => return error,
            };
            match leaf.and_then(|id| sources.doc.nodes.get(id.0 as usize)) {
                Some(crate::model::Node {
                    kind: NodeKind::Leaf { token, .. },
                    ..
                }) => match reader_source(sources.doc, &token.value, budget) {
                    Ok(span) => span,
                    Err(error) => return error,
                },
                _ => None,
            }
        }
        Some(p::BindingOwner::Form(index)) => {
            match declaration(sources.doc, p::PackageSubject::Form(index), budget) {
                Ok(Some(id)) => sources.doc.nodes.get(id.0 as usize).map(|node| &node.span),
                Ok(None) => None,
                Err(error) => return error,
            }
        }
        None => None,
    };
    let mismatch = matches!(
        error.cause(),
        CompileError::Package(p::PackageError::InvalidBinding)
    ) && matches!(&node.kind, NodeKind::Bind { field, .. } | NodeKind::Reference { field, .. } | NodeKind::Export { field, .. } if field.value == "self");
    let mut located = error.at(primary, related, budget);
    if mismatch
        && let Some(p::BindingOwner::Leaf(index)) = owner
        && let Some(leaf) = package.leaves.get(index as usize)
    {
        located = located.with_types(&TypeDescriptor::Text, &leaf.payload, budget);
    }
    located
}
fn operand<'a>(kind: &'a NodeKind, error: &CompileError) -> Option<&'a Span> {
    let missing_namespace = matches!(
        error,
        CompileError::Package(p::PackageError::MissingNamespace)
    );
    Some(match kind {
        NodeKind::Language { root, .. } => &root.span,
        NodeKind::Category { mode, .. } => &mode.span,
        NodeKind::Local { category }
        | NodeKind::Form { category, .. }
        | NodeKind::Leaf { category, .. }
            if matches!(
                error,
                CompileError::Package(p::PackageError::MissingCategory)
            ) =>
        {
            &category.span
        }
        NodeKind::WithMode { mode, .. } => &mode.span,
        NodeKind::Take { reader, .. } | NodeKind::Skip { reader } => &reader.span,
        NodeKind::Extension { signature, .. } => &signature.span,
        NodeKind::Bind { namespace, field }
        | NodeKind::Reference { namespace, field }
        | NodeKind::Export { namespace, field } => {
            if missing_namespace {
                &namespace.span
            } else {
                &field.span
            }
        }
        NodeKind::Visit { child } | NodeKind::Import { child } | NodeKind::Propagate { child } => {
            &child.span
        }
        NodeKind::Custom { provider } => &provider.span,
        _ => return None,
    })
}
fn resolve(
    subject: p::PackageSubject,
    sources: &Sources<'_>,
    budget: &mut Budget,
) -> Result<(Option<NodeId>, Option<p::BindingOwner>), CompileError> {
    use p::PackageSubject as S;
    let doc = sources.doc;
    Ok(match subject {
        S::Root => (Some(doc.root), None),
        S::Read(read) => (inverse(sources.reads, read, budget)?, None),
        S::Binding { owner, binding } => {
            let at = if let Some(binding) = binding {
                inverse(sources.bindings, binding, budget)?
            } else {
                match owner {
                    Some(p::BindingOwner::Form(i)) => declaration(doc, S::Form(i), budget)?,
                    Some(p::BindingOwner::Leaf(i)) => declaration(doc, S::Leaf(i), budget)?,
                    None => None,
                }
            };
            (at, owner)
        }
        S::FormField { form, field } => {
            let at = if let Some(id) = declaration(doc, S::Form(form), budget)? {
                if let NodeKind::Form { fields, .. } = &doc.node(id)?.kind {
                    fields.items.get(field as usize).copied()
                } else {
                    None
                }
            } else {
                None
            };
            (at, None)
        }
        S::ModeSkip { rule, .. } | S::ModeTake { rule, .. } => {
            let at = if let Some(id) = declaration(doc, subject, budget)? {
                if let NodeKind::Mode { rules, .. } = &doc.node(id)?.kind {
                    let mut index = 0;
                    let mut result = None;
                    for id in &rules.items {
                        budget.charge(Resource::Work, 1)?;
                        if matches!(
                            (&doc.node(*id)?.kind, subject),
                            (NodeKind::Skip { .. }, S::ModeSkip { .. })
                                | (NodeKind::Take { .. }, S::ModeTake { .. })
                        ) {
                            if index == rule {
                                result = Some(*id);
                                break;
                            }
                            index += 1;
                        }
                    }
                    result
                } else {
                    None
                }
            } else {
                None
            };
            (at, None)
        }
        S::Provenance(_) => (None, None),
        _ => (declaration(doc, subject, budget)?, None),
    })
}
fn inverse<T: Copy + Eq>(
    map: &[Option<T>],
    wanted: T,
    budget: &mut Budget,
) -> Result<Option<NodeId>, CompileError> {
    for (index, value) in map.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if *value == Some(wanted) {
            return Ok(Some(NodeId(index as u64)));
        }
    }
    Ok(None)
}
fn declaration(
    doc: &Document,
    subject: p::PackageSubject,
    budget: &mut Budget,
) -> Result<Option<NodeId>, CompileError> {
    use p::PackageSubject as S;
    let wanted = match subject {
        S::Category(i) | S::Form(i) | S::Leaf(i) | S::Extension(i) => i,
        S::ModeSkip { mode, .. } | S::ModeTake { mode, .. } => mode,
        _ => return Ok(None),
    };
    let NodeKind::Language { declarations, .. } = &doc.node(doc.root)?.kind else {
        return Err(CompileError::WrongConstructor(doc.root));
    };
    let mut index = 0;
    for id in &declarations.items {
        budget.charge(Resource::Work, 1)?;
        if matches!(
            (&doc.node(*id)?.kind, subject),
            (NodeKind::Category { .. }, S::Category(_))
                | (NodeKind::Form { .. }, S::Form(_))
                | (NodeKind::Leaf { .. }, S::Leaf(_))
                | (NodeKind::Extension { .. }, S::Extension(_))
                | (
                    NodeKind::Mode { .. },
                    S::ModeSkip { .. } | S::ModeTake { .. }
                )
        ) {
            if index == wanted {
                return Ok(Some(*id));
            }
            index += 1;
        }
    }
    Ok(None)
}
fn reader_source<'a>(
    doc: &'a Document,
    token: &str,
    budget: &mut Budget,
) -> Result<Option<&'a Span>, CompileError> {
    for node in &doc.nodes {
        budget.charge(Resource::Work, token.len() as u64 + 1)?;
        if let NodeKind::Take { kind, reader } = &node.kind
            && kind.value == token
        {
            for declaration in &doc.nodes {
                budget.charge(Resource::Work, reader.value.len() as u64 + 1)?;
                if let NodeKind::Reader { name, expression } = &declaration.kind
                    && name.value == reader.value
                {
                    return Ok(Some(&doc.node(*expression)?.span));
                }
            }
        }
    }
    Ok(None)
}
