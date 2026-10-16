use super::*;
use alloc::{boxed::Box, vec, vec::Vec};
use nepl3_core::{
    budget::{Limits, StopReason},
    schema::{TypeRef, foundation},
    source::SourceAdmission,
    syntax::group::ForeignClosureGroup,
    value::{NdfValue, Record},
};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    })
}
fn setup(change: Option<(&str, TypeDescriptor)>) -> Result<(SchemaRef, SchemaRegistry), WireError> {
    let mut b = budget();
    let mut d = foundation::descriptor(&mut b)?;
    if let Some((name, replacement)) = change {
        let n = d
            .types
            .iter_mut()
            .find(|n| n.name == name)
            .ok_or(SchemaError::UnknownType)?;
        let TypeShape::Record { fields } = &mut n.shape else {
            return Err(WireError::InvalidType);
        };
        let f = fields
            .iter_mut()
            .find(|f| f.name == "sources")
            .ok_or(SchemaError::WrongType)?;
        f.ty = replacement;
    }
    registry(d)
}
fn registry(
    d: nepl3_core::schema::SchemaDescriptor,
) -> Result<(SchemaRef, SchemaRegistry), WireError> {
    let mut b = budget();
    let s = d.reference(&mut b)?;
    let mut r = SchemaRegistry::default();
    r.register(s.clone(), d, &mut b)?;
    r.finalize(&mut b)?;
    Ok((s, r))
}
#[test]
fn empty_list_does_not_prove_source_element_contract() -> Result<(), WireError> {
    for name in ["SyntaxBundle", "ForeignProvenance"] {
        let (s, r) = setup(Some((
            name,
            TypeDescriptor::List(Box::new(TypeDescriptor::Text)),
        )))?;
        // Independent reproduction: an empty list satisfies List(Text).
        r.validate(
            &TypeDescriptor::List(Box::new(TypeDescriptor::Text)),
            &NdfValue::List(vec![]),
            &mut budget(),
        )?;
        assert_eq!(
            check(&s, &r, true, &mut budget()),
            Err(SchemaError::WrongType.into())
        );
        if name == "ForeignProvenance" {
            // SyntaxArchive does not replace provenance fields.
            check(&s, &r, false, &mut budget())?;
        } else {
            assert_eq!(
                check(&s, &r, false, &mut budget()),
                Err(SchemaError::WrongType.into())
            );
        }
    }
    Ok(())
}
#[test]
fn reject_wrong_shape_and_named_element() -> Result<(), WireError> {
    for ty in [
        TypeDescriptor::Text,
        TypeDescriptor::List(Box::new(TypeDescriptor::Named(TypeRef {
            package: "nepl3.foundation".into(),
            revision: 1,
            name: "SourceRef".into(),
        }))),
    ] {
        let (s, r) = setup(Some(("SyntaxBundle", ty)))?;
        assert_eq!(
            check(&s, &r, true, &mut budget()),
            Err(SchemaError::WrongType.into())
        );
    }
    Ok(())
}
#[test]
fn current_identity_finalization_and_sticky_work_are_required() -> Result<(), WireError> {
    let (s, r) = setup(None)?;
    let mut b = budget();
    check(&s, &r, true, &mut b)?;
    let work = b.usage().work;
    assert!(work > 0);
    assert_eq!(b.usage().allocation_units, 0);
    let mut exact = Budget::new(Limits {
        work,
        ..budget().limits()
    });
    check(&s, &r, true, &mut exact)?;
    assert_eq!(exact.usage(), b.usage());
    let mut short = Budget::new(Limits {
        work: work - 1,
        ..budget().limits()
    });
    assert_eq!(
        check(&s, &r, true, &mut short),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    let retained = short.usage();
    assert_eq!(
        check(&s, &r, true, &mut short),
        Err(WireError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(short.usage(), retained);
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        check(&s, &r, false, &mut cancelled),
        Err(WireError::Stopped(StopReason::Cancelled))
    );
    for (package, revision) in [("other.foundation", 1), ("nepl3.foundation", 2)] {
        let mut wrong = s.clone();
        wrong.package = package.into();
        wrong.revision = revision;
        assert_eq!(
            check(&wrong, &r, true, &mut budget()),
            Err(SchemaError::IdentityMismatch.into())
        );
    }
    let mut wrong = s.clone();
    wrong.digest.0[0] ^= 1;
    assert_eq!(
        check(&wrong, &r, true, &mut budget()),
        Err(SchemaError::UnknownSchema.into())
    );
    assert_eq!(
        check(&s, &SchemaRegistry::default(), false, &mut budget()),
        Err(SchemaError::Unfinalized.into())
    );
    Ok(())
}
#[test]
fn group_public_boundaries_reject_even_empty_mutated_contract() -> Result<(), WireError> {
    let (s, r) = setup(Some((
        "ForeignProvenance",
        TypeDescriptor::List(Box::new(TypeDescriptor::Text)),
    )))?;
    let group = ForeignClosureGroup {
        provenances: Vec::new(),
        environments: Vec::new(),
        entries: Vec::new(),
    };
    // An empty expanded v1 root is structurally valid under this descriptor.
    let value = NdfValue::Record(Record {
        schema: s.clone(),
        kind: "ForeignClosureGroup".into(),
        fields: vec![
            NdfValue::List(vec![]),
            NdfValue::List(vec![]),
            NdfValue::List(vec![]),
        ],
    });
    r.validate(
        &crate::source::expected("ForeignClosureGroup"),
        &value,
        &mut budget(),
    )?;
    // Archives deliberately require both typed replacement contracts up front,
    // even if the group has no instantiated entries. The guard precedes bytes.
    assert_eq!(
        crate::syntax::group::archive::encode(
            &group,
            &s,
            &s,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(SchemaError::WrongType.into())
    );
    assert_eq!(
        crate::syntax::group::archive::decode(
            &[],
            &s,
            &s,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(SchemaError::WrongType.into())
    );
    let (s, r) = setup(Some((
        "SyntaxBundle",
        TypeDescriptor::List(Box::new(TypeDescriptor::Text)),
    )))?;
    assert_eq!(
        crate::syntax::archive::decode(
            &[],
            &s,
            &s,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(SchemaError::WrongType.into())
    );
    Ok(())
}

#[test]
fn replaced_field_name_position_and_record_shape_are_checked() -> Result<(), WireError> {
    for mutation in 0..3 {
        let mut d = foundation::descriptor(&mut budget())?;
        let n = d
            .types
            .iter_mut()
            .find(|n| n.name == "SyntaxBundle")
            .ok_or(SchemaError::UnknownType)?;
        let TypeShape::Record { fields } = &mut n.shape else {
            return Err(WireError::InvalidType);
        };
        match mutation {
            0 => fields[0].name = "notSources".into(),
            1 => fields.swap(0, 1),
            _ => {
                n.shape = TypeShape::Variant {
                    variants: vec![nepl3_core::schema::VariantDescriptor {
                        name: "Empty".into(),
                        fields: vec![],
                    }],
                }
            }
        }
        let (s, r) = registry(d)?;
        assert_eq!(
            check(&s, &r, false, &mut budget()),
            Err(SchemaError::WrongType.into())
        );
    }
    Ok(())
}
#[test]
fn syntax_encode_checks_descriptor_before_invalid_native_input() -> Result<(), WireError> {
    let (s, r) = setup(Some((
        "SyntaxBundle",
        TypeDescriptor::List(Box::new(TypeDescriptor::Text)),
    )))?;
    let bundle = nepl3_core::syntax::SyntaxBundle {
        sources: vec![],
        nodes: vec![],
        origins: vec![],
        root: nepl3_core::syntax::NodeRef(0),
        environments: vec![],
        tokens: vec![],
        source_maps: vec![],
    };
    assert_eq!(
        crate::syntax::archive::encode(
            &bundle,
            &s,
            &s,
            &s,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        ),
        Err(SchemaError::WrongType.into())
    );
    Ok(())
}
