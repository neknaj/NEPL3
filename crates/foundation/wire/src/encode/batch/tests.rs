use super::*;
use alloc::vec;
use nepl3_core::budget::Limits;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        output_bytes: 10_000,
        nodes: 10_000,
        depth: 100,
        ..Limits::default()
    })
}

#[test]
fn allocation_order_preserves_usage_and_work_boundary() -> Result<(), WireError> {
    // All values are equal but distinct. Reversing references changes pointer
    // insertion/lookup positions while preserving the logical input sequence.
    let values = [
        NdfValue::Unit,
        NdfValue::Unit,
        NdfValue::Unit,
        NdfValue::Unit,
    ];
    let forward: Vec<_> = values
        .iter()
        .map(|value| CanonicalDigestInput {
            domain: b"layout",
            value,
        })
        .collect();
    let reverse: Vec<_> = values
        .iter()
        .rev()
        .map(|value| CanonicalDigestInput {
            domain: b"layout",
            value,
        })
        .collect();
    let mut first = budget();
    let mut second = budget();
    assert_eq!(
        digests(&forward, &mut first)?,
        digests(&reverse, &mut second)?
    );
    assert_eq!(first.usage(), second.usage());
    for work in [first.usage().work - 1, first.usage().work] {
        let mut limits = budget().limits();
        limits.work = work;
        let mut a = Budget::new(limits);
        let mut b = Budget::new(limits);
        assert_eq!(digests(&forward, &mut a), digests(&reverse, &mut b));
        assert_eq!(a.usage(), b.usage());
    }
    Ok(())
}

#[test]
fn nested_hashes_exclude_parent_headers_and_siblings() -> Result<(), WireError> {
    let root = NdfValue::List(vec![NdfValue::Unit, NdfValue::Bool(true)]);
    let NdfValue::List(children) = &root else {
        return Err(WireError::InvalidType);
    };
    let inputs = [
        CanonicalDigestInput {
            domain: b"root",
            value: &root,
        },
        CanonicalDigestInput {
            domain: b"child",
            value: &children[0],
        },
        CanonicalDigestInput {
            domain: b"last",
            value: &children[1],
        },
        CanonicalDigestInput {
            domain: b"another",
            value: &children[0],
        },
    ];
    // NDF List is [7, elements]; Unit is [0]; Bool is [1, true].
    // These bytes follow the wire contract independently of the encoder.
    let expected = vec![
        Digest::domain(b"root", &[0x82, 7, 0x82, 0x81, 0, 0x82, 1, 0xf5]),
        Digest::domain(b"child", &[0x81, 0]),
        Digest::domain(b"last", &[0x82, 1, 0xf5]),
        Digest::domain(b"another", &[0x81, 0]),
    ];
    let mut b = budget();
    assert_eq!(digests(&inputs, &mut b)?, expected);
    assert_eq!(b.usage().nodes, 3);
    assert_eq!(b.usage().output_bytes, 128);
    Ok(())
}

#[test]
fn child_first_disjoint_and_equal_values_keep_request_order() -> Result<(), WireError> {
    let root = NdfValue::List(vec![NdfValue::Unit]);
    let NdfValue::List(children) = &root else {
        return Err(WireError::InvalidType);
    };
    let separate = NdfValue::Unit;
    let inputs = [
        CanonicalDigestInput {
            domain: b"a",
            value: &children[0],
        },
        CanonicalDigestInput {
            domain: b"b",
            value: &root,
        },
        CanonicalDigestInput {
            domain: b"a",
            value: &separate,
        },
        CanonicalDigestInput {
            domain: b"a",
            value: &children[0],
        },
    ];
    let expected = inputs
        .iter()
        .map(|input| super::super::digest(input.domain, input.value, &mut budget()))
        .collect::<Result<Vec<_>, _>>()?;
    assert_eq!(digests(&inputs, &mut budget())?, expected);
    assert_eq!(digests(&[], &mut budget())?, vec![]);
    Ok(())
}

#[test]
fn intermediate_container_keeps_unrequested_siblings_in_its_hash() -> Result<(), WireError> {
    use alloc::boxed::Box;
    let root = NdfValue::Some(Box::new(NdfValue::List(vec![
        NdfValue::Bool(false),
        NdfValue::Unit,
        NdfValue::None,
    ])));
    let NdfValue::Some(middle) = &root else {
        return Err(WireError::InvalidType);
    };
    let NdfValue::List(children) = middle.as_ref() else {
        return Err(WireError::InvalidType);
    };
    let inputs = [
        CanonicalDigestInput {
            domain: b"root",
            value: &root,
        },
        CanonicalDigestInput {
            domain: b"middle",
            value: middle,
        },
        CanonicalDigestInput {
            domain: b"leaf",
            value: &children[1],
        },
    ];
    let expected = vec![
        Digest::domain(
            b"root",
            &[0x82, 9, 0x82, 7, 0x83, 0x82, 1, 0xf4, 0x81, 0, 0x81, 8],
        ),
        Digest::domain(b"middle", &[0x82, 7, 0x83, 0x82, 1, 0xf4, 0x81, 0, 0x81, 8]),
        Digest::domain(b"leaf", &[0x81, 0]),
    ];
    assert_eq!(digests(&inputs, &mut budget())?, expected);
    Ok(())
}

#[test]
fn resource_boundaries_and_empty_cancel_are_sticky() -> Result<(), WireError> {
    let root = NdfValue::List(vec![NdfValue::Unit]);
    let inputs = [CanonicalDigestInput {
        domain: b"test",
        value: &root,
    }];
    let mut measured = budget();
    let expected = digests(&inputs, &mut measured)?;
    let used = measured.usage();
    for (amount, reason) in [
        (used.work, StopReason::WorkLimit),
        (used.allocation_units, StopReason::AllocationLimit),
        (used.output_bytes, StopReason::OutputLimit),
        (used.nodes, StopReason::NodeLimit),
        (used.depth, StopReason::DepthLimit),
    ] {
        for below in [false, true] {
            let mut limits = budget().limits();
            let limit = amount - u64::from(below);
            match reason {
                StopReason::WorkLimit => limits.work = limit,
                StopReason::AllocationLimit => limits.allocation_units = limit,
                StopReason::OutputLimit => limits.output_bytes = limit,
                StopReason::NodeLimit => limits.nodes = limit,
                StopReason::DepthLimit => limits.depth = limit,
                _ => unreachable!("enumerated resource"),
            }
            let mut b = Budget::new(limits);
            let result = digests(&inputs, &mut b);
            if below {
                assert_eq!(result, Err(WireError::Stopped(reason)));
                assert_eq!(b.poll(), Err(reason));
                assert_eq!(digests(&[], &mut b), Err(WireError::Stopped(reason)));
            } else {
                assert_eq!(result?, expected);
            }
        }
    }
    let mut b = budget();
    b.stop(StopReason::Cancelled);
    assert_eq!(
        digests(&[], &mut b),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn enclosing_request_encodes_large_child_once_without_a_byte_buffer() -> Result<(), WireError> {
    let root = NdfValue::List(vec![NdfValue::Text("x".repeat(100_000))]);
    let NdfValue::List(children) = &root else {
        return Err(WireError::InvalidType);
    };
    let inputs = [
        CanonicalDigestInput {
            domain: b"root",
            value: &root,
        },
        CanonicalDigestInput {
            domain: b"child",
            value: &children[0],
        },
    ];
    let mut independent = budget();
    let expected = inputs
        .iter()
        .map(|input| super::super::digest(input.domain, input.value, &mut independent))
        .collect::<Result<Vec<_>, _>>()?;
    let mut limits = budget().limits();
    limits.allocation_units = 8192;
    limits.output_bytes = 64;
    let mut batch = Budget::new(limits);
    assert_eq!(digests(&inputs, &mut batch)?, expected);
    assert!(batch.usage().work + 90_000 < independent.usage().work);
    assert_eq!(batch.usage().nodes, 2);
    Ok(())
}
