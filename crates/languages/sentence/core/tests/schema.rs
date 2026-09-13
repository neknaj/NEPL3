use nepl3_core::{
    budget::{Budget, Limits},
    schema::{SchemaError, SchemaRegistry, TypeDescriptor},
    value::{NdfValue, Record, SchemaRef, Variant},
};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 1000,
        output_bytes: 10_000_000,
        ..Limits::default()
    })
}

fn registry() -> Result<(SchemaRegistry, SchemaRef), SchemaError> {
    let mut registry = SchemaRegistry::default();
    let foundation = nepl3_core::schema::foundation::descriptor(&mut budget())?;
    registry.register(
        foundation.reference(&mut budget())?,
        foundation,
        &mut budget(),
    )?;
    let sentence = nepl3_sentence_core::schema::descriptor(&mut budget())?;
    let identity = sentence.reference(&mut budget())?;
    registry.register(identity.clone(), sentence, &mut budget())?;
    registry.finalize(&mut budget())?;
    Ok((registry, identity))
}

#[test]
fn sentence_wire_uses_its_own_identity_and_ordered_typed_fields() -> Result<(), SchemaError> {
    let (registry, identity) = registry()?;
    assert_eq!(identity.package, "nepl3.sentence");
    // Independent wire fixture: root, nodes, embeds, with typed root reference.
    let value = NdfValue::Record(Record {
        schema: identity.clone(),
        kind: "SentenceValue".into(),
        fields: vec![
            NdfValue::Variant(Variant {
                schema: identity.clone(),
                type_name: "SentenceRoot".into(),
                variant: "Sentence".into(),
                fields: vec![NdfValue::Record(Record {
                    schema: identity.clone(),
                    kind: "SentenceRef".into(),
                    fields: vec![NdfValue::U64(0)],
                })],
            }),
            NdfValue::List(vec![NdfValue::Variant(Variant {
                schema: identity,
                type_name: "SentenceKind".into(),
                variant: "Sentence".into(),
                fields: vec![NdfValue::List(vec![])],
            })]),
            NdfValue::List(vec![]),
        ],
    });
    registry.validate(&TypeDescriptor::TypedValue, &value, &mut budget())?;
    let NdfValue::Record(record) = &value else {
        return Err(SchemaError::WrongType);
    };
    let mut reordered = record.clone();
    reordered.fields.swap(0, 1);
    assert!(
        registry
            .validate(
                &TypeDescriptor::TypedValue,
                &NdfValue::Record(reordered),
                &mut budget()
            )
            .is_err()
    );
    let mut wrong_owner = record.clone();
    wrong_owner.schema.package = "nepl3.doc".into();
    assert!(
        registry
            .validate(
                &TypeDescriptor::TypedValue,
                &NdfValue::Record(wrong_owner),
                &mut budget()
            )
            .is_err()
    );
    let mut wrong_revision = record.clone();
    wrong_revision.schema.revision += 1;
    assert!(
        registry
            .validate(
                &TypeDescriptor::TypedValue,
                &NdfValue::Record(wrong_revision),
                &mut budget()
            )
            .is_err()
    );
    Ok(())
}

#[test]
fn descriptor_requires_foundation_closure_and_does_not_claim_operations() -> Result<(), SchemaError>
{
    let sentence = nepl3_sentence_core::schema::descriptor(&mut budget())?;
    assert!(sentence.operations.is_empty());
    let mut registry = SchemaRegistry::default();
    registry.register(sentence.reference(&mut budget())?, sentence, &mut budget())?;
    assert!(registry.finalize(&mut budget()).is_err());
    Ok(())
}
