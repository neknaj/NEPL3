use super::*;
use alloc::boxed::Box;
use nepl3_core::{
    origin::Origin,
    schema::{FieldDescriptor, TypeDescriptor},
    source::Digest,
    syntax::{Environment, EnvironmentEntry, EnvironmentRef, ForeignSyntax},
    value::{NdfScalar, SchemaRef},
};
fn node(schema: &SchemaRef, kind: &str, fields: Vec<FieldValue>) -> SyntaxNode {
    SyntaxNode {
        schema: schema.clone(),
        kind: kind.into(),
        fields,
        head: None,
        cover: None,
        origin: OriginId(0),
        token: None,
    }
}
fn bundle(node: SyntaxNode) -> SyntaxBundle {
    SyntaxBundle {
        sources: vec![],
        nodes: vec![node],
        origins: vec![Origin::Synthetic {
            reason: "test".into(),
            anchor: None,
        }],
        root: NodeRef(0),
        environments: vec![],
        tokens: vec![],
        source_maps: vec![],
    }
}
fn fixture(padding: usize) -> Result<(SchemaRegistry, SyntaxBundle), String> {
    let mut r = SchemaRegistry::default();
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: alloc::format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        r.register(
            d.reference(&mut budget())
                .map_err(|e| alloc::format!("{e:?}"))?,
            d,
            &mut budget(),
        )
        .map_err(|e| alloc::format!("{e:?}"))?;
    }
    let d = SchemaDescriptor {
        package: "target".into(),
        revision: 1,
        types: vec![
            NamedType {
                name: "Leaf".into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            },
            NamedType {
                name: "Root".into(),
                shape: TypeShape::Record {
                    fields: ["unused", "guest"]
                        .into_iter()
                        .map(|name| FieldDescriptor {
                            name: name.into(),
                            ty: TypeDescriptor::Unit,
                        })
                        .collect(),
                },
                constraints: vec![],
            },
            NamedType {
                name: "Variant".into(),
                shape: TypeShape::Variant { variants: vec![] },
                constraints: vec![],
            },
        ],
        operations: vec![],
    };
    let schema = d
        .reference(&mut budget())
        .map_err(|e| alloc::format!("{e:?}"))?;
    r.register(schema.clone(), d, &mut budget())
        .map_err(|e| alloc::format!("{e:?}"))?;
    let guest = bundle(node(&schema, "Leaf", vec![]));
    let foreign = ForeignSyntax {
        schema: schema.clone(),
        category: "Leaf".into(),
        root: NodeRef(0),
        bundle: guest,
        environment: EnvironmentRef {
            id: 0,
            digest: Digest([0; 32]),
        },
    };
    let mut root = bundle(node(
        &schema,
        "Root",
        vec![
            FieldValue::Atom(NdfScalar::Unit),
            FieldValue::Foreign(Box::new(foreign)),
        ],
    ));
    root.environments.push(EnvironmentEntry {
        id: 0,
        digest: Digest([0; 32]),
        value: Environment {
            bindings: vec![],
            resources: vec![],
        },
    });
    Ok((r, root))
}
fn step() -> ForeignStep {
    ForeignStep {
        node: NodeRef(0),
        field: "guest".into(),
    }
}
#[test]
fn foreign_path_lookup_charges_only_work_and_depth_and_borrows_the_guest() -> Result<(), String> {
    for padding in [0, 32] {
        let (r, root) = fixture(padding)?;
        let FieldValue::Foreign(guest) = &root.nodes[0].fields[1] else {
            return Err("guest".into());
        };
        let work = 1
            + 1
            + padding as u64 * (10 + 6 + 9)
            + (6 + 6 + 9)
            + (6 + 41)
            + 3 * (4 + 1)
            + 2 * (5 + 1);
        let mut limits = budget().limits();
        limits.work = work;
        limits.nodes = 0;
        limits.allocation_units = 0;
        let mut b = Budget::new(limits);
        let result = path(&root, &[step()], &r, &mut b).map_err(|e| alloc::format!("{e:?}"))?;
        assert!(core::ptr::eq(result, &guest.bundle));
        assert_eq!(
            b.usage(),
            nepl3_core::budget::Usage {
                work,
                depth: 1,
                ..Default::default()
            }
        );
        for cap in [1, work - 1] {
            limits.work = cap;
            let mut stopped = Budget::new(limits);
            assert!(matches!(
                path(&root, &[step()], &r, &mut stopped),
                Err(TreeError::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        }
    }
    Ok(())
}
#[test]
fn foreign_path_keeps_semantic_errors_and_step_ordering() -> Result<(), String> {
    let (r, root) = fixture(0)?;
    for case in 0..8 {
        let mut changed = root.clone();
        let mut step = step();
        match case {
            0 => changed.nodes[0].schema.package = "missing".into(),
            1 => changed.nodes[0].schema.revision += 1,
            2 => changed.nodes[0].schema.digest.0[0] ^= 1,
            3 => changed.nodes[0].kind = "missing".into(),
            4 => changed.nodes[0].kind = "Variant".into(),
            5 => step.field = "missing".into(),
            6 => changed.nodes[0].fields.clear(),
            _ => changed.nodes[0].fields[1] = FieldValue::Atom(NdfScalar::Unit),
        }
        assert!(matches!(
            path(&changed, &[step], &r, &mut budget()),
            Err(TreeError::Path)
        ));
    }
    let invalid = ForeignStep {
        node: NodeRef(99),
        field: "guest".into(),
    };
    let mut b = budget();
    assert!(matches!(
        path(&root, core::slice::from_ref(&invalid), &r, &mut b),
        Err(TreeError::Path)
    ));
    assert_eq!(
        b.usage(),
        nepl3_core::budget::Usage {
            work: 1,
            depth: 1,
            ..Default::default()
        }
    );
    let mut limits = budget().limits();
    limits.depth = 0;
    assert!(matches!(
        path(
            &root,
            core::slice::from_ref(&invalid),
            &r,
            &mut Budget::new(limits)
        ),
        Err(TreeError::Stopped(StopReason::DepthLimit))
    ));
    limits.work = 0;
    assert!(matches!(
        path(&root, &[invalid], &r, &mut Budget::new(limits)),
        Err(TreeError::Stopped(StopReason::WorkLimit))
    ));
    let mut cancelled = budget();
    cancelled.cancel();
    assert!(matches!(
        path(&root, &[step()], &r, &mut cancelled),
        Err(TreeError::Stopped(StopReason::Cancelled))
    ));
    assert!(core::ptr::eq(
        path(&root, &[], &r, &mut cancelled).map_err(|e| alloc::format!("{e:?}"))?,
        &root
    ));
    assert_eq!(cancelled.usage(), nepl3_core::budget::Usage::default());
    Ok(())
}
