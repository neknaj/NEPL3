use super::{check::unique, *};
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{FieldDescriptor, SchemaRegistry, TypeDescriptor, TypeShape},
    value::KindRef,
};
use nepl3_reader::{builtin::BuiltinReader, tokenizer::TokenReader};

pub(super) fn record_kind<'a>(
    kind: &KindRef,
    registry: &'a SchemaRegistry,
    budget: &mut Budget,
) -> Result<&'a [FieldDescriptor], PackageError> {
    let name = registry.kind_name(&kind.schema, kind.local_kind)?;
    let descriptor = registry
        .descriptor(&kind.schema)
        .ok_or(PackageError::KindShape)?;
    budget.charge(Resource::Work, descriptor.types.len() as u64 + 1)?;
    let ty = descriptor
        .types
        .iter()
        .find(|v| v.name == name)
        .ok_or(PackageError::KindShape)?;
    match &ty.shape {
        TypeShape::Record { fields } => Ok(fields),
        _ => Err(PackageError::KindShape),
    }
}
pub(super) fn builtin_type(reader: BuiltinReader) -> TypeDescriptor {
    match reader {
        BuiltinReader::Name | BuiltinReader::Text | BuiltinReader::Lang => TypeDescriptor::Text,
        BuiltinReader::Nat => TypeDescriptor::Integer,
        BuiltinReader::Number => TypeDescriptor::Rational,
        BuiltinReader::Trivia => TypeDescriptor::Unit,
    }
}
fn named(ty: &TypeDescriptor, name: &str) -> bool {
    matches!(ty,TypeDescriptor::Named(r) if r.package=="nepl3.foundation"&&r.revision==1&&r.name==name)
}
pub(super) fn terminal<'a>(
    package: &'a LanguagePackage,
    mut id: ReadSpecId,
    budget: &mut Budget,
) -> Result<&'a ReadSpec, PackageError> {
    for depth in 0..=package.reads.len() {
        budget.charge(Resource::Work, 1)?;
        budget.observe_depth(depth as u64 + 1)?;
        match package.read(id)? {
            ReadSpec::WithMode { read, .. } => id = *read,
            read => return Ok(read),
        }
    }
    Err(PackageError::DirectCycle)
}
pub(super) fn literal(
    package: &LanguagePackage,
    id: ReadSpecId,
    budget: &mut Budget,
) -> Result<bool, PackageError> {
    Ok(matches!(
        terminal(package, id, budget)?,
        ReadSpec::Builtin { .. }
    ))
}
pub(super) fn name_read(
    package: &LanguagePackage,
    id: ReadSpecId,
    budget: &mut Budget,
) -> Result<bool, PackageError> {
    Ok(match terminal(package, id, budget)? {
        ReadSpec::Builtin {
            reader: BuiltinReader::Name | BuiltinReader::Text,
            ..
        } => true,
        ReadSpec::Local { category } => {
            let mut any = false;
            for form in &package.forms {
                budget.charge(Resource::Work, 1)?;
                if &form.category == category {
                    return Ok(false);
                }
            }
            for leaf in &package.leaves {
                budget.charge(Resource::Work, 1)?;
                if &leaf.category == category {
                    any = true;
                    if leaf.payload != TypeDescriptor::Text {
                        return Ok(false);
                    }
                }
            }
            any
        }
        _ => false,
    })
}
pub(super) fn check(
    package: &LanguagePackage,
    registry: &SchemaRegistry,
    budget: &mut Budget,
    subject: &mut Option<PackageSubject>,
) -> Result<(), PackageError> {
    // Each ReadSpec has at most one direct edge; categories carry recursion by name.
    for (root, read) in package.reads.iter().enumerate() {
        *subject = Some(PackageSubject::Read(ReadSpecId(root as u64)));
        let mut current = ReadSpecId(root as u64);
        for depth in 0..=package.reads.len() {
            budget.charge(Resource::Work, 1)?;
            budget.observe_depth(depth as u64 + 1)?;
            if depth == package.reads.len() {
                return Err(PackageError::DirectCycle);
            }
            match package.read(current)? {
                ReadSpec::WithMode { read, .. } => current = *read,
                ReadSpec::ListOf { element, .. } => current = *element,
                _ => break,
            }
        }
        match read {
            ReadSpec::Builtin {
                reader,
                kind,
                token_kind,
            } => {
                if kind.schema != package.schema
                    || token_kind.schema != package.schema
                    || !record_kind(kind, registry, budget)?.is_empty()
                {
                    return Err(PackageError::KindShape);
                }
                let fields = record_kind(token_kind, registry, budget)?;
                if fields.len() != 1
                    || fields[0].name != "payload"
                    || fields[0].ty != builtin_type(*reader)
                {
                    return Err(PackageError::KindShape);
                }
            }
            ReadSpec::Local { category } => {
                package.category(category)?;
            }
            ReadSpec::Foreign { alias, category } => {
                if alias.is_empty() || category.is_empty() {
                    return Err(PackageError::EmptyName);
                }
            }
            ReadSpec::WithMode { mode, read } => {
                if mode.is_empty() {
                    return Err(PackageError::EmptyName);
                }
                // The root owner of Foreign is its guest; a resolved profile checks
                // that mode later. A host mode with the same name proves nothing.
                let guest = matches!(terminal(package, *read, budget)?, ReadSpec::Foreign { .. });
                budget.charge(Resource::Work, package.modes.len() as u64 + 1)?;
                if !guest && !package.modes.iter().any(|v| &v.name == mode) {
                    return Err(PackageError::MissingMode);
                }
            }
            ReadSpec::ListOf { element, cons, nil } => {
                if cons.schema != package.schema || nil.schema != package.schema {
                    return Err(PackageError::KindShape);
                }
                let fields = record_kind(cons, registry, budget)?;
                let head_type = if matches!(
                    terminal(package, *element, budget)?,
                    ReadSpec::Foreign { .. }
                ) {
                    "ForeignSyntax"
                } else {
                    "NodeRef"
                };
                if fields.len() != 2
                    || fields[0].name != "head"
                    || fields[1].name != "tail"
                    || !named(&fields[0].ty, head_type)
                    || !named(&fields[1].ty, "NodeRef")
                    || !record_kind(nil, registry, budget)?.is_empty()
                {
                    return Err(PackageError::KindShape);
                }
            }
        }
    }
    for (i, form) in package.forms.iter().enumerate() {
        *subject = Some(PackageSubject::Form(i as u64));
        package.category(&form.category)?;
        if form.spelling.is_empty() || form.kind.schema != package.schema {
            return Err(PackageError::KindShape);
        }
        for prior in &package.forms[..i] {
            budget.charge(
                Resource::Work,
                (form.spelling.len() + form.category.len()) as u64 + 1,
            )?;
            if prior.category == form.category && prior.spelling == form.spelling {
                return Err(PackageError::DuplicateName);
            }
        }
        unique(form.fields.iter().map(|v| v.name.as_str()), budget)?;
        let fields = record_kind(&form.kind, registry, budget)?;
        if fields.len() != form.fields.len() {
            return Err(PackageError::KindShape);
        }
        for (index, (field, expected)) in form.fields.iter().zip(fields).enumerate() {
            *subject = Some(PackageSubject::FormField {
                form: i as u64,
                field: index as u64,
            });
            let name = if matches!(
                terminal(package, field.read, budget)?,
                ReadSpec::Foreign { .. }
            ) {
                "ForeignSyntax"
            } else {
                "NodeRef"
            };
            if field.name != expected.name || !named(&expected.ty, name) {
                return Err(PackageError::KindShape);
            }
        }
    }
    for (i, leaf) in package.leaves.iter().enumerate() {
        *subject = Some(PackageSubject::Leaf(i as u64));
        package.category(&leaf.category)?;
        if leaf.kind.schema != package.schema || leaf.token_kind.schema != package.schema {
            return Err(PackageError::KindShape);
        }
        for prior in &package.leaves[..i] {
            budget.charge(Resource::Work, 1)?;
            if prior.category == leaf.category && prior.token_kind == leaf.token_kind {
                return Err(PackageError::DuplicateName);
            }
        }
        registry.validate_type(&leaf.payload, budget)?;
        if !record_kind(&leaf.kind, registry, budget)?.is_empty() {
            return Err(PackageError::KindShape);
        }
        let fields = record_kind(&leaf.token_kind, registry, budget)?;
        if fields.len() != 1 || fields[0].name != "payload" || fields[0].ty != leaf.payload {
            return Err(PackageError::KindShape);
        }
    }
    for (mode_index, mode) in package.modes.iter().enumerate() {
        for (index, take) in mode.take.iter().enumerate() {
            *subject = Some(PackageSubject::ModeTake {
                mode: mode_index as u64,
                rule: index as u64,
            });
            let expected = match &take.reader {
                TokenReader::Builtin(reader) => builtin_type(*reader),
                TokenReader::Rule(name) => package
                    .reader
                    .rule(name)?
                    .output
                    .clone_with_budget(budget)?,
            };
            let fields = record_kind(&take.kind, registry, budget)?;
            if fields.len() != 1 || fields[0].name != "payload" || fields[0].ty != expected {
                return Err(PackageError::KindShape);
            }
        }
    }
    Ok(())
}
