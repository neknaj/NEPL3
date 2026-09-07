//! A returned plan is re-derived from the explicit request document. Matching
//! IDs alone, or a sender's digest assertion, cannot supply missing requirements.
use super::*;
use crate::prepare::{self, DocPreparationPlan, DocRequirement, PreparationError};
fn matches(
    actual: &DocPreparationPlan,
    received: &DocPreparationPlan,
    b: &mut Budget,
) -> Result<bool, StopReason> {
    b.charge(Resource::Work, 33)?;
    if actual.document_digest != received.document_digest
        || actual.requirements.len() != received.requirements.len()
    {
        return Ok(false);
    }
    for (a, r) in actual.requirements.iter().zip(&received.requirements) {
        let work = match (a, r) {
            (DocRequirement::Link { target: a, .. }, DocRequirement::Link { target: r, .. }) => {
                let length = |t: &LinkTarget| match t {
                    LinkTarget::Page { page, fragment } => page
                        .len()
                        .saturating_add(fragment.as_ref().map_or(0, |f| f.len())),
                    LinkTarget::Relative { path, fragment } => path
                        .len()
                        .saturating_add(fragment.as_ref().map_or(0, |f| f.len())),
                    LinkTarget::External { uri } => uri.len(),
                };
                (length(a) as u64).saturating_add(length(r) as u64)
            }
            (DocRequirement::Asset { asset: a, .. }, DocRequirement::Asset { asset: r, .. }) => {
                (a.id.len() as u64)
                    .saturating_add(r.id.len() as u64)
                    .saturating_add(64)
            }
            _ => 64,
        };
        b.charge(Resource::Work, work.saturating_add(1))?;
        if a != r {
            return Ok(false);
        }
    }
    Ok(true)
}
use crate::model::LinkTarget;
pub fn plan_to_value<'a, C: FoundationValueCodec>(
    plan: &DocPreparationPlan,
    document: &'a DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PreparationError<'a, C::Error>> {
    let actual = prepare::inspect(document, r, c, b)?;
    if !matches(&actual, plan, b)? {
        return Err(PreparationError::Boundary(PortableError::Shape));
    }
    let value = plan.put(schema(r)?, c, b)?;
    super::text::check_type(&value, "DocPreparationPlan", r, b)?;
    Ok(value)
}
pub fn plan_from_value<'a, C: FoundationValueCodec>(
    value: &NdfValue,
    document: &'a DocumentSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<DocPreparationPlan, PreparationError<'a, C::Error>> {
    super::text::check_type(value, "DocPreparationPlan", r, b)?;
    let plan = DocPreparationPlan::read(value, schema(r)?, c, b)?;
    let actual = prepare::inspect(document, r, c, b)?;
    if !matches(&actual, &plan, b)? {
        return Err(PreparationError::Boundary(PortableError::Shape));
    }
    Ok(plan)
}
