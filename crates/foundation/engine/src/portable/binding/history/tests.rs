use super::*;
use alloc::{format, vec};
use core::convert::Infallible;
use nepl3_core::{
    budget::Limits,
    facts::{FactAuthority, FactReservation, IdRange, Scope, ScopeId},
    schema::{
        NamedType, OperationDescriptor, SchemaDescriptor, TypeDescriptor, TypeRef, TypeShape,
    },
    value::SchemaRef,
};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 10000,
        work: 1000000,
        depth: 128,
        nodes: 10000,
        allocation_units: 1000000,
        output_bytes: 10000,
        diagnostics: 100,
        events: 100,
    })
}
fn registry(padding: usize) -> Result<(SchemaRegistry, SchemaRef), PortableError<Infallible>> {
    let mut r = SchemaRegistry::default();
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}.{}", "x".repeat(32)),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        r.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    let named = |name: &str| {
        TypeDescriptor::Named(TypeRef {
            package: "nepl3.engine".into(),
            revision: 1,
            name: name.into(),
        })
    };
    let d = SchemaDescriptor {
        package: "nepl3.engine".into(),
        revision: 1,
        types: ["FactsReply", "FactsRequest"]
            .into_iter()
            .map(|name| NamedType {
                name: name.into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            })
            .collect(),
        operations: vec![OperationDescriptor {
            name: "facts".into(),
            input: named("FactsRequest"),
            output: named("FactsReply"),
            pure: true,
        }],
    };
    let schema = d.reference(&mut budget())?;
    r.register(schema.clone(), d, &mut budget())?;
    r.finalize(&mut budget())?;
    Ok((r, schema))
}
fn facts() -> FactSet {
    FactSet {
        analysis_id: "history".into(),
        namespaces: vec![],
        scopes: vec![Scope {
            id: ScopeId(0),
            parent: None,
            origin: None,
        }],
        entities: vec![],
        occurrences: vec![],
        relations: vec![],
        edges: vec![],
        sources: vec![],
        origins: vec![],
        source_maps: vec![],
    }
}
fn history(schema: SchemaRef) -> BindingResolutionBatch {
    let empty = IdRange { start: 1, end: 1 };
    BindingResolutionBatch {
        provider: ProviderRequirement {
            provider: "retained".into(),
            revision: 1,
            implementation_digest: Digest::of(b"provider"),
            operation: OperationRef {
                schema,
                name: "facts".into(),
            },
        },
        authority: FactAuthority {
            analysis_id: "history".into(),
            current_scope: ScopeId(0),
            namespaces: vec![],
            writable_scopes: vec![],
            import_scopes: vec![],
            resolution_updates: vec![],
            relation_sources: vec![],
            reservation: FactReservation {
                scopes: empty,
                entities: empty,
                occurrences: empty,
                relations: empty,
            },
        },
        target: CanonicalBindingTarget {
            path: vec![],
            node: NodeRef(0),
        },
        updates: vec![],
    }
}
fn data<'a>(facts: &'a FactSet, history: &'a [BindingResolutionBatch]) -> Data<'a> {
    Data {
        bundles: &[],
        facts: Some(facts),
        sources: &[],
        maps: &[],
        stages: &[],
        occurrences: &[],
        open_inputs: &[],
        exports: &[],
        history,
    }
}

#[test]
fn retained_history_schema_search_charges_registry_padding() -> Result<(), PortableError<Infallible>>
{
    let facts = facts();
    let mut usages = vec![];
    for padding in [0, 16] {
        let (r, schema) = registry(padding)?;
        let history = [history(schema)];
        let data = data(&facts, &history);
        let check =
            |b: &mut Budget| validate::<Infallible>(&data, &r, b, &mut SourceAdmission::default());
        let mut b = budget();
        b.charge(Resource::Work, 7)?;
        check(&mut b)?;
        usages.push(b.usage());
        if padding != 0 {
            // All preceding facts/authority work is unchanged. The old raw
            // provider scan succeeds at this cap, including its next operation
            // table charge; only the missing registry scan differentiates it.
            let mut limited = Budget::new(Limits {
                work: usages[0].work,
                ..budget().limits()
            });
            limited.charge(Resource::Work, 7)?;
            assert!(matches!(
                check(&mut limited),
                Err(PortableError::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
        }
        let mut cancelled = budget();
        cancelled.cancel();
        assert!(matches!(
            check(&mut cancelled),
            Err(PortableError::Stopped(StopReason::Cancelled))
        ));
    }
    let expected = 16 * (("padding.00.".len() + 32) + "nepl3.engine".len() + 9) as u64;
    assert_eq!(usages[1].work - usages[0].work, expected);
    let mut equal = usages[0];
    equal.work = usages[1].work;
    assert_eq!(equal, usages[1]);
    Ok(())
}

#[test]
fn retained_history_schema_search_preserves_unknown_identity_error()
-> Result<(), PortableError<Infallible>> {
    let (r, mut schema) = registry(16)?;
    schema.digest = Digest::of(b"forged");
    let facts = facts();
    let history = [history(schema)];
    assert!(matches!(
        validate::<Infallible>(
            &data(&facts, &history),
            &r,
            &mut budget(),
            &mut SourceAdmission::default()
        ),
        Err(PortableError::Shape)
    ));
    Ok(())
}
