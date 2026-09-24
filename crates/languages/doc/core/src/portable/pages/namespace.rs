//! Portable plans are compared with a freshly resolved native namespace proof.
//! Guest selection and lowering remain explicit responsibilities of the caller.
use super::*;
use crate::pages::namespace::{CheckedPageNamespaces, PageNamespacePlan};

fn sequence<T: Value, C: FoundationValueCodec>(
    values: &[T],
    s: &nepl3_core::value::SchemaRef,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut out = Vec::new();
    for value in values {
        let value = value.put(s, c, b)?;
        pages::push(&mut out, value, b)?;
    }
    Ok(NdfValue::List(out))
}

/// Encode member ownership, semantic destinations and unresolved requirements
/// from this exact proof. This operation performs no guest discovery or I/O.
pub fn to_value<C: FoundationValueCodec>(
    proof: &CheckedPageNamespaces<'_, '_, '_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.poll()?;
    let s = schema(r)?;
    let mut members = Vec::new();
    for member in proof.members() {
        let owner = member.owner();
        let value = record(
            s,
            "PageMemberPlan",
            [
                owner.page.put(s, c, b)?,
                owner.member.0.put(s, c, b)?,
                member.document_digest().put(s, c, b)?,
                sequence(member.links(), s, c, b)?,
                sequence(member.remaining(), s, c, b)?,
            ],
            b,
        )?;
        pages::push(&mut members, value, b)?;
    }
    let value = record(
        s,
        "PageNamespacePlan",
        [proof.identity().put(s, c, b)?, NdfValue::List(members)],
        b,
    )?;
    text::check_type(&value, "PageNamespacePlan", r, b)?;
    Ok(value)
}

/// Recompute expected data from the receiver's proof and compare every field.
/// The caller first decodes documents, selects guests and resolves namespaces.
/// Successful receipt returns data; the native proof is still required by HTML.
pub fn from_value<C: FoundationValueCodec>(
    input: &NdfValue,
    proof: &CheckedPageNamespaces<'_, '_, '_>,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<PageNamespacePlan, PortableError<C::Error>> {
    text::check_type(input, "PageNamespacePlan", r, b)?;
    let expected = to_value(proof, r, c, b)?;
    if !input.equal_with_budget(&expected, b)? {
        return Err(PortableError::Shape);
    }
    PageNamespacePlan::read(input, schema(r)?, c, b)
}
