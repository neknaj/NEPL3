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
fn index_sort_preserves_partial_runs_and_duplicate_order() -> Result<(), WireError> {
    for count in [0_usize, 1, 3, 7, 129, 255, 257] {
        let mut index: Vec<_> = (0..count)
            .map(|request| ((count - request) % 11, request))
            .collect();
        let mut expected = index.clone();
        expected.sort_by_key(|entry| entry.0);
        sort_index(&mut index, &mut budget())?;
        assert_eq!(index, expected);
    }
    Ok(())
}

#[test]
fn index_growth_and_layout_independent_resource_boundaries() -> Result<(), WireError> {
    let mut previous = None;
    for count in [128_usize, 256, 512] {
        let mut reference_usage = None;
        for layout in 0..4 {
            let original: Vec<_> = (0..count)
                .map(|request| {
                    let address = match layout {
                        0 => request,
                        1 => count - request - 1,
                        2 => (request % 2) * (count / 2) + request / 2,
                        _ => request % 7,
                    };
                    (address, request)
                })
                .collect();
            let mut expected = original.clone();
            // Independent test-only stable sort preserves duplicate requests.
            expected.sort_by_key(|entry| entry.0);
            let mut index = original.clone();
            let mut measured = budget();
            sort_index(&mut index, &mut measured)?;
            assert_eq!(index, expected);
            let used = measured.usage();
            if let Some(reference) = reference_usage {
                assert_eq!(used, reference);
            } else {
                reference_usage = Some(used);
                if let Some(previous_work) = previous {
                    // Doubling n in n log n stays below 3x; insertion sorting
                    // incurs approximately 4x for the same request counts.
                    assert!(used.work < previous_work * 3);
                }
                previous = Some(used.work);
            }
            for reason in [StopReason::WorkLimit, StopReason::AllocationLimit] {
                for below in [false, true] {
                    let mut limits = budget().limits();
                    match reason {
                        StopReason::WorkLimit => limits.work = used.work - u64::from(below),
                        StopReason::AllocationLimit => {
                            limits.allocation_units = used.allocation_units - u64::from(below);
                        }
                        _ => unreachable!("enumerated resource"),
                    }
                    let mut bounded = Budget::new(limits);
                    let mut index = original.clone();
                    let result = sort_index(&mut index, &mut bounded);
                    if below {
                        assert_eq!(result, Err(WireError::Stopped(reason)));
                        assert_eq!(bounded.poll(), Err(reason));
                    } else {
                        result?;
                        assert_eq!(index, expected);
                    }
                }
            }
        }
    }
    Ok(())
}

#[test]
fn unrequested_nodes_keep_layout_independent_lookup_cost() -> Result<(), WireError> {
    let root = NdfValue::List((0..32).map(|_| NdfValue::Unit).collect());
    let NdfValue::List(children) = &root else {
        return Err(WireError::InvalidType);
    };
    let inputs = |start| {
        let mut inputs = vec![CanonicalDigestInput {
            domain: b"root",
            value: &root,
        }];
        for index in [start, start + 1, start + 2, start] {
            inputs.push(CanonicalDigestInput {
                domain: b"child",
                value: &children[index],
            });
        }
        inputs
    };
    // Unrequested children lie on both sides of the selected address range.
    // Equal children and one duplicate request preserve expected hash values.
    let first = inputs(0);
    let last = inputs(29);
    let mut a = budget();
    let mut b = budget();
    assert_eq!(digests(&first, &mut a)?, digests(&last, &mut b)?);
    assert_eq!(a.usage(), b.usage());
    for below in [false, true] {
        let mut limits = budget().limits();
        limits.work = a.usage().work - u64::from(below);
        let mut left = Budget::new(limits);
        let mut right = Budget::new(limits);
        let result = digests(&first, &mut left);
        assert_eq!(result, digests(&last, &mut right));
        assert_eq!(left.usage(), right.usage());
        if below {
            assert_eq!(result, Err(WireError::Stopped(StopReason::WorkLimit)));
        } else {
            result?;
        }
    }
    Ok(())
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
fn enclosing_request_encodes_large_child_once_with_bounded_storage() -> Result<(), WireError> {
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

#[test]
fn buffered_hashes_preserve_scope_and_stop_at_chunk_boundaries() -> Result<(), WireError> {
    let mut allocation = None;
    for length in [0, CHUNK - 1, CHUNK, CHUNK + 1, 3 * CHUNK + 7] {
        let root = NdfValue::List(vec![
            NdfValue::Bytes(vec![0x41; length]),
            NdfValue::List(vec![NdfValue::Text("selected".into())]),
            NdfValue::Bytes(vec![0x5a; length]),
        ]);
        let NdfValue::List(children) = &root else {
            return Err(WireError::InvalidType);
        };
        let inputs = [
            CanonicalDigestInput {
                domain: b"root",
                value: &root,
            },
            CanonicalDigestInput {
                domain: b"middle",
                value: &children[1],
            },
            CanonicalDigestInput {
                domain: b"last",
                value: &children[2],
            },
            CanonicalDigestInput {
                domain: b"duplicate",
                value: &children[1],
            },
        ];
        // Independent single-value hashing includes exactly each selected
        // subtree and its domain, irrespective of the batch chunk boundaries.
        let expected = inputs
            .iter()
            .map(|input| super::super::digest(input.domain, input.value, &mut budget()))
            .collect::<Result<Vec<_>, _>>()?;
        let mut measured = budget();
        assert_eq!(digests(&inputs, &mut measured)?, expected);
        let usage = measured.usage();
        if let Some(previous) = allocation {
            assert_eq!(usage.allocation_units, previous);
        }
        allocation = Some(usage.allocation_units);
        for work_limit in [true, false] {
            for short in [false, true] {
                let mut limits = budget().limits();
                let reason = if work_limit {
                    limits.work = usage.work - u64::from(short);
                    StopReason::WorkLimit
                } else {
                    limits.allocation_units = usage.allocation_units - u64::from(short);
                    StopReason::AllocationLimit
                };
                let result = digests(&inputs, &mut Budget::new(limits));
                if short {
                    assert_eq!(result, Err(WireError::Stopped(reason)));
                } else {
                    assert_eq!(result?, expected);
                }
            }
        }
    }
    Ok(())
}
