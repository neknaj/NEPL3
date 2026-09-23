//! Host declarations join schema assembly before kind IDs are assigned.
use super::*;

/// One explicit prefix form with a foreign field. The host owns its semantics
/// and supplies the selected foreign alias through the ParseProfile.
pub struct ForeignForm<'a> {
    pub kind: &'a str,
    pub category: &'a str,
    pub spelling: &'a str,
    pub field: &'a str,
    pub alias: &'a str,
    pub guest_category: &'a str,
    pub origin_reason: &'a str,
}

pub(super) fn declare(
    descriptor: &mut SchemaDescriptor,
    forms: &[ForeignForm<'_>],
    b: &mut Budget,
) -> Result<(), CompileError> {
    for form in forms {
        b.charge(Resource::Work, 7)?;
        if [
            form.kind,
            form.category,
            form.spelling,
            form.field,
            form.alias,
            form.guest_category,
            form.origin_reason,
        ]
        .iter()
        .any(|s| s.is_empty())
        {
            return Err(CompileError::OutputType);
        }
        let name = name("Form:", form.kind, b)?;
        if lookup(&descriptor.types, &name, |t| t.name.as_str(), b)?.is_some() {
            return Err(CompileError::OutputType);
        }
        let mut fields = Vec::new();
        push(
            &mut fields,
            FieldDescriptor {
                name: text(form.field, b)?,
                ty: TypeDescriptor::Named(TypeRef {
                    package: text("nepl3.foundation", b)?,
                    revision: 1,
                    name: text("ForeignSyntax", b)?,
                }),
            },
            b,
        )?;
        push(
            &mut descriptor.types,
            NamedType {
                name,
                shape: TypeShape::Record { fields },
                constraints: Vec::new(),
            },
            b,
        )?;
    }
    Ok(())
}

pub(super) fn append(
    package: &mut p::LanguagePackage,
    forms: &[ForeignForm<'_>],
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<(), CompileError> {
    for form in forms {
        let read = p::ReadSpecId(package.reads.len() as u64);
        push(
            &mut package.reads,
            p::ReadSpec::Foreign {
                alias: text(form.alias, b)?,
                category: text(form.guest_category, b)?,
            },
            b,
        )?;
        let binding = p::BindingId(package.bindings.len() as u64);
        push(
            &mut package.bindings,
            p::Binding::Visit(text(form.field, b)?),
            b,
        )?;
        let mut fields = Vec::new();
        push(
            &mut fields,
            p::FieldSpec {
                name: text(form.field, b)?,
                read,
            },
            b,
        )?;
        push(
            &mut package.forms,
            p::Form {
                category: text(form.category, b)?,
                kind: kind(registry, &package.schema, &name("Form:", form.kind, b)?, b)?,
                spelling: text(form.spelling, b)?,
                fields,
                binding,
                styles: Vec::new(),
                selection_rules: Vec::new(),
            },
            b,
        )?;
        let origin = OriginId(package.provenance.origins.len() as u64);
        push(
            &mut package.provenance.origins,
            Origin::Synthetic {
                reason: text(form.origin_reason, b)?,
                anchor: None,
            },
            b,
        )?;
        push(
            &mut package.provenance.declarations,
            p::DeclarationOrigin {
                kind: p::DeclarationKind::Form,
                name: text(form.kind, b)?,
                category: Some(text(form.category, b)?),
                origin,
            },
            b,
        )?;
    }
    Ok(())
}
