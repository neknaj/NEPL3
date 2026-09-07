//! Relate the engine's binding arena failure to the compiler's original AST map.
use super::*;
use crate::model::Document;
use nepl3_core::source::Span;

pub(super) fn locate(
    package: &p::LanguagePackage,
    error: p::PackageError,
    doc: &Document,
    bindings: &[Option<p::BindingId>],
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> CompileError {
    if !matches!(
        error,
        p::PackageError::InvalidBinding
            | p::PackageError::MissingNamespace
            | p::PackageError::InvalidSelector
            | p::PackageError::UnvisitedField
            | p::PackageError::MissingExtension
    ) {
        return error.into();
    }
    let result = (|| -> Result<Option<CompileError>, CompileError> {
        let NodeKind::Language { declarations, .. } = &doc.node(doc.root)?.kind else {
            return Err(CompileError::WrongConstructor(doc.root));
        };
        for leaf in [false, true] {
            let mut index = 0;
            for node_id in &declarations.items {
                let node = doc.node(*node_id)?;
                budget.charge(Resource::Work, 1)?;
                if !matches!(
                    (&node.kind, leaf),
                    (NodeKind::Form { .. }, false) | (NodeKind::Leaf { .. }, true)
                ) {
                    continue;
                }
                let owner = if leaf {
                    p::BindingOwner::Leaf(index)
                } else {
                    p::BindingOwner::Form(index)
                };
                index += 1;
                let Err(failure) = package.check_binding_owner(owner, registry, budget) else {
                    continue;
                };
                let mut at = *node_id;
                if let Some(binding) = failure.binding {
                    for (i, candidate) in bindings.iter().enumerate() {
                        budget.charge(Resource::Work, 1)?;
                        if *candidate == Some(binding) {
                            at = NodeId(i as u64);
                            break;
                        }
                    }
                }
                let failed = doc.node(at)?;
                let primary = match &failed.kind {
                    NodeKind::Bind { namespace, field }
                    | NodeKind::Reference { namespace, field }
                    | NodeKind::Export { namespace, field } => {
                        if failure.error == p::PackageError::MissingNamespace {
                            &namespace.span
                        } else {
                            &field.span
                        }
                    }
                    NodeKind::Visit { child }
                    | NodeKind::Import { child }
                    | NodeKind::Propagate { child } => &child.span,
                    NodeKind::Custom { provider } => &provider.span,
                    _ => &failed.span,
                };
                let related = if let NodeKind::Leaf { token, .. } = &node.kind {
                    reader_source(doc, &token.value, budget)?
                } else {
                    Some(&node.span)
                };
                let mismatch = failure.error == p::PackageError::InvalidBinding
                    && leaf
                    && matches!(&failed.kind, NodeKind::Bind { field, .. } | NodeKind::Reference { field, .. } | NodeKind::Export { field, .. } if field.value == "self");
                let mut error = CompileError::from(failure.error).at(primary, related, budget);
                if mismatch {
                    let actual = &package.leaves[(index - 1) as usize].payload;
                    error = error.with_types(&TypeDescriptor::Text, actual, budget);
                }
                return Ok(Some(error));
            }
        }
        Ok(None)
    })();
    match result {
        Ok(Some(located)) => located,
        Ok(None) => error.into(),
        Err(error) => error,
    }
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
