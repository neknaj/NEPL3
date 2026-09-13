//! Bounded deterministic input exploration, not a substitute for sustained fuzzing.
use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_wire::{WireError, decode, encode};

fn limits() -> Limits {
    Limits {
        source_bytes: 4096,
        work: 4096,
        depth: 24,
        nodes: 128,
        allocation_units: 8192,
        output_bytes: 4096,
        ..Limits::default()
    }
}

fn examine(input: &[u8], limits: Limits) -> bool {
    let mut first = Budget::new(limits);
    let mut second = Budget::new(limits);
    let result = decode(input, &mut first);
    assert_eq!(result, decode(input, &mut second), "input {input:02x?}");
    assert_eq!(first.usage(), second.usage());
    assert_eq!(first.current_depth(), 0);
    match result {
        Ok(value) => {
            // NDF admits canonical encodings only; this is a byte identity
            // requirement, not an expected value copied from the decoder.
            assert_eq!(
                encode(&value, &mut Budget::new(self::limits())),
                Ok(input.to_vec())
            );
            true
        }
        Err(WireError::Stopped(reason)) => {
            assert_eq!(first.poll(), Err(reason));
            assert_eq!(
                decode(&[0x81, 0x00], &mut first),
                Err(WireError::Stopped(reason))
            );
            false
        }
        Err(_) => false,
    }
}

#[test]
fn every_zero_one_and_two_byte_input() {
    let mut accepted = usize::from(examine(&[], limits()));
    for a in 0..=u8::MAX {
        accepted += usize::from(examine(&[a], limits()));
        for b in 0..=u8::MAX {
            accepted += usize::from(examine(&[a, b], limits()));
        }
    }
    // The only complete NDF encodings with <=2 bytes are [Unit] and [None].
    // Other tags require payloads in addition to the CBOR array and tag.
    assert_eq!(accepted, 2);
}

#[test]
fn mutated_spec_vectors_and_bounded_arbitrary_bytes() {
    // Handwritten from the NDF/1 tag table and CBOR, independent of encode().
    let seeds: &[&[u8]] = &[
        &[0x81, 0],
        &[0x82, 1, 0xf5],
        &[0x82, 2, 0x18, 24],
        &[0x83, 3, 0xf5, 0x41, 1],
        &[0x83, 4, 0x83, 3, 0xf4, 0x41, 1, 0x41, 2],
        &[0x82, 5, 0x63, 0xe6, 0x97, 0xa5],
        &[0x82, 6, 0x42, 0, 255],
        &[0x82, 7, 0x82, 0x81, 0, 0x81, 8],
        &[0x81, 8],
        &[0x82, 9, 0x81, 0],
    ];
    for seed in seeds {
        assert!(examine(seed, limits()));
        for end in 0..seed.len() {
            assert!(!examine(&seed[..end], limits()));
        }
        for index in 0..seed.len() {
            for bit in 0..8 {
                let mut altered = seed.to_vec();
                altered[index] ^= 1 << bit;
                examine(&altered, limits());
            }
        }
        for tail in [0, 0xff] {
            let mut extended = seed.to_vec();
            extended.push(tail);
            assert!(!examine(&extended, limits()));
        }
    }
    // Fixed xorshift sequence makes failures reproducible on native and WASI.
    let mut state = 0x6e64_6621_u32;
    for length in 0..96 {
        for _ in 0..32 {
            let mut bytes = vec![0; length];
            for byte in &mut bytes {
                state ^= state << 13;
                state ^= state >> 17;
                state ^= state << 5;
                *byte = state as u8;
            }
            examine(&bytes, limits());
        }
    }
}

#[test]
fn nested_options_exhaust_each_logical_budget() {
    let mut bytes = [0x82, 9].repeat(32);
    bytes.extend_from_slice(&[0x81, 0]);
    let mut sufficient = limits();
    sufficient.depth = 64;
    sufficient.allocation_units = 1_000_000;
    // Positive control: the exact same input is valid when no cap intervenes.
    let valid = decode(&bytes, &mut Budget::new(sufficient));
    assert!(valid.is_ok());
    if let Ok(value) = valid {
        assert_eq!(
            encode(&value, &mut Budget::new(sufficient)),
            Ok(bytes.clone())
        );
    }
    for cap in 0..32 {
        for resource in 0..4 {
            let mut limits = limits();
            limits.depth = 64;
            limits.allocation_units = 1_000_000;
            let reason = match resource {
                0 => {
                    limits.depth = cap;
                    StopReason::DepthLimit
                }
                1 => {
                    limits.nodes = cap;
                    StopReason::NodeLimit
                }
                2 => {
                    limits.work = cap;
                    StopReason::WorkLimit
                }
                _ => {
                    limits.allocation_units = cap;
                    StopReason::AllocationLimit
                }
            };
            assert_eq!(
                decode(&bytes, &mut Budget::new(limits)),
                Err(WireError::Stopped(reason))
            );
            assert!(!examine(&bytes, limits));
        }
    }
}
