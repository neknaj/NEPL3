use super::*;
use crate::package::{EntryContext, PackageIdentity};
use alloc::{format, vec};
use nepl3_core::{
    budget::Limits,
    schema::{NamedType, SchemaDescriptor},
    source::{Digest, SourceId, SourceRef},
    syntax::EnvironmentRef,
    value::{KindRef, OperationRef},
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
fn registry(padding: usize) -> Result<(SchemaRegistry, SchemaRef), HeadError> {
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
    let d = SchemaDescriptor {
        package: "fixture.head".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Node".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        }],
        operations: vec![],
    };
    let schema = d.reference(&mut budget())?;
    r.register(schema.clone(), d, &mut budget())?;
    r.finalize(&mut budget())?;
    Ok((r, schema))
}
fn call(schema: &SchemaRef) -> HeadCall {
    let span = ProjectedSpan {
        source: SourceRef {
            source_id: SourceId("head".into()),
            revision: 0,
            digest: Digest::of(b"x"),
        },
        start: 0,
        end: 1,
    };
    HeadCall {
        identity: HeadCallIdentity {
            session_id: "head".into(),
            call_id: 0,
            operation: OperationRef {
                schema: schema.clone(),
                name: "shape".into(),
            },
            profile_digest: Digest::of(b"profile"),
            execution_digest: Digest::of(b"execution"),
        },
        depth_base: 0,
        entry: EntryContext {
            package: PackageIdentity {
                schema: schema.clone(),
                semantic_digest: Digest::of(b"package"),
            },
            alias: "A".into(),
            category: "Node".into(),
            mode: "Code".into(),
        },
        environment: EnvironmentRef {
            id: 0,
            digest: Digest::of(b"environment"),
        },
        head: ProjectedHead {
            token: ProjectedToken {
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
                head: span.clone(),
                payload: NdfValue::Unit,
            },
            window: SourceWindow {
                span,
                bytes: vec![b'x'],
            },
        },
        request: HeadRequest::Shape,
    }
}

#[test]
fn projection_schema_searches_charge_registry_padding() -> Result<(), HeadError> {
    // Isolate each receiving check: no earlier profile lookup can accidentally
    // account for the same registry traversal or make an old raw scan fail.
    for token in [false, true] {
        let mut usages = vec![];
        for padding in [0, 16] {
            let (r, schema) = registry(padding)?;
            let call = call(&schema);
            let node = ProjectedNode {
                schema,
                kind: "Node".into(),
                head: None,
                cover: None,
                token: None,
                fields: vec![],
            };
            let syntax = ProjectedSyntax {
                nodes: vec![],
                roots: vec![],
                windows: vec![],
            };
            let check = |b: &mut Budget| {
                if token {
                    call.check_token(&call.head.token, &r, b)
                } else {
                    call.check_node(&node, &syntax, &r, b)
                }
            };
            let mut b = budget();
            b.charge(Resource::Work, 7)?;
            check(&mut b)?;
            usages.push(b.usage());
            if padding != 0 {
                let mut limited = Budget::new(Limits {
                    work: usages[0].work,
                    ..budget().limits()
                });
                limited.charge(Resource::Work, 7)?;
                assert_eq!(
                    check(&mut limited),
                    Err(HeadError::Stopped(StopReason::WorkLimit))
                );
                assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert_eq!(
                check(&mut cancelled),
                Err(HeadError::Stopped(StopReason::Cancelled))
            );
        }
        let expected = 16 * (("padding.00.".len() + 32) + "fixture.head".len() + 9) as u64;
        assert_eq!(usages[1].work - usages[0].work, expected);
        let mut equal = usages[0];
        equal.work = usages[1].work;
        assert_eq!(equal, usages[1]);
    }
    Ok(())
}

#[test]
fn projection_schema_search_preserves_identity_errors() -> Result<(), HeadError> {
    let (r, mut schema) = registry(16)?;
    schema.digest = Digest::of(b"forged");
    let call = call(&schema);
    assert_eq!(
        call.check_token(&call.head.token, &r, &mut budget()),
        Err(HeadError::Schema(SchemaError::UnknownSchema))
    );
    let node = ProjectedNode {
        schema,
        kind: "Node".into(),
        head: None,
        cover: None,
        token: None,
        fields: vec![],
    };
    let syntax = ProjectedSyntax {
        nodes: vec![],
        roots: vec![],
        windows: vec![],
    };
    assert_eq!(
        call.check_node(&node, &syntax, &r, &mut budget()),
        Err(HeadError::Schema(SchemaError::UnknownSchema))
    );
    Ok(())
}
