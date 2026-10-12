use super::*;
use crate::schema::{SchemaDescriptor, SchemaError, TypeRef};
use alloc::{boxed::Box, vec, vec::Vec};

fn budget(work: u64) -> Budget {
    Budget::new(crate::budget::Limits {
        work,
        depth: 10_000,
        allocation_units: 10_000_000,
        output_bytes: 10_000_000,
        nodes: 100_000,
        ..Default::default()
    })
}
fn named(package: &str, revision: u64, name: &str) -> TypeDescriptor {
    TypeDescriptor::Named(TypeRef {
        package: package.into(),
        revision,
        name: name.into(),
    })
}
#[test]
fn bounded_type_equality_matches_structural_equality_without_allocation() -> Result<(), StopReason>
{
    let types = vec![
        TypeDescriptor::Unit,
        TypeDescriptor::Bool,
        TypeDescriptor::U64,
        TypeDescriptor::Integer,
        TypeDescriptor::Natural,
        TypeDescriptor::Rational,
        TypeDescriptor::Text,
        TypeDescriptor::Bytes,
        TypeDescriptor::Bytes32,
        TypeDescriptor::NdfValue,
        TypeDescriptor::NdfScalar,
        TypeDescriptor::TypedValue,
        TypeDescriptor::List(Box::new(TypeDescriptor::Text)),
        TypeDescriptor::Option(Box::new(TypeDescriptor::Text)),
        named("package", 1, "名前"),
        named("package", 2, "名前"),
        named("package", 1, "別名"),
        named("other", 1, "名前"),
    ];
    for left in &types {
        for right in &types {
            let mut measured = budget(1000);
            assert_eq!(left.equal_with_budget(right, &mut measured)?, left == right);
            assert_eq!(measured.usage().allocation_units, 0);
            let work = measured.usage().work;
            assert_eq!(
                left.equal_with_budget(right, &mut budget(work))?,
                left == right
            );
            let mut short = budget(work - 1);
            assert_eq!(
                left.equal_with_budget(right, &mut short),
                Err(StopReason::WorkLimit)
            );
            let used = short.usage();
            assert_eq!(
                TypeDescriptor::Unit.equal_with_budget(&TypeDescriptor::Unit, &mut short),
                Err(StopReason::WorkLimit)
            );
            assert_eq!(short.usage(), used);
        }
    }
    Ok(())
}
#[test]
fn deep_type_comparison_is_iterative_and_admits_each_layer() -> Result<(), StopReason> {
    let mut left = TypeDescriptor::Text;
    let mut right = TypeDescriptor::Text;
    for _ in 0..4096 {
        left = TypeDescriptor::List(Box::new(left));
        right = TypeDescriptor::List(Box::new(right));
    }
    let mut exact = budget(4097);
    assert!(left.equal_with_budget(&right, &mut exact)?);
    assert_eq!(exact.usage().work, 4097);
    assert_eq!(exact.usage().depth, 4097);
    assert_eq!(exact.usage().allocation_units, 0);
    let mut short = budget(4096);
    assert_eq!(
        left.equal_with_budget(&right, &mut short),
        Err(StopReason::WorkLimit)
    );
    let mut shallow = Budget::new(crate::budget::Limits {
        depth: 1,
        ..budget(5000).limits()
    });
    assert_eq!(
        left.equal_with_budget(&right, &mut shallow),
        Err(StopReason::DepthLimit)
    );
    assert_eq!(shallow.usage().work, 2);
    let mut cancelled = budget(5000);
    cancelled.stop(StopReason::Cancelled);
    assert_eq!(
        left.equal_with_budget(&right, &mut cancelled),
        Err(StopReason::Cancelled)
    );
    assert_eq!(cancelled.usage().work, 0);
    Ok(())
}
#[test]
fn operation_lookup_uses_sorted_borrowed_names_and_exact_admission() -> Result<(), SchemaError> {
    let descriptor = SchemaDescriptor {
        package: "operations".into(),
        revision: 1,
        types: vec![],
        operations: ["中", "z", "a"]
            .into_iter()
            .map(|name| OperationDescriptor {
                name: name.into(),
                input: TypeDescriptor::Text,
                output: TypeDescriptor::Unit,
                pure: true,
            })
            .collect::<Vec<_>>(),
    };
    let mut setup = budget(100_000);
    let schema = descriptor.reference(&mut setup)?;
    let mut registry = SchemaRegistry::default();
    registry.register(schema.clone(), descriptor, &mut setup)?;
    let mut admission = budget(1000);
    registry.descriptor_with_budget(&schema, &mut admission)?;
    // Sorted names are a,z,中: z takes one 1-byte comparison; m takes two;
    // 中 visits z then its three UTF-8 bytes. Each comparison costs1+3*bytes.
    for (name, found, comparisons) in [("z", true, 4), ("m", false, 8), ("中", true, 14)] {
        let reference = OperationRef {
            schema: schema.clone(),
            name: name.into(),
        };
        let expected = admission.usage().work + comparisons;
        let mut exact = budget(expected);
        let operation = registry.operation_with_budget(&reference, &mut exact)?;
        assert_eq!(operation.is_some(), found);
        if let Some(operation) = operation {
            assert_eq!(operation.name, name);
        }
        assert_eq!(exact.usage().work, expected);
        assert_eq!(exact.usage().allocation_units, 0);
        let mut short = budget(expected - 1);
        assert_eq!(
            registry.operation_with_budget(&reference, &mut short),
            Err(StopReason::WorkLimit)
        );
        let used = short.usage();
        assert_eq!(
            registry.operation_with_budget(&reference, &mut short),
            Err(StopReason::WorkLimit)
        );
        assert_eq!(short.usage(), used);
    }
    let mut forged = OperationRef {
        schema,
        name: "z".into(),
    };
    forged.schema.digest.0[0] ^= 1;
    assert!(
        registry
            .operation_with_budget(&forged, &mut budget(1000))?
            .is_none()
    );
    assert_eq!(
        registry.operation_with_budget(&forged, &mut budget(0)),
        Err(StopReason::WorkLimit)
    );
    Ok(())
}

#[test]
fn long_named_prefixes_and_caller_relative_depth_keep_exact_boundaries() -> Result<(), StopReason> {
    let package = "界".repeat(1024);
    let prefix = "名".repeat(1024);
    let left = named(&package, 1, &alloc::format!("{prefix}a"));
    let right = named(&package, 1, &alloc::format!("{prefix}b"));
    let expected = 1 + (1 + 3 * package.len() as u64) + (1 + 3 * (prefix.len() as u64 + 1));
    let mut exact = budget(expected);
    assert!(!left.equal_with_budget(&right, &mut exact)?);
    assert_eq!(exact.usage().work, expected);
    let mut short = budget(expected - 1);
    assert_eq!(
        left.equal_with_budget(&right, &mut short),
        Err(StopReason::WorkLimit)
    );
    assert_eq!(short.usage().work, expected - 3);
    let mut nested = Budget::new(crate::budget::Limits {
        depth: 2,
        ..budget(10).limits()
    });
    assert!(
        nested.with_depth(|b| TypeDescriptor::Unit.equal_with_budget(&TypeDescriptor::Unit, b))?
    );
    assert_eq!(nested.usage().depth, 2);
    assert_eq!(nested.current_depth(), 0);
    let mut shallow = Budget::new(crate::budget::Limits {
        depth: 1,
        ..budget(10).limits()
    });
    assert_eq!(
        shallow.with_depth(|b| TypeDescriptor::Unit.equal_with_budget(&TypeDescriptor::Unit, b)),
        Err(StopReason::DepthLimit)
    );
    assert_eq!(shallow.current_depth(), 0);
    Ok(())
}
