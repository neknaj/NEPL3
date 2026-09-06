use super::*;
use nepl3_core::schema::CanonicalWriter;

fn named(value: &str, budget: &mut Budget) -> Result<TypeDescriptor, CompileError> {
    Ok(TypeDescriptor::Named(TypeRef {
        package: text("nepl3.foundation", budget)?,
        revision: 1,
        name: text(value, budget)?,
    }))
}
fn record(
    types: &mut Vec<NamedType>,
    name: String,
    fields: Vec<FieldDescriptor>,
    budget: &mut Budget,
) -> Result<(), CompileError> {
    if let Some(prior) = lookup(types, &name, |v| v.name.as_str(), budget)? {
        // Multiple take rules/list occurrences may intentionally share one kind.
        budget.charge(Resource::Work, fields.len() as u64 + 1)?;
        if prior.shape != (TypeShape::Record { fields }) {
            return Err(CompileError::OutputType);
        }
        return Ok(());
    }
    push(
        types,
        NamedType {
            name,
            shape: TypeShape::Record { fields },
            constraints: Vec::new(),
        },
        budget,
    )
}
fn payload(
    value: TypeDescriptor,
    budget: &mut Budget,
) -> Result<Vec<FieldDescriptor>, CompileError> {
    let mut fields = Vec::new();
    push(
        &mut fields,
        FieldDescriptor {
            name: text("payload", budget)?,
            ty: value,
        },
        budget,
    )?;
    Ok(fields)
}
pub(super) fn token_type(
    doc: &crate::model::Document,
    token: &str,
    outputs: &[(String, TypeDescriptor)],
    budget: &mut Budget,
) -> Result<Option<TypeDescriptor>, CompileError> {
    let mut result = None;
    for (i, node) in doc.nodes.iter().enumerate() {
        budget.charge(Resource::Work, token.len() as u64 + 1)?;
        if let NodeKind::Take { kind, reader } = &node.kind
            && kind.value == token
        {
            let value = &lookup(outputs, &reader.value, |v| v.0.as_str(), budget)?
                .ok_or(issue(NodeId(i as u64), DeclarationError::MissingReader))?
                .1;
            if result.as_ref().is_some_and(|prior| prior != value) {
                return Err(CompileError::OutputType);
            }
            if result.is_none() {
                result = Some(value.clone_with_budget(budget)?);
            }
        }
    }
    Ok(result)
}
fn foreign(
    doc: &crate::model::Document,
    mut id: NodeId,
    budget: &mut Budget,
) -> Result<bool, CompileError> {
    loop {
        budget.charge(Resource::Work, 1)?;
        match &doc.node(id)?.kind {
            NodeKind::WithMode { read, .. } => id = *read,
            NodeKind::Foreign { .. } => return Ok(true),
            _ => return Ok(false),
        }
    }
}
/// Stable symbolic ReadSpec identity: unary constructors are unfolded; names are
/// JSON strings, independent of AST order, DAG sharing and final schema digest.
pub(super) fn list_name(
    doc: &crate::model::Document,
    mut id: NodeId,
    budget: &mut Budget,
) -> Result<String, CompileError> {
    let mut w = CanonicalWriter::new(budget);
    w.push("[")?;
    let mut first = true;
    loop {
        if !first {
            w.push(",")?;
        }
        first = false;
        w.budget().charge(Resource::Work, 1)?;
        match &doc.node(id)?.kind {
            NodeKind::ListOf { element } => {
                w.push("[\"ListOf\"]")?;
                id = *element;
            }
            NodeKind::WithMode { mode, read } => {
                w.push("[\"WithMode\",")?;
                w.quoted(&mode.value)?;
                w.push("]")?;
                id = *read;
            }
            NodeKind::Local { category } => {
                w.push("[\"Local\",")?;
                w.quoted(&category.value)?;
                w.push("]")?;
                break;
            }
            NodeKind::Foreign { alias, category } => {
                w.push("[\"Foreign\",")?;
                w.quoted(&alias.value)?;
                w.push(",")?;
                w.quoted(&category.value)?;
                w.push("]")?;
                break;
            }
            NodeKind::Builtin { reader } => {
                w.push("[\"Builtin\",")?;
                w.quoted(&reader.value)?;
                w.push("]")?;
                break;
            }
            _ => return Err(issue(id, DeclarationError::InvalidRead)),
        }
    }
    w.push("]")?;
    let bytes = w.finish();
    budget.charge(Resource::Work, bytes.len() as u64 + 21)?;
    let digest = Digest::domain(b"NEPL3-GRAMMAR-READ-1\0", &bytes);
    budget.charge(Resource::AllocationUnits, 69)?;
    budget.charge(Resource::Work, 64)?;
    let mut result = String::from("List:");
    const HEX: &[u8; 16] = b"0123456789abcdef";
    for byte in digest.0 {
        result.push(HEX[(byte >> 4) as usize] as char);
        result.push(HEX[(byte & 15) as usize] as char);
    }
    Ok(result)
}
pub(super) fn descriptor(
    document: &CheckedDocument<'_>,
    context: &PackageContext<'_>,
    revision: u64,
    outputs: &[(String, TypeDescriptor)],
    budget: &mut Budget,
) -> Result<SchemaDescriptor, CompileError> {
    let doc = document.document();
    let mut types = Vec::new();
    for (i, node) in doc.nodes.iter().enumerate() {
        let id = NodeId(i as u64);
        budget.charge(Resource::Work, 1)?;
        match &node.kind {
            NodeKind::Form { kind, fields, .. } => {
                let mut out = Vec::new();
                for field in &fields.items {
                    let NodeKind::FieldDeclaration { name: n, read } = &doc.node(*field)?.kind
                    else {
                        return Err(CompileError::WrongConstructor(*field));
                    };
                    push(
                        &mut out,
                        FieldDescriptor {
                            name: text(&n.value, budget)?,
                            ty: named(
                                if foreign(doc, *read, budget)? {
                                    "ForeignSyntax"
                                } else {
                                    "NodeRef"
                                },
                                budget,
                            )?,
                        },
                        budget,
                    )?;
                }
                record(&mut types, name("Form:", &kind.value, budget)?, out, budget)?;
            }
            NodeKind::Leaf { kind, .. } => record(
                &mut types,
                name("Leaf:", &kind.value, budget)?,
                Vec::new(),
                budget,
            )?,
            NodeKind::Take { kind, reader } => {
                let ty = lookup(outputs, &reader.value, |v| v.0.as_str(), budget)?
                    .ok_or(issue(id, DeclarationError::MissingReader))?
                    .1
                    .clone_with_budget(budget)?;
                record(
                    &mut types,
                    name("Token:", &kind.value, budget)?,
                    payload(ty, budget)?,
                    budget,
                )?;
            }
            NodeKind::Builtin { reader } => {
                let ty = builtin_type(builtin(&reader.value, id)?);
                record(
                    &mut types,
                    name("Builtin:", &reader.value, budget)?,
                    Vec::new(),
                    budget,
                )?;
                record(
                    &mut types,
                    name("BuiltinToken:", &reader.value, budget)?,
                    payload(ty, budget)?,
                    budget,
                )?;
            }
            NodeKind::ListOf { element } => {
                let key = list_name(doc, id, budget)?;
                let mut fields = Vec::new();
                push(
                    &mut fields,
                    FieldDescriptor {
                        name: text("head", budget)?,
                        ty: named(
                            if foreign(doc, *element, budget)? {
                                "ForeignSyntax"
                            } else {
                                "NodeRef"
                            },
                            budget,
                        )?,
                    },
                    budget,
                )?;
                push(
                    &mut fields,
                    FieldDescriptor {
                        name: text("tail", budget)?,
                        ty: named("NodeRef", budget)?,
                    },
                    budget,
                )?;
                record(&mut types, name(&key, ":Cons", budget)?, fields, budget)?;
                record(&mut types, name(&key, ":Nil", budget)?, Vec::new(), budget)?;
            }
            _ => {}
        }
    }
    Ok(SchemaDescriptor {
        package: text(context.package, budget)?,
        revision,
        types,
        operations: Vec::new(),
    })
}
