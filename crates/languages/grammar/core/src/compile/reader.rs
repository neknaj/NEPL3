//! Lower every Grammar ReaderExpr constructor into the shared production reader plan.
use super::*;
use crate::model::{Category, NodeKind};
use alloc::boxed::Box;
use nepl3_reader::plan::{CharClass, ProviderKind, ReaderExpr, ReaderId, ReaderPlan, ReaderRule};
pub(crate) mod output;

fn id(map: &[Option<ReaderId>], id: NodeId) -> Result<ReaderId, CompileError> {
    usize::try_from(id.0)
        .ok()
        .and_then(|i| map.get(i))
        .copied()
        .flatten()
        .ok_or(CompileError::WrongConstructor(id))
}
fn class(
    document: &CheckedDocument<'_>,
    node: NodeId,
    budget: &mut Budget,
) -> Result<CharClass, CompileError> {
    budget.charge(Resource::Work, 1)?;
    Ok(match &document.document().node(node)?.kind {
        NodeKind::Any => CharClass::Any,
        NodeKind::Whitespace => CharClass::Whitespace,
        NodeKind::Identifierstart => CharClass::IdentifierStart,
        NodeKind::Identifiercontinue => CharClass::IdentifierContinue,
        NodeKind::Digit => CharClass::Digit,
        NodeKind::Asciiletter => CharClass::AsciiLetter,
        NodeKind::Chars { text: v } => CharClass::Chars(text(&v.value, budget)?),
        NodeKind::Except { text: v } => CharClass::Except(text(&v.value, budget)?),
        NodeKind::Range { lo, hi } => {
            budget.charge(Resource::Work, (lo.value.len() + hi.value.len()) as u64 + 1)?;
            let mut a = lo.value.chars();
            let mut b = hi.value.chars();
            let lower = a.next();
            let upper = b.next();
            if lower.is_none() || a.next().is_some() {
                return Err(CompileError::InvalidRange.at(&lo.span, Some(&hi.span), budget));
            }
            if upper.is_none() || b.next().is_some() || lower > upper {
                return Err(CompileError::InvalidRange.at(&hi.span, Some(&lo.span), budget));
            }
            let (Some(lo), Some(hi)) = (lower, upper) else {
                return Err(CompileError::WrongConstructor(node));
            };
            CharClass::Range { lo, hi }
        }
        _ => return Err(CompileError::WrongConstructor(node)),
    })
}
fn provider(
    context: &ReaderContext<'_>,
    name: &str,
    kind: ProviderKind,
    budget: &mut Budget,
) -> Result<nepl3_core::value::OperationRef, CompileError> {
    budget.charge(
        Resource::Work,
        (context.imports.len() as u64).saturating_mul(name.len() as u64 + 1),
    )?;
    let import = context
        .imports
        .iter()
        .find(|v| v.provider == name)
        .ok_or(CompileError::MissingProvider)?;
    if import.signature.kind != kind {
        return Err(CompileError::ProviderKind);
    }
    budget.charge(
        Resource::AllocationUnits,
        (import.signature.operation.schema.package.len() + import.signature.operation.name.len())
            as u64,
    )?;
    Ok(import.signature.operation.clone())
}
fn lower(
    document: &CheckedDocument<'_>,
    node: NodeId,
    map: &[Option<ReaderId>],
    context: &ReaderContext<'_>,
    budget: &mut Budget,
) -> Result<ReaderExpr, CompileError> {
    budget.charge(Resource::Work, 1)?;
    let expression = &document.document().node(node)?.kind;
    let one = |v: &NodeId| id(map, *v);
    Ok(match expression {
        NodeKind::Literal { text: v } => ReaderExpr::Literal(text(&v.value, budget)?),
        NodeKind::Scalar { class: c } => ReaderExpr::Scalar(class(document, *c, budget)?),
        NodeKind::Seq { parts }
        | NodeKind::Choice {
            alternatives: parts,
        } => {
            let mut values = Vec::new();
            for value in &parts.items {
                push(&mut values, id(map, *value)?, budget)?;
            }
            if matches!(expression, NodeKind::Seq { .. }) {
                ReaderExpr::Seq(values)
            } else {
                ReaderExpr::Choice(values)
            }
        }
        NodeKind::Many { body } => ReaderExpr::Many(one(body)?),
        NodeKind::Some { body } => ReaderExpr::Some(one(body)?),
        NodeKind::Optional { body } => ReaderExpr::Optional(one(body)?),
        NodeKind::Look { body } => ReaderExpr::Look(one(body)?),
        NodeKind::Not { body } => ReaderExpr::Not(one(body)?),
        NodeKind::Commit { body } => ReaderExpr::Commit(one(body)?),
        NodeKind::Discard { body } => ReaderExpr::Discard(one(body)?),
        NodeKind::Repeat { min, max, body } => {
            let minimum = natural_u64(min, budget)?;
            let maximum = natural_u64(max, budget)?;
            if minimum > maximum {
                return Err(CompileError::InvalidRepeat.at(&max.span, Some(&min.span), budget));
            }
            ReaderExpr::Repeat {
                min: minimum,
                max: maximum,
                body: one(body)?,
            }
        }
        NodeKind::Capture { name, body } => ReaderExpr::Capture {
            name: text(&name.value, budget)?,
            body: one(body)?,
        },
        NodeKind::Region { role, body } => {
            budget.charge(
                Resource::Work,
                (context.classes.len() as u64).saturating_mul(role.value.len() as u64 + 1),
            )?;
            let class = &context
                .classes
                .iter()
                .find(|v| v.name == role.value)
                .ok_or_else(|| CompileError::MissingClass.at(&role.span, None, budget))?
                .class;
            budget.charge(
                Resource::AllocationUnits,
                (class.schema.package.len() + class.name.len()) as u64,
            )?;
            ReaderExpr::Region {
                class: class.clone(),
                body: one(body)?,
            }
        }
        NodeKind::Node { kind, body } => {
            budget.charge(
                Resource::Work,
                (context.views.len() as u64).saturating_mul(kind.value.len() as u64 + 1),
            )?;
            let kind = &context
                .views
                .iter()
                .find(|v| v.name == kind.value)
                .ok_or_else(|| CompileError::MissingView.at(&kind.span, None, budget))?
                .kind;
            budget.charge(Resource::AllocationUnits, kind.schema.package.len() as u64)?;
            ReaderExpr::Node {
                kind: kind.clone(),
                body: one(body)?,
            }
        }
        NodeKind::Ref { name } => {
            let mut found = false;
            for candidate in &document.document().nodes {
                budget.charge(Resource::Work, name.value.len() as u64 + 1)?;
                if matches!(&candidate.kind, NodeKind::Reader { name: declared, .. } if declared.value == name.value)
                {
                    found = true;
                    break;
                }
            }
            if !found {
                return Err(CompileError::MissingRule.at(&name.span, None, budget));
            }
            ReaderExpr::Ref(text(&name.value, budget)?)
        }
        NodeKind::Decode { decoder, body } => ReaderExpr::Decode {
            provider: provider(context, &decoder.value, ProviderKind::Transform, budget)
                .map_err(|error| error.at(&decoder.span, None, budget))?,
            body: one(body)?,
        },
        NodeKind::Map { provider: p, body } => ReaderExpr::Map {
            provider: provider(context, &p.value, ProviderKind::Transform, budget)
                .map_err(|error| error.at(&p.span, None, budget))?,
            body: one(body)?,
        },
        NodeKind::Then { first, provider: p } => ReaderExpr::Then {
            first: one(first)?,
            provider: provider(context, &p.value, ProviderKind::Dependent, budget)
                .map_err(|error| error.at(&p.span, None, budget))?,
        },
        NodeKind::Call { provider: p } => ReaderExpr::Call(
            provider(context, &p.value, ProviderKind::Read, budget)
                .map_err(|error| error.at(&p.span, None, budget))?,
        ),
        NodeKind::Eof => ReaderExpr::Eof,
        NodeKind::Takecount { count } => ReaderExpr::TakeCount(natural_u64(count, budget)?),
        NodeKind::Until { delimiter } => {
            if delimiter.value.is_empty() {
                return Err(CompileError::Reader(PlanError::EmptyDelimiter).at(
                    &delimiter.span,
                    None,
                    budget,
                ));
            }
            ReaderExpr::Until(text(&delimiter.value, budget)?)
        }
        _ => return Err(CompileError::WrongConstructor(node)),
    })
}
/// Compile the reader declarations of a checked complete Grammar AST. The supplied
/// schema/import catalog must be real, registered contracts; calls are never executed.
/// Form/binding/style/package assembly is a separate compiler stage.
pub fn compile(
    document: &CheckedDocument<'_>,
    context: &ReaderContext<'_>,
    budget: &mut Budget,
) -> Result<ReaderPlan, CompileError> {
    context.validate(budget)?;
    let doc = document.document();
    let NodeKind::Language { declarations, .. } = &doc.node(doc.root)?.kind else {
        return Err(CompileError::WrongConstructor(doc.root));
    };
    budget.charge(
        Resource::AllocationUnits,
        (doc.nodes.len() as u64).saturating_mul(core::mem::size_of::<Option<ReaderId>>() as u64),
    )?;
    let mut map = alloc::vec![None;doc.nodes.len()];
    let mut count = 0u64;
    for (i, node) in doc.nodes.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if node.kind.category() == Category::ReaderExpr {
            map[i] = Some(ReaderId(count));
            count += 1;
        }
    }
    let mut expressions = Vec::new();
    for (i, slot) in map.iter().enumerate() {
        if slot.is_some() {
            let expr = lower(document, NodeId(i as u64), &map, context, budget)
                .map_err(|error| error.at_node(doc, NodeId(i as u64), budget))?;
            push(&mut expressions, expr, budget)?;
        }
    }
    let mut roots: Vec<(String, ReaderId)> = Vec::new();
    for declaration in &declarations.items {
        if let NodeKind::Reader { name, expression } = &doc.node(*declaration)?.kind {
            budget.charge(
                Resource::Work,
                (roots.len() as u64).saturating_mul(name.value.len() as u64 + 1),
            )?;
            if roots.iter().any(|(n, _)| n == &name.value) {
                return Err(CompileError::DuplicateRule.at(&name.span, None, budget));
            }
            push(
                &mut roots,
                (text(&name.value, budget)?, id(&map, *expression)?),
                budget,
            )?;
        }
    }
    let outputs = infer(&expressions, &roots, context, budget)?;
    let mut rules = Vec::new();
    for (name, root) in roots {
        let output = outputs
            .get(root.0 as usize)
            .ok_or(CompileError::OutputType)?
            .clone_with_budget(budget)?;
        push(&mut rules, ReaderRule { name, root, output }, budget)?;
    }
    let mut providers = Vec::new();
    for import in context.imports {
        let s = &import.signature;
        budget.charge(
            Resource::AllocationUnits,
            (s.operation.schema.package.len() + s.operation.name.len()) as u64
                + core::mem::size_of::<ProviderSignature>() as u64,
        )?;
        providers.push(ProviderSignature {
            operation: s.operation.clone(),
            kind: s.kind,
            pure: s.pure,
            value_input: s.value_input.clone_with_budget(budget)?,
            value_output: s.value_output.clone_with_budget(budget)?,
            state_type: s.state_type.clone_with_budget(budget)?,
            continuation_type: s.continuation_type.clone_with_budget(budget)?,
        });
    }
    budget.charge(
        Resource::AllocationUnits,
        context.schema.package.len() as u64,
    )?;
    let plan = ReaderPlan {
        schema: context.schema.clone(),
        state_type: context.state_type.clone_with_budget(budget)?,
        expressions,
        rules,
        providers,
    };
    if let Err(failure) = plan.check_detailed(context.registry, budget) {
        let error = CompileError::from(failure.error);
        if let Some(expression) = failure.expression {
            for (index, id) in map.iter().enumerate() {
                budget.charge(Resource::Work, 1)?;
                if *id == Some(expression) {
                    if let ReaderExpr::Choice(parts) = &plan.expressions[expression.0 as usize]
                        && let Some(first) = parts.first()
                    {
                        for part in parts.iter().skip(1) {
                            budget.charge(Resource::Work, 1)?;
                            if !same_output(
                                &outputs[part.0 as usize],
                                &outputs[first.0 as usize],
                                budget,
                            )? {
                                let primary = source_node(&map, *part, budget)?;
                                let related = source_node(&map, *first, budget)?;
                                return Err(error
                                    .at(
                                        &doc.node(primary)?.span,
                                        Some(&doc.node(related)?.span),
                                        budget,
                                    )
                                    .with_types(
                                        &outputs[first.0 as usize],
                                        &outputs[part.0 as usize],
                                        budget,
                                    ));
                            }
                        }
                    }
                    return Err(error.at_node(doc, NodeId(index as u64), budget));
                }
            }
        }
        return Err(error);
    }
    Ok(plan)
}
fn infer(
    expressions: &[ReaderExpr],
    roots: &[(String, ReaderId)],
    context: &ReaderContext<'_>,
    budget: &mut Budget,
) -> Result<Vec<TypeDescriptor>, CompileError> {
    budget.charge(
        Resource::AllocationUnits,
        (expressions.len() as u64)
            .saturating_mul(core::mem::size_of::<Option<TypeDescriptor>>() as u64),
    )?;
    let mut types = alloc::vec![None;expressions.len()];
    loop {
        let mut progress = false;
        for (i, expression) in expressions.iter().enumerate() {
            budget.charge(Resource::Work, 1)?;
            if types[i].is_some() {
                continue;
            }
            let get = |id: ReaderId| types.get(id.0 as usize).and_then(Option::as_ref);
            let borrowed = match expression {
                ReaderExpr::Commit(v)
                | ReaderExpr::Capture { body: v, .. }
                | ReaderExpr::Region { body: v, .. }
                | ReaderExpr::Node { body: v, .. } => get(*v),
                ReaderExpr::Choice(v) => v.iter().find_map(|id| get(*id)),
                ReaderExpr::Ref(name) => {
                    budget.charge(
                        Resource::Work,
                        (roots.len() as u64).saturating_mul(name.len() as u64 + 1),
                    )?;
                    let root = roots
                        .iter()
                        .find(|(n, _)| n == name)
                        .ok_or(CompileError::MissingRule)?
                        .1;
                    get(root)
                }
                ReaderExpr::Call(op)
                | ReaderExpr::Map { provider: op, .. }
                | ReaderExpr::Decode { provider: op, .. }
                | ReaderExpr::Then { provider: op, .. } => {
                    budget.charge(
                        Resource::Work,
                        (context.imports.len() as u64)
                            .saturating_mul((op.name.len() + op.schema.package.len()) as u64 + 41),
                    )?;
                    Some(
                        &context
                            .imports
                            .iter()
                            .find(|v| v.signature.operation == *op)
                            .ok_or(CompileError::MissingProvider)?
                            .signature
                            .value_output,
                    )
                }
                _ => None,
            };
            let ty = if let Some(ty) = borrowed {
                Some(ty.clone_with_budget(budget)?)
            } else {
                match expression {
                    ReaderExpr::Literal(_)
                    | ReaderExpr::Look(_)
                    | ReaderExpr::Not(_)
                    | ReaderExpr::Discard(_)
                    | ReaderExpr::Eof => Some(TypeDescriptor::Unit),
                    ReaderExpr::Scalar(_) | ReaderExpr::TakeCount(_) | ReaderExpr::Until(_) => {
                        Some(TypeDescriptor::Text)
                    }
                    ReaderExpr::Seq(_) => {
                        budget.charge(
                            Resource::AllocationUnits,
                            core::mem::size_of::<TypeDescriptor>() as u64,
                        )?;
                        Some(TypeDescriptor::List(Box::new(TypeDescriptor::NdfValue)))
                    }
                    ReaderExpr::Many(body)
                    | ReaderExpr::Some(body)
                    | ReaderExpr::Repeat { body, .. }
                    | ReaderExpr::Optional(body) => {
                        if let Some(ty) = get(*body) {
                            let ty = ty.clone_with_budget(budget)?;
                            budget.charge(
                                Resource::AllocationUnits,
                                core::mem::size_of::<TypeDescriptor>() as u64,
                            )?;
                            Some(if matches!(expression, ReaderExpr::Optional(_)) {
                                TypeDescriptor::Option(Box::new(ty))
                            } else {
                                TypeDescriptor::List(Box::new(ty))
                            })
                        } else {
                            None
                        }
                    }
                    _ => None,
                }
            };
            if let Some(ty) = ty {
                types[i] = Some(ty);
                progress = true;
            }
        }
        if !progress {
            break;
        }
    }
    let mut result = Vec::new();
    for ty in types {
        push(&mut result, ty.ok_or(CompileError::OutputType)?, budget)?;
    }
    Ok(result)
}

fn source_node(
    map: &[Option<ReaderId>],
    reader: ReaderId,
    budget: &mut Budget,
) -> Result<NodeId, CompileError> {
    for (index, candidate) in map.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if *candidate == Some(reader) {
            return Ok(NodeId(index as u64));
        }
    }
    Err(CompileError::OutputType)
}

// Descriptor equality must account for symbolic-name bytes before comparing them.
fn same_output(
    mut left: &TypeDescriptor,
    mut right: &TypeDescriptor,
    budget: &mut Budget,
) -> Result<bool, CompileError> {
    let mut depth = 1;
    loop {
        budget.observe_depth(depth)?;
        budget.charge(Resource::Work, 1)?;
        match (left, right) {
            (TypeDescriptor::List(a), TypeDescriptor::List(b))
            | (TypeDescriptor::Option(a), TypeDescriptor::Option(b)) => {
                left = a;
                right = b;
                depth += 1;
            }
            (TypeDescriptor::Named(a), TypeDescriptor::Named(b)) => {
                budget.charge(
                    Resource::Work,
                    (a.package.len() + a.name.len() + b.package.len() + b.name.len()) as u64 + 1,
                )?;
                return Ok(a == b);
            }
            _ => return Ok(core::mem::discriminant(left) == core::mem::discriminant(right)),
        }
    }
}
