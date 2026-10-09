use super::*;
use nepl3_core::syntax::{EnvironmentBinding, NamespaceRef};

#[test]
fn checked_context_still_requires_runtime_context_and_namespace_membership() -> Result<(), String> {
    let (mut admitted, schema) = registry().map_err(err)?;
    let owner = SchemaDescriptor {
        package: "owner".into(),
        revision: 1,
        types: vec![],
        operations: vec![],
    };
    let owner_ref = owner.reference(&mut budget()).map_err(err)?;
    admitted
        .register(owner_ref.clone(), owner, &mut budget())
        .map_err(err)?;
    admitted.finalize(&mut budget()).map_err(err)?;
    let (runtime, _) = registry_with_padding(32).map_err(err)?;
    let p = plan(
        &schema,
        vec![ReaderExpr::Literal("a".into())],
        0,
        TypeDescriptor::Unit,
    );
    let checked = p.check(&runtime, &mut budget()).map_err(err)?;
    let input = source("a").map_err(err)?;
    let mut sources = SourceStore::default();
    sources.insert(input.clone()).map_err(err)?;
    for namespace in [false, true] {
        let mut raw = context(&schema, &admitted).map_err(err)?;
        if namespace {
            raw.environment.value.bindings.push(EnvironmentBinding {
                namespace: NamespaceRef {
                    schema: owner_ref.clone(),
                    name: "Value".into(),
                },
                name: "x".into(),
                value: TypedValue::Record(Record {
                    schema: schema.clone(),
                    kind: "Node".into(),
                    fields: vec![],
                }),
                origin: None,
            });
            raw.environment.digest = nepl3_wire::environment::environment_digest(
                &raw.environment.value,
                admitted
                    .selected("nepl3.foundation", 1)
                    .ok_or("foundation")?,
                &admitted,
                &mut budget(),
            )
            .map_err(err)?;
        } else {
            raw.schema = owner_ref.clone();
        }
        let mut admission = SourceAdmission::default();
        let proof =
            check_context(&raw, &sources, &admitted, &mut budget(), &mut admission).map_err(err)?;
        if namespace {
            use nepl3_reader::context::ContextError;
            assert!(matches!(
                proof.retarget_preserving_sources(&schema, "next", "test", &runtime, &mut budget()),
                Err(ContextError::InvalidContext)
            ));
            assert!(matches!(
                proof.retarget(&schema, "next", "test", &sources, &runtime, &mut budget()),
                Err(ContextError::InvalidContext)
            ));
        }
        let mut b = budget();
        let mut session =
            ReaderSession::new("context".into(), &checked, &runtime, &mut b).map_err(err)?;
        let result = session.read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 1,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit,
            },
            &sources,
            &mut b,
            &mut admission,
        );
        assert!(
            matches!(result, Err(error) if error == if namespace { ReaderError::Schema(SchemaError::UnknownSchema) } else { ReaderError::Context })
        );
        assert_eq!(b.poll(), Ok(()));
        assert_eq!(b.usage().source_bytes, 1); // Admission before schema rejection is not refunded.
    }
    Ok(())
}

#[test]
fn runtime_context_lookup_exhaustion_is_a_normal_reader_stop() -> Result<(), String> {
    let (registry, schema) = registry().map_err(err)?;
    let p = plan(
        &schema,
        vec![ReaderExpr::Literal("a".into())],
        0,
        TypeDescriptor::Unit,
    );
    let checked = p.check(&registry, &mut budget()).map_err(err)?;
    let input = source("a").map_err(err)?;
    let mut sources = SourceStore::default();
    sources.insert(input.clone()).map_err(err)?;
    let raw = context(&schema, &registry).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let proof =
        check_context(&raw, &sources, &registry, &mut budget(), &mut admission).map_err(err)?;
    let mut b = budget();
    let mut session =
        ReaderSession::new("context".into(), &checked, &registry, &mut b).map_err(err)?;
    // Reproduce only the unchanged public request prefix with the same ledger:
    // source equality, admission and Unit state validation. The next Work unit
    // enters selected-foundation lookup, before its first comparison.
    admission.admit_existing(&input, &mut b).map_err(err)?;
    let start = b.usage().work;
    assert!(input.eq_with_budget(&input, &mut b).map_err(err)?);
    admission.admit_existing(&input, &mut b).map_err(err)?;
    registry
        .validate(&TypeDescriptor::Unit, &NdfValue::Unit, &mut b)
        .map_err(err)?;
    let prefix = b.usage().work - start;
    // The input was admitted before calibration, so this repeats the same
    // cached admission path without adding SourceBytes.
    let remaining = b.limits().work - b.usage().work;
    b.charge(Resource::Work, remaining - (prefix + 1))
        .map_err(err)?;
    let before = b.usage();
    let reply = session
        .read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 1,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit,
            },
            &sources,
            &mut b,
            &mut admission,
        )
        .map_err(err)?;
    assert!(matches!(
        reply,
        ReadReply::Stopped {
            reason: StopReason::WorkLimit,
            ..
        }
    ));
    assert_eq!(b.usage().work - before.work, prefix + 1);
    assert_eq!(b.usage().allocation_units, before.allocation_units);
    assert_eq!(b.usage().source_bytes, 1);
    Ok(())
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn runtime_request_rejects_a_different_selected_foundation_identity() -> Result<(), String> {
    let (admitted, schema) = registry().map_err(err)?;
    let mut runtime = SchemaRegistry::default();
    for package in ["nepl3.foundation", "nepl3.reader", "test"] {
        let reference = admitted.selected(package, 1).ok_or("schema")?;
        let mut descriptor = admitted.descriptor(reference).ok_or("descriptor")?.clone();
        if package == "nepl3.foundation" {
            descriptor.types.push(NamedType {
                name: "ContextAdmissionExtra".into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            });
        }
        let reference = descriptor.reference(&mut budget()).map_err(err)?;
        runtime
            .register(reference, descriptor, &mut budget())
            .map_err(err)?;
    }
    runtime.finalize(&mut budget()).map_err(err)?;
    assert_ne!(
        runtime.selected("nepl3.foundation", 1),
        admitted.selected("nepl3.foundation", 1)
    );
    let p = plan(
        &schema,
        vec![ReaderExpr::Literal("a".into())],
        0,
        TypeDescriptor::Unit,
    );
    let checked = p.check(&runtime, &mut budget()).map_err(err)?;
    let input = source("a").map_err(err)?;
    let mut sources = SourceStore::default();
    sources.insert(input.clone()).map_err(err)?;
    let raw = context(&schema, &admitted).map_err(err)?;
    let mut admission = SourceAdmission::default();
    let proof =
        check_context(&raw, &sources, &admitted, &mut budget(), &mut admission).map_err(err)?;
    let mut b = budget();
    let mut session =
        ReaderSession::new("context".into(), &checked, &runtime, &mut b).map_err(err)?;
    assert!(matches!(
        session.read(
            "entry",
            ReadRequest {
                snapshot: &input,
                start: 0,
                limit: 1,
                final_input: true,
                context: &proof,
                state: &NdfValue::Unit
            },
            &sources,
            &mut b,
            &mut admission
        ),
        Err(ReaderError::Context)
    ));
    Ok(())
}
