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

fn enter_fixture(
    selected: &[usize],
    query: usize,
    work: u64,
    completed: bool,
) -> (Result<bool, WireError>, nepl3_core::budget::Usage) {
    let values: [NdfValue; 7] = core::array::from_fn(|_| NdfValue::Unit);
    let inputs: Vec<_> = selected
        .iter()
        .map(|&i| CanonicalDigestInput {
            domain: b"x",
            value: &values[i],
        })
        .collect();
    let mut index: Vec<_> = inputs
        .iter()
        .enumerate()
        .map(|(i, input)| (core::ptr::from_ref(input.value).addr(), i))
        .collect();
    // Lower-bound insertion places later duplicate requests first.
    index.sort_unstable_by_key(|&(address, request)| (address, usize::MAX - request));
    let mut batch = Batch {
        inputs: &inputs,
        index,
        states: (0..inputs.len())
            .map(|_| State {
                hash: None,
                digest: completed.then_some(Digest([0; 32])),
            })
            .collect(),
        active: Vec::with_capacity(inputs.len()),
        scopes: Vec::with_capacity(inputs.len()),
    };
    let mut limits = budget().limits();
    limits.work = work;
    let mut b = Budget::new(limits);
    let result = batch.enter(&values[query], &mut b);
    if result.is_err() {
        let used = b.usage();
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        assert_eq!(
            batch.enter(&values[query], &mut b),
            Err(WireError::Stopped(StopReason::WorkLimit))
        );
        assert_eq!(b.usage(), used);
    }
    (result, b.usage())
}

#[test]
fn absent_lookup_terminal_probe_is_independent_of_address_rank() {
    // Below, between and above all selected addresses, including end-of-index.
    // n=3: lower-bound charge=2, exactly one terminal probe=1.
    for work in 0..=3 {
        let reference = enter_fixture(&[1, 3, 5], 0, work, false);
        for query in [2, 4, 6] {
            assert_eq!(enter_fixture(&[1, 3, 5], query, work, false), reference);
        }
        if work == 3 {
            assert_eq!(reference.0, Ok(false));
        } else {
            assert_eq!(reference.0, Err(WireError::Stopped(StopReason::WorkLimit)));
        }
    }
}

#[test]
fn duplicate_runs_and_completed_matches_charge_the_terminal_probe() {
    let layouts: [(&[usize], usize); 3] =
        [(&[1, 1, 3, 5], 1), (&[1, 3, 3, 5], 3), (&[1, 3, 5, 5], 5)];
    for completed in [false, true] {
        // n=4 lower bound=3; two run entries + terminal=3; live
        // states also pay two domain bytes and one scope push, total9.
        let exact = if completed { 6 } else { 9 };
        for work in 0..=exact {
            let reference = enter_fixture(layouts[0].0, layouts[0].1, work, completed);
            for &(selected, query) in &layouts[1..] {
                assert_eq!(enter_fixture(selected, query, work, completed), reference);
            }
            if work == exact {
                assert_eq!(reference.0, Ok(!completed));
            } else {
                assert_eq!(reference.0, Err(WireError::Stopped(StopReason::WorkLimit)));
            }
        }
    }
    // A batch consisting entirely of equal references still advances each
    // already-completed match instead of restarting its hash or looping.
    assert_eq!(enter_fixture(&[3, 3, 3, 3], 3, 8, true).0, Ok(false));
    assert_eq!(enter_fixture(&[3, 3, 3, 3], 3, 13, false).0, Ok(true));
}
