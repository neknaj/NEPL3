//! Reader-owned code membership, not a universal diagnostic catalog validator.
use super::*;
use nepl3_core::schema::TypeShape;

pub(super) fn code(
    diagnostic: &Diagnostic,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    budget.charge(
        Resource::Work,
        (diagnostic.schema.package.len() as u64)
            .saturating_add(schema::PACKAGE.len() as u64)
            .saturating_add(9),
    )?;
    if diagnostic.schema.package != schema::PACKAGE
        || diagnostic.schema.revision != schema::REVISION
    {
        return Ok(());
    }
    let (selected, descriptor) = registry
        .selected_descriptor_with_budget(schema::PACKAGE, schema::REVISION, budget)?
        .ok_or(ReaderError::ProviderContract)?;
    budget.charge(
        Resource::Work,
        (selected.package.len() as u64)
            .saturating_add(diagnostic.schema.package.len() as u64)
            .saturating_add(41),
    )?;
    if selected != &diagnostic.schema {
        return Err(ReaderError::ProviderContract);
    }
    for ty in &descriptor.types {
        budget.charge(Resource::Work, (ty.name.len() as u64).saturating_add(21))?;
        if ty.name == "ReaderDiagnosticCode" {
            let TypeShape::Variant { variants } = &ty.shape else {
                return Err(ReaderError::ProviderContract);
            };
            for variant in variants {
                budget.charge(
                    Resource::Work,
                    (variant.name.len() as u64)
                        .saturating_add(diagnostic.code.len() as u64)
                        .saturating_add(1),
                )?;
                if variant.name == diagnostic.code {
                    return Ok(());
                }
            }
            return Err(ReaderError::ProviderContract);
        }
    }
    Err(ReaderError::ProviderContract)
}

pub(super) fn report(
    report: &Report,
    registry: &SchemaRegistry,
    budget: &mut Budget,
) -> Result<(), ReaderError> {
    for diagnostic in &report.diagnostics {
        budget.charge(Resource::Work, 1)?;
        code(diagnostic, registry, budget)?;
    }
    Ok(())
}
