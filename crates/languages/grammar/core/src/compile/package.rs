//! Complete declaration assembly. The registry is owned by the transaction:
//! an error publishes neither a partially registered package nor an executable callback.
use super::*;
use crate::model::{Category as AstCategory, NodeKind};
use nepl3_core::{
    origin::{Origin, OriginId},
    schema::{FieldDescriptor, NamedType, SchemaDescriptor, TypeRef, TypeShape},
    source::Digest,
};
use nepl3_engine::{
    package as p,
    recovery::{RecoveryPlan, UnexpectedPolicy},
};
use nepl3_reader::{
    builtin::BuiltinReader,
    tokenizer::{ReaderMode, SkipRule, TakeRule, TokenReader},
};
mod surface;

pub struct PackageContext<'a> {
    /// Explicit surface package owner selected by the compiling host.
    pub package: &'a str,
    pub state_type: &'a TypeDescriptor,
    pub reader_imports: &'a [ReaderImport],
    /// Known operation contracts; source extension declarations select these.
    /// No runtime implementation is invoked or implied by this catalog.
    pub extensions: &'a [p::ExtensionRequirement],
    pub classes: &'a [NamedClass],
    pub views: &'a [NamedView],
}
pub struct CompiledLanguage {
    pub package: p::LanguagePackage,
    pub registry: SchemaRegistry,
}
fn issue(node: NodeId, reason: DeclarationError) -> CompileError {
    CompileError::Declaration {
        node,
        related: None,
        reason,
    }
}
fn name(prefix: &str, value: &str, budget: &mut Budget) -> Result<String, CompileError> {
    budget.charge(Resource::Work, (prefix.len() + value.len()) as u64 + 1)?;
    budget.charge(
        Resource::AllocationUnits,
        (prefix.len() + value.len()) as u64,
    )?;
    let mut result = String::with_capacity(prefix.len() + value.len());
    result.push_str(prefix);
    result.push_str(value);
    Ok(result)
}
fn kind(
    registry: &SchemaRegistry,
    schema: &SchemaRef,
    value: &str,
    budget: &mut Budget,
) -> Result<KindRef, CompileError> {
    budget.charge(Resource::Work, value.len() as u64 + 1)?;
    budget.charge(Resource::AllocationUnits, schema.package.len() as u64)?;
    Ok(KindRef {
        schema: schema.clone(),
        local_kind: registry.kind_id(schema, value)?,
    })
}
fn builtin(value: &str, node: NodeId) -> Result<BuiltinReader, CompileError> {
    Ok(match value {
        "Name" => BuiltinReader::Name,
        "Text" => BuiltinReader::Text,
        "Nat" => BuiltinReader::Nat,
        "Number" => BuiltinReader::Number,
        "Lang" => BuiltinReader::Lang,
        "Trivia" => BuiltinReader::Trivia,
        _ => return Err(issue(node, DeclarationError::InvalidBuiltin)),
    })
}
fn builtin_type(value: BuiltinReader) -> TypeDescriptor {
    match value {
        BuiltinReader::Name | BuiltinReader::Text | BuiltinReader::Lang => TypeDescriptor::Text,
        BuiltinReader::Nat => TypeDescriptor::Integer,
        BuiltinReader::Number => TypeDescriptor::Rational,
        BuiltinReader::Trivia => TypeDescriptor::Unit,
    }
}
fn lookup<'a, T>(
    values: &'a [T],
    value: &str,
    key: impl Fn(&T) -> &str,
    budget: &mut Budget,
) -> Result<Option<&'a T>, CompileError> {
    for candidate in values {
        let name = key(candidate);
        budget.charge(Resource::Work, name.len().min(value.len()) as u64 + 1)?;
        if name == value {
            return Ok(Some(candidate));
        }
    }
    Ok(None)
}
fn mapped<T: Copy>(map: &[Option<T>], id: NodeId) -> Result<T, CompileError> {
    map.get(id.0 as usize)
        .copied()
        .flatten()
        .ok_or(CompileError::WrongConstructor(id))
}
fn fields(
    doc: &crate::model::Document,
    list: &crate::model::NodeList,
    reads: &[Option<p::ReadSpecId>],
    budget: &mut Budget,
) -> Result<Vec<p::FieldSpec>, CompileError> {
    let mut result = Vec::new();
    for id in &list.items {
        let NodeKind::FieldDeclaration { name, read } = &doc.node(*id)?.kind else {
            return Err(CompileError::WrongConstructor(*id));
        };
        push(
            &mut result,
            p::FieldSpec {
                name: text(&name.value, budget)?,
                read: mapped(reads, *read)?,
            },
            budget,
        )?;
    }
    Ok(result)
}
fn selector(
    doc: &crate::model::Document,
    id: NodeId,
    budget: &mut Budget,
) -> Result<p::StyleSelector, CompileError> {
    Ok(match &doc.node(id)?.kind {
        NodeKind::Head => p::StyleSelector::Head,
        NodeKind::SelfSelector => p::StyleSelector::SelfValue,
        NodeKind::FieldSelector { name } => p::StyleSelector::Field(text(&name.value, budget)?),
        NodeKind::CaptureSelector { name } => p::StyleSelector::Capture(text(&name.value, budget)?),
        _ => return Err(issue(id, DeclarationError::InvalidSelector)),
    })
}
fn styles(
    doc: &crate::model::Document,
    list: &crate::model::NodeList,
    context: &PackageContext<'_>,
    budget: &mut Budget,
) -> Result<Vec<p::StyleRule>, CompileError> {
    let mut result = Vec::new();
    for id in &list.items {
        let NodeKind::Style { selector: s, class } = &doc.node(*id)?.kind else {
            return Err(CompileError::WrongConstructor(*id));
        };
        let class = &lookup(context.classes, &class.value, |v| v.name.as_str(), budget)?
            .ok_or(issue(*id, DeclarationError::MissingClass))?
            .class;
        budget.charge(
            Resource::AllocationUnits,
            (class.schema.package.len() + class.name.len()) as u64,
        )?;
        push(
            &mut result,
            p::StyleRule {
                selector: selector(doc, *s, budget)?,
                class: class.clone(),
            },
            budget,
        )?;
    }
    Ok(result)
}
/// Compile all declarations into an actual registered surface schema and checked
/// package. Runtime providers are intentionally outside this operation.
pub fn compile(
    document: &CheckedDocument<'_>,
    context: &PackageContext<'_>,
    mut registry: SchemaRegistry,
    budget: &mut Budget,
) -> Result<CompiledLanguage, CompileError> {
    budget.charge(Resource::Work, 1)?;
    let doc = document.document();
    let NodeKind::Language {
        revision,
        root,
        declarations,
        ..
    } = &doc.node(doc.root)?.kind
    else {
        return Err(CompileError::WrongConstructor(doc.root));
    };
    let revision = natural_u64(revision, budget)?;
    // Validate independent catalog names even if source happens not to use one.
    let registry_schema = registry
        .selected("nepl3.foundation", 1)
        .ok_or(SchemaError::UnknownSchema)?;
    ReaderContext {
        schema: registry_schema,
        state_type: context.state_type,
        imports: context.reader_imports,
        classes: context.classes,
        views: context.views,
        registry: &registry,
    }
    .validate(budget)?;
    for (i, item) in context.extensions.iter().enumerate() {
        if item.provider.is_empty() {
            return Err(issue(doc.root, DeclarationError::EmptyName));
        }
        if lookup(
            &context.extensions[..i],
            &item.provider,
            |v| v.provider.as_str(),
            budget,
        )?
        .is_some()
        {
            return Err(issue(doc.root, DeclarationError::DuplicateName));
        }
    }
    let mut declared: Vec<(p::DeclarationKind, String, NodeId)> = Vec::new();
    let mut extensions = Vec::new();
    let mut imports = Vec::new();
    for id in &declarations.items {
        let (kind_value, value) = match &doc.node(*id)?.kind {
            NodeKind::Category { name, .. } => (p::DeclarationKind::Category, &name.value),
            NodeKind::Mode { name, .. } => (p::DeclarationKind::Mode, &name.value),
            NodeKind::Reader { name, .. } => (p::DeclarationKind::Reader, &name.value),
            NodeKind::Form { kind, .. } => (p::DeclarationKind::Form, &kind.value),
            NodeKind::Leaf { kind, .. } => (p::DeclarationKind::Leaf, &kind.value),
            NodeKind::Namespace { name, .. } => (p::DeclarationKind::Namespace, &name.value),
            NodeKind::Extension { alias, .. } => (p::DeclarationKind::Extension, &alias.value),
            _ => return Err(CompileError::WrongConstructor(*id)),
        };
        budget.charge(Resource::Work, value.len() as u64 + 1)?;
        if value.is_empty() {
            return Err(issue(*id, DeclarationError::EmptyName));
        }
        for (k, n, prior) in &declared {
            budget.charge(Resource::Work, n.len().min(value.len()) as u64 + 1)?;
            if *k == kind_value && n == value {
                return Err(CompileError::Declaration {
                    node: *id,
                    related: Some(*prior),
                    reason: DeclarationError::DuplicateName,
                });
            }
        }
        push(
            &mut declared,
            (kind_value, text(value, budget)?, *id),
            budget,
        )?;
        if let NodeKind::Extension {
            alias,
            provider,
            signature,
        } = &doc.node(*id)?.kind
        {
            let ext = lookup(
                context.extensions,
                &provider.value,
                |v| v.provider.as_str(),
                budget,
            )?
            .ok_or(issue(*id, DeclarationError::MissingExtension))?;
            if signature.value != ext.signature {
                return Err(issue(*id, DeclarationError::Signature));
            }
            let operation = registry
                .descriptor(&ext.operation.schema)
                .and_then(|d| d.operations.iter().find(|v| v.name == ext.operation.name))
                .ok_or(issue(*id, DeclarationError::Signature))?;
            if operation.input != ext.input
                || operation.output != ext.output
                || operation.pure != ext.pure
            {
                return Err(issue(*id, DeclarationError::Signature));
            }
            budget.charge(
                Resource::AllocationUnits,
                (ext.operation.schema.package.len() + ext.operation.name.len()) as u64,
            )?;
            push(
                &mut extensions,
                p::ExtensionRequirement {
                    alias: text(&alias.value, budget)?,
                    provider: text(&provider.value, budget)?,
                    signature: text(&signature.value, budget)?,
                    operation: ext.operation.clone(),
                    input: ext.input.clone_with_budget(budget)?,
                    output: ext.output.clone_with_budget(budget)?,
                    pure: ext.pure,
                },
                budget,
            )?;
            if let Some(import) = lookup(
                context.reader_imports,
                &provider.value,
                |v| v.provider.as_str(),
                budget,
            )? {
                if import.signature.operation != ext.operation {
                    return Err(issue(*id, DeclarationError::Signature));
                }
                // The reader compiler accounts the owned signature copy itself.
                budget.charge(
                    Resource::AllocationUnits,
                    (core::mem::size_of::<ReaderImport>()
                        + provider.value.len()
                        + import.signature.operation.name.len()
                        + import.signature.operation.schema.package.len())
                        as u64,
                )?;
                let s = &import.signature;
                push(
                    &mut imports,
                    ReaderImport {
                        provider: text(&provider.value, budget)?,
                        signature: ProviderSignature {
                            operation: s.operation.clone(),
                            kind: s.kind,
                            pure: s.pure,
                            value_input: s.value_input.clone_with_budget(budget)?,
                            value_output: s.value_output.clone_with_budget(budget)?,
                            state_type: s.state_type.clone_with_budget(budget)?,
                            continuation_type: s.continuation_type.clone_with_budget(budget)?,
                        },
                    },
                    budget,
                )?;
            }
        }
    }
    let outputs = reader::output::infer_document(document, &imports, budget)?;
    let descriptor = surface::descriptor(document, context, revision, &outputs, budget)?;
    let schema = descriptor.reference(budget)?;
    budget.charge(Resource::AllocationUnits, schema.package.len() as u64)?;
    registry.register(schema.clone(), descriptor, budget)?;
    registry.finalize(budget)?;
    let reader = reader::compile(
        document,
        &ReaderContext {
            schema: &schema,
            state_type: context.state_type,
            imports: &imports,
            classes: context.classes,
            views: context.views,
            registry: &registry,
        },
        budget,
    )?;
    let mut reads = Vec::new();
    let mut read_ids = Vec::new();
    let mut binding_ids = Vec::new();
    let mut bindings = Vec::new();
    for node in &doc.nodes {
        push(
            &mut read_ids,
            if node.kind.category() == AstCategory::ReadSpec {
                let id = p::ReadSpecId(reads.len() as u64);
                push(&mut reads, None, budget)?;
                Some(id)
            } else {
                None
            },
            budget,
        )?;
        push(
            &mut binding_ids,
            if node.kind.category() == AstCategory::Binding {
                let id = p::BindingId(bindings.len() as u64);
                push(&mut bindings, None, budget)?;
                Some(id)
            } else {
                None
            },
            budget,
        )?;
    }
    for (i, node) in doc.nodes.iter().enumerate() {
        let id = NodeId(i as u64);
        budget.charge(Resource::Work, 1)?;
        if let Some(slot) = read_ids[i] {
            let value = match &node.kind {
                NodeKind::Builtin { reader } => p::ReadSpec::Builtin {
                    reader: builtin(&reader.value, id)?,
                    kind: kind(
                        &registry,
                        &schema,
                        &name("Builtin:", &reader.value, budget)?,
                        budget,
                    )?,
                    token_kind: kind(
                        &registry,
                        &schema,
                        &name("BuiltinToken:", &reader.value, budget)?,
                        budget,
                    )?,
                },
                NodeKind::Local { category } => p::ReadSpec::Local {
                    category: text(&category.value, budget)?,
                },
                NodeKind::Foreign { alias, category } => p::ReadSpec::Foreign {
                    alias: text(&alias.value, budget)?,
                    category: text(&category.value, budget)?,
                },
                NodeKind::WithMode { mode, read } => p::ReadSpec::WithMode {
                    mode: text(&mode.value, budget)?,
                    read: mapped(&read_ids, *read)?,
                },
                NodeKind::ListOf { element } => {
                    let key = surface::list_name(doc, id, budget)?;
                    p::ReadSpec::ListOf {
                        element: mapped(&read_ids, *element)?,
                        cons: kind(&registry, &schema, &name(&key, ":Cons", budget)?, budget)?,
                        nil: kind(&registry, &schema, &name(&key, ":Nil", budget)?, budget)?,
                    }
                }
                _ => return Err(issue(id, DeclarationError::InvalidRead)),
            };
            reads[slot.0 as usize] = Some(value);
        }
        if let Some(slot) = binding_ids[i] {
            bindings[slot.0 as usize] = Some(binding(doc, id, &binding_ids, &extensions, budget)?);
        }
    }
    let mut categories = Vec::new();
    let mut modes = Vec::new();
    let mut forms = Vec::new();
    let mut leaves = Vec::new();
    let mut namespaces = Vec::new();
    for id in &declarations.items {
        match &doc.node(*id)?.kind {
            NodeKind::Category { name, mode } => push(
                &mut categories,
                p::Category {
                    name: text(&name.value, budget)?,
                    mode: text(&mode.value, budget)?,
                },
                budget,
            )?,
            NodeKind::Namespace { name, policy } => push(
                &mut namespaces,
                p::Namespace {
                    name: text(&name.value, budget)?,
                    policy: match doc.node(*policy)?.kind {
                        NodeKind::Lexical => p::NamespacePolicy::Lexical,
                        NodeKind::Global => p::NamespacePolicy::Global,
                        NodeKind::Open => p::NamespacePolicy::Open,
                        _ => return Err(CompileError::WrongConstructor(*policy)),
                    },
                },
                budget,
            )?,
            NodeKind::Mode {
                name: mode_name,
                rules,
            } => {
                let mut skip = Vec::new();
                let mut take = Vec::new();
                for rule in &rules.items {
                    match &doc.node(*rule)?.kind {
                        NodeKind::Skip { reader } => push(
                            &mut skip,
                            SkipRule {
                                reader: TokenReader::Rule(text(&reader.value, budget)?),
                            },
                            budget,
                        )?,
                        NodeKind::Take { kind: k, reader } => push(
                            &mut take,
                            TakeRule {
                                reader: TokenReader::Rule(text(&reader.value, budget)?),
                                kind: kind(
                                    &registry,
                                    &schema,
                                    &name("Token:", &k.value, budget)?,
                                    budget,
                                )?,
                            },
                            budget,
                        )?,
                        _ => return Err(CompileError::WrongConstructor(*rule)),
                    }
                }
                push(
                    &mut modes,
                    ReaderMode {
                        name: text(&mode_name.value, budget)?,
                        skip,
                        take,
                    },
                    budget,
                )?;
            }
            NodeKind::Form {
                kind: k,
                category,
                spelling,
                fields: f,
                bindings: b,
                styles: s,
            } => push(
                &mut forms,
                p::Form {
                    category: text(&category.value, budget)?,
                    kind: kind(
                        &registry,
                        &schema,
                        &name("Form:", &k.value, budget)?,
                        budget,
                    )?,
                    spelling: text(&spelling.value, budget)?,
                    fields: fields(doc, f, &read_ids, budget)?,
                    binding: mapped(&binding_ids, *b)?,
                    styles: styles(doc, s, context, budget)?,
                },
                budget,
            )?,
            NodeKind::Leaf {
                kind: k,
                category,
                token,
                bindings: b,
                styles: s,
            } => {
                let payload = surface::token_type(doc, &token.value, &outputs, budget)?
                    .ok_or(issue(*id, DeclarationError::MissingToken))?;
                push(
                    &mut leaves,
                    p::Leaf {
                        category: text(&category.value, budget)?,
                        kind: kind(
                            &registry,
                            &schema,
                            &name("Leaf:", &k.value, budget)?,
                            budget,
                        )?,
                        token_kind: kind(
                            &registry,
                            &schema,
                            &name("Token:", &token.value, budget)?,
                            budget,
                        )?,
                        payload,
                        binding: mapped(&binding_ids, *b)?,
                        styles: styles(doc, s, context, budget)?,
                    },
                    budget,
                )?;
            }
            NodeKind::Extension { .. } | NodeKind::Reader { .. } => {}
            _ => return Err(CompileError::WrongConstructor(*id)),
        }
    }
    let mut origins = Vec::new();
    let mut declarations = Vec::new();
    let mut sources = Vec::new();
    for source in &doc.sources {
        push(&mut sources, source.clone_with_budget(budget)?, budget)?;
    }
    for (kind, name, id) in declared {
        let span = &doc.node(id)?.span;
        budget.charge(
            Resource::AllocationUnits,
            (core::mem::size_of_val(span) + span.snapshot_ref().source.0.len()) as u64,
        )?;
        let origin = Origin::Direct(span.clone());
        let oid = OriginId(origins.len() as u64);
        push(&mut origins, origin, budget)?;
        push(
            &mut declarations,
            p::DeclarationOrigin {
                kind,
                name,
                origin: oid,
            },
            budget,
        )?;
    }
    let mut final_reads = Vec::new();
    for value in reads {
        push(
            &mut final_reads,
            value.ok_or(CompileError::OutputType)?,
            budget,
        )?;
    }
    let mut final_bindings = Vec::new();
    for value in bindings {
        push(
            &mut final_bindings,
            value.ok_or(CompileError::OutputType)?,
            budget,
        )?;
    }
    let package = p::LanguagePackage {
        schema,
        payload_schemas: Vec::new(),
        root: text(&root.value, budget)?,
        reader,
        modes,
        categories,
        reads: final_reads,
        forms,
        leaves,
        namespaces,
        bindings: final_bindings,
        extensions,
        recovery: RecoveryPlan {
            default_unexpected: UnexpectedPolicy::PreserveRemainder,
            rules: Vec::new(),
        },
        provenance: p::PackageProvenance {
            sources,
            origins,
            source_maps: Vec::new(),
            declarations,
        },
    };
    package.check(&registry, budget)?;
    Ok(CompiledLanguage { package, registry })
}
fn binding(
    doc: &crate::model::Document,
    id: NodeId,
    map: &[Option<p::BindingId>],
    extensions: &[p::ExtensionRequirement],
    budget: &mut Budget,
) -> Result<p::Binding, CompileError> {
    Ok(match &doc.node(id)?.kind {
        NodeKind::None => p::Binding::None,
        NodeKind::Visit { child } => p::Binding::Visit(text(&child.value, budget)?),
        NodeKind::Import { child } => p::Binding::Import(text(&child.value, budget)?),
        NodeKind::Propagate { child } => p::Binding::Propagate(text(&child.value, budget)?),
        NodeKind::Group { plans } | NodeKind::Scope { plans } => {
            let mut values = Vec::new();
            for child in &plans.items {
                push(&mut values, mapped(map, *child)?, budget)?;
            }
            if matches!(doc.node(id)?.kind, NodeKind::Group { .. }) {
                p::Binding::Group(values)
            } else {
                p::Binding::Scope(values)
            }
        }
        NodeKind::Bind { namespace, field }
        | NodeKind::Reference { namespace, field }
        | NodeKind::Export { namespace, field } => {
            let namespace = text(&namespace.value, budget)?;
            let field = if field.value == "self" {
                p::NameSelector::SelfValue
            } else {
                p::NameSelector::Field(text(&field.value, budget)?)
            };
            match &doc.node(id)?.kind {
                NodeKind::Bind { .. } => p::Binding::Bind {
                    namespace,
                    name: field,
                },
                NodeKind::Reference { .. } => p::Binding::Reference {
                    namespace,
                    name: field,
                },
                _ => p::Binding::Export {
                    namespace,
                    name: field,
                },
            }
        }
        NodeKind::Sequential { declarations, body } => p::Binding::Sequential {
            declarations: text(&declarations.value, budget)?,
            body: text(&body.value, budget)?,
        },
        NodeKind::Recursive { declarations, body } => p::Binding::Recursive {
            declarations: text(&declarations.value, budget)?,
            body: text(&body.value, budget)?,
        },
        NodeKind::Custom { provider } => {
            let ext = lookup(extensions, &provider.value, |v| v.provider.as_str(), budget)?
                .ok_or(issue(id, DeclarationError::MissingExtension))?;
            if ext.signature != "facts/v1" {
                return Err(issue(id, DeclarationError::Signature));
            }
            budget.charge(
                Resource::AllocationUnits,
                (ext.operation.schema.package.len() + ext.operation.name.len()) as u64,
            )?;
            p::Binding::Custom(ext.operation.clone())
        }
        _ => return Err(issue(id, DeclarationError::InvalidBinding)),
    })
}
