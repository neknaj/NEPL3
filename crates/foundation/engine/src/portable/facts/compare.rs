use super::*;
/// A borrowed iterative comparison charges variable payload comparisons before
/// touching them. No full-value clone or recursive derived equality is used.
pub(super) fn equal<E>(
    a: &NdfValue,
    c: &NdfValue,
    b: &mut Budget,
) -> Result<bool, PortableError<E>> {
    let mut pending = Vec::new();
    push(&mut pending, (a, c, 1u64), b)?;
    while let Some((a, c, depth)) = pending.pop() {
        b.observe_depth(depth)?;
        b.charge(Resource::Work, 1)?;
        let children = match (a, c) {
            (NdfValue::Some(a), NdfValue::Some(c)) => Some((
                core::slice::from_ref(a.as_ref()),
                core::slice::from_ref(c.as_ref()),
            )),
            (NdfValue::List(a), NdfValue::List(c)) => Some((a.as_slice(), c.as_slice())),
            (NdfValue::Record(a), NdfValue::Record(c)) => {
                b.charge(
                    Resource::Work,
                    (a.schema.package.len() + c.schema.package.len() + a.kind.len() + c.kind.len())
                        as u64
                        + 41,
                )?;
                if a.schema != c.schema || a.kind != c.kind {
                    return Ok(false);
                }
                Some((a.fields.as_slice(), c.fields.as_slice()))
            }
            (NdfValue::Variant(a), NdfValue::Variant(c)) => {
                b.charge(
                    Resource::Work,
                    (a.schema.package.len()
                        + c.schema.package.len()
                        + a.type_name.len()
                        + c.type_name.len()
                        + a.variant.len()
                        + c.variant.len()) as u64
                        + 41,
                )?;
                if a.schema != c.schema || a.type_name != c.type_name || a.variant != c.variant {
                    return Ok(false);
                }
                Some((a.fields.as_slice(), c.fields.as_slice()))
            }
            _ => {
                let size = |v: &NdfValue| match v {
                    NdfValue::Text(v) => v.len() as u64,
                    NdfValue::Bytes(v) => v.len() as u64,
                    NdfValue::Integer(v) => v.as_bigint().bits().div_ceil(8),
                    NdfValue::Rational(v) => {
                        v.numerator().as_bigint().bits().div_ceil(8)
                            + v.denominator().bits().div_ceil(8)
                    }
                    _ => 1,
                };
                b.charge(Resource::Work, size(a).saturating_add(size(c)))?;
                // At least one side may be composite, but unequal enum tags
                // return without traversing that side's children.
                if a != c {
                    return Ok(false);
                }
                None
            }
        };
        if let Some((a, c)) = children {
            if a.len() != c.len() {
                return Ok(false);
            }
            for (a, c) in a.iter().zip(c) {
                push(&mut pending, (a, c, depth.saturating_add(1)), b)?;
            }
        }
    }
    Ok(true)
}
