use super::*;
pub(in crate::portable) fn equal<E>(
    a: &NdfValue,
    c: &NdfValue,
    b: &mut Budget,
) -> Result<bool, PortableError<E>> {
    Ok(a.equal_with_budget(c, b)?)
}
