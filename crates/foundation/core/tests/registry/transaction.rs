use super::budget;
use nepl3_core::{budget::*, schema::*, source::Digest, value::*};

fn descriptor(package: &str, field: TypeDescriptor) -> SchemaDescriptor {
    SchemaDescriptor {
        package: package.into(),
        revision: 1,
        types: vec![NamedType {
            name: "Item".into(),
            constraints: vec![],
            shape: TypeShape::Record {
                fields: vec![FieldDescriptor {
                    name: "value".into(),
                    ty: field,
                }],
            },
        }],
        operations: vec![],
    }
}
fn baseline() -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let mut r = SchemaRegistry::default();
    let d = descriptor("base", TypeDescriptor::Unit);
    let identity = d.reference(&mut budget())?;
    r.register(identity.clone(), d, &mut budget())?;
    r.finalize(&mut budget())?;
    Ok((r, identity))
}
fn check_base(r: &SchemaRegistry, identity: &SchemaRef) -> Result<(), SchemaError> {
    assert!(r.is_finalized());
    assert_eq!(r.selected("base", 1), Some(identity));
    // Exercise structural validation, not merely the finalized flag/lookup.
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "base".into(),
            revision: 1,
            name: "Item".into(),
        }),
        &NdfValue::Record(Record {
            schema: identity.clone(),
            kind: "Item".into(),
            fields: vec![NdfValue::Unit],
        }),
        &mut budget(),
    )
    .map(|_| ())
}
#[test]
fn successful_extension_is_finalized_and_repeated_identity_is_idempotent() -> Result<(), SchemaError>
{
    let (mut r, original) = baseline()?;
    let d = descriptor(
        "extension",
        TypeDescriptor::Named(TypeRef {
            package: "base".into(),
            revision: 1,
            name: "Item".into(),
        }),
    );
    let identity = d.reference(&mut budget())?;
    r.register_finalized(identity.clone(), d.clone(), &mut budget())?;
    r.register_finalized(identity.clone(), d, &mut budget())?;
    assert_eq!(r.selected("extension", 1), Some(&identity));
    check_base(&r, &original)
}
#[test]
fn missing_schema_or_type_rolls_back_before_diagnostic_context_is_reused() -> Result<(), SchemaError>
{
    for (package, name, expected) in [
        ("missing", "Item", SchemaError::UnknownSchema),
        ("base", "Absent", SchemaError::UnknownType),
    ] {
        let (mut r, original) = baseline()?;
        let d = descriptor(
            "extension",
            TypeDescriptor::Named(TypeRef {
                package: package.into(),
                revision: 1,
                name: name.into(),
            }),
        );
        let identity = d.reference(&mut budget())?;
        let mut b = budget();
        assert_eq!(r.register_finalized(identity, d, &mut b), Err(expected));
        assert!(b.usage().work > 0);
        assert!(b.usage().allocation_units > 0);
        assert!(r.selected("extension", 1).is_none());
        check_base(&r, &original)?;
        // A corrected descriptor with the same owner/revision is not a conflict.
        let corrected = descriptor("extension", TypeDescriptor::Unit);
        r.register_finalized(corrected.reference(&mut budget())?, corrected, &mut b)?;
        check_base(&r, &original)?;
    }
    Ok(())
}
#[test]
fn identity_conflict_and_unfinalized_input_preserve_state() -> Result<(), SchemaError> {
    let (mut r, original) = baseline()?;
    let d = descriptor("base", TypeDescriptor::Bool);
    assert_eq!(
        r.register_finalized(d.reference(&mut budget())?, d, &mut budget()),
        Err(SchemaError::ConflictingSchema)
    );
    let d = descriptor("extension", TypeDescriptor::Unit);
    let mut wrong = d.reference(&mut budget())?;
    wrong.digest = Digest([0; 32]);
    assert_eq!(
        r.register_finalized(wrong, d.clone(), &mut budget()),
        Err(SchemaError::IdentityMismatch)
    );
    check_base(&r, &original)?;
    let mut empty = SchemaRegistry::default();
    assert_eq!(
        empty.register_finalized(d.reference(&mut budget())?, d.clone(), &mut budget()),
        Err(SchemaError::Unfinalized)
    );
    assert!(!empty.is_finalized());
    assert!(empty.selected("extension", 1).is_none());
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        empty.register_finalized(d.reference(&mut budget())?, d, &mut cancelled),
        Err(SchemaError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
#[test]
fn exact_and_one_short_limits_restore_state_without_refunding_or_clearing_stop()
-> Result<(), SchemaError> {
    let d = descriptor("extension", TypeDescriptor::Unit);
    let identity = d.reference(&mut budget())?;
    let (mut measure, _) = baseline()?;
    let mut measured = budget();
    measured.charge(Resource::Work, 17)?;
    measure.register_finalized(identity.clone(), d.clone(), &mut measured)?;
    let cost = measured.usage();
    for (resource, amount, stop) in [
        (Resource::Work, cost.work, StopReason::WorkLimit),
        (
            Resource::AllocationUnits,
            cost.allocation_units,
            StopReason::AllocationLimit,
        ),
    ] {
        for short in [false, true] {
            let (mut r, original) = baseline()?;
            let mut limits = budget().limits();
            let bound = amount - u64::from(short);
            match resource {
                Resource::Work => limits.work = bound,
                _ => limits.allocation_units = bound,
            }
            let mut b = Budget::new(limits);
            b.charge(Resource::Work, 17)?;
            let result = r.register_finalized(identity.clone(), d.clone(), &mut b);
            if short {
                assert_eq!(result, Err(SchemaError::Stopped(stop)));
                assert_eq!(b.poll(), Err(stop));
                assert!(b.usage().work >= 17);
                assert!(r.selected("extension", 1).is_none());
            } else {
                result?;
                assert_eq!(r.selected("extension", 1), Some(&identity));
            }
            check_base(&r, &original)?;
        }
    }
    Ok(())
}
#[test]
fn finalization_stop_after_registration_restores_a_valid_prefix() -> Result<(), SchemaError> {
    let d = descriptor("extension", TypeDescriptor::Unit);
    let identity = d.reference(&mut budget())?;
    // Independently measure registration only: this allows the append but not
    // the first finalization allocation, proving rollback after mutation.
    let (mut ordinary, _) = baseline()?;
    let mut cost = budget();
    ordinary.register(identity.clone(), d.clone(), &mut cost)?;
    let (mut r, original) = baseline()?;
    let mut stopped = Budget::new(Limits {
        allocation_units: cost.usage().allocation_units,
        ..budget().limits()
    });
    assert_eq!(
        r.register_finalized(identity, d, &mut stopped),
        Err(SchemaError::Stopped(StopReason::AllocationLimit))
    );
    assert_eq!(
        stopped.usage().allocation_units,
        cost.usage().allocation_units
    );
    assert_eq!(stopped.poll(), Err(StopReason::AllocationLimit));
    assert!(r.selected("extension", 1).is_none());
    check_base(&r, &original)
}

#[test]
fn existing_identity_failure_and_populated_unfinalized_registry_keep_their_proofs()
-> Result<(), SchemaError> {
    let (mut r, original) = baseline()?;
    let d = descriptor("base", TypeDescriptor::Unit);
    let mut registration = budget();
    r.register(original.clone(), d.clone(), &mut registration)?;
    let mut limited = Budget::new(Limits {
        allocation_units: registration.usage().allocation_units,
        ..budget().limits()
    });
    assert_eq!(
        r.register_finalized(original.clone(), d, &mut limited),
        Err(SchemaError::Stopped(StopReason::AllocationLimit))
    );
    check_base(&r, &original)?;
    let extension = descriptor("extension", TypeDescriptor::Unit);
    let identity = extension.reference(&mut budget())?;
    r.register_finalized(identity.clone(), extension.clone(), &mut budget())?;
    r.validate(
        &TypeDescriptor::Named(TypeRef {
            package: "extension".into(),
            revision: 1,
            name: "Item".into(),
        }),
        &NdfValue::Record(Record {
            schema: identity.clone(),
            kind: "Item".into(),
            fields: vec![NdfValue::Unit],
        }),
        &mut budget(),
    )?;
    let mut unfinalized = SchemaRegistry::default();
    unfinalized.register(identity.clone(), extension, &mut budget())?;
    let another = descriptor("another", TypeDescriptor::Unit);
    assert_eq!(
        unfinalized.register_finalized(another.reference(&mut budget())?, another, &mut budget()),
        Err(SchemaError::Unfinalized)
    );
    assert!(!unfinalized.is_finalized());
    assert_eq!(unfinalized.selected("extension", 1), Some(&identity));
    assert!(unfinalized.selected("another", 1).is_none());
    Ok(())
}
