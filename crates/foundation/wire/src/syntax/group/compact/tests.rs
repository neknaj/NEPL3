use super::*;
use nepl3_core::budget::Limits;
#[test]
fn component_membership_boundaries_are_not_flattened() -> Result<(), WireError> {
    let mut b = Budget::new(Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    });
    let a = NdfValue::Bytes(alloc::vec![1;32]);
    let z = NdfValue::Bytes(alloc::vec![2;32]);
    let left = NdfValue::List(alloc::vec![
        NdfValue::List(alloc::vec![a.clone()]),
        NdfValue::List(alloc::vec![z.clone()])
    ]);
    let right = NdfValue::List(alloc::vec![
        NdfValue::List(alloc::vec![]),
        NdfValue::List(alloc::vec![a, z])
    ]);
    // Deliberately the same flattened Bytes32 sequence, different local boundaries.
    // Unit is an isolated framing fixture, not a claim of native syntax validity.
    assert_ne!(
        component(SYNTAX, &NdfValue::Unit, left, &mut b)?,
        component(SYNTAX, &NdfValue::Unit, right, &mut b)?
    );
    Ok(())
}
