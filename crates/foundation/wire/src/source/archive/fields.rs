//! Empty archive skeleton lists cannot establish their element type. Check the
//! typed replacement sites against the selected descriptor before either codec.
use crate::WireError;
use nepl3_core::{
    budget::{Budget, Resource},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor, TypeShape},
    value::SchemaRef,
};

pub(crate) fn check(
    foundation: &SchemaRef,
    registry: &SchemaRegistry,
    group: bool,
    budget: &mut Budget,
) -> Result<(), WireError> {
    budget.poll()?;
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    budget.charge(Resource::Work, foundation.package.len() as u64 + 1)?;
    if foundation.package != "nepl3.foundation" || foundation.revision != 1 {
        return Err(SchemaError::IdentityMismatch.into());
    }
    let descriptor = registry
        .descriptor_with_budget(foundation, budget)?
        .ok_or(SchemaError::UnknownSchema)?;
    for (name, position) in [("SyntaxBundle", 0), ("ForeignProvenance", 1)] {
        if name == "ForeignProvenance" && !group {
            continue;
        }
        let mut found = None;
        for named in &descriptor.types {
            budget.charge(Resource::Work, named.name.len() as u64 + 1)?;
            if named.name == name {
                found = Some(named);
                break;
            }
        }
        let named = found.ok_or(SchemaError::UnknownType)?;
        let TypeShape::Record { fields } = &named.shape else {
            return Err(SchemaError::WrongType.into());
        };
        let field = fields.get(position).ok_or(SchemaError::FieldCount)?;
        budget.charge(Resource::Work, field.name.len() as u64 + 1)?;
        if field.name != "sources" {
            return Err(SchemaError::WrongType.into());
        }
        let TypeDescriptor::List(element) = &field.ty else {
            return Err(SchemaError::WrongType.into());
        };
        let TypeDescriptor::Named(reference) = element.as_ref() else {
            return Err(SchemaError::WrongType.into());
        };
        budget.charge(
            Resource::Work,
            (reference.package.len() as u64)
                .saturating_add(reference.name.len() as u64)
                .saturating_add(1),
        )?;
        if reference.package != "nepl3.foundation"
            || reference.revision != 1
            || reference.name != "SourceContent"
        {
            return Err(SchemaError::WrongType.into());
        }
    }
    budget.poll()?;
    Ok(())
}

#[cfg(test)]
mod tests;
