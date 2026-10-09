use super::*;
use alloc::{string::String, vec};
use nepl3_core::{
    budget::Limits,
    origin::OriginId,
    schema::{NamedType, SchemaDescriptor},
    syntax::SyntaxNode,
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        nodes: 1000,
        depth: 100,
        ..Limits::default()
    })
}
fn fixture() -> Result<(SchemaRegistry, SyntaxNode, KindRef), String> {
    let d = SchemaDescriptor {
        package: "p".into(),
        revision: 1,
        types: vec![NamedType {
            name: "Node".into(),
            shape: TypeShape::Record { fields: vec![] },
            constraints: vec![],
        }],
        operations: vec![],
    };
    let schema = d
        .reference(&mut budget())
        .map_err(|e| alloc::format!("{e:?}"))?;
    let mut r = SchemaRegistry::default();
    r.register(schema.clone(), d, &mut budget())
        .map_err(|e| alloc::format!("{e:?}"))?;
    let node = SyntaxNode {
        schema: schema.clone(),
        kind: "Node".into(),
        fields: vec![],
        head: None,
        cover: None,
        origin: OriginId(0),
        token: None,
    };
    Ok((
        r,
        node,
        KindRef {
            schema,
            local_kind: 0,
        },
    ))
}
#[test]
fn same_kind_retains_semantic_mismatches_and_schema_short_circuit() -> Result<(), String> {
    let (r, node, kind) = fixture()?;
    assert_eq!(same_kind(&node, &kind, &r, &mut budget()), Ok(true));
    for case in 0..4 {
        let mut node = node.clone();
        let mut kind = kind.clone();
        match case {
            0 => node.kind = "Other".into(),
            1 => kind.local_kind = u64::MAX,
            2 => {
                node.schema.package = "missing".into();
                kind.schema = node.schema.clone();
            }
            _ => {
                node.schema.digest.0[0] ^= 1;
                kind.schema = node.schema.clone();
            }
        }
        assert_eq!(same_kind(&node, &kind, &r, &mut budget()), Ok(false));
    }
    let mut different = kind;
    different.schema.digest.0[0] ^= 1;
    let mut b = budget();
    b.cancel();
    assert_eq!(same_kind(&node, &different, &r, &mut b), Ok(false));
    assert_eq!(b.usage(), nepl3_core::budget::Usage::default());
    assert_eq!(b.poll(), Err(StopReason::Cancelled));
    Ok(())
}
#[test]
fn same_kind_does_not_swallow_lookup_stops() -> Result<(), String> {
    let (r, node, kind) = fixture()?;
    let mut limits = budget().limits();
    limits.work = 0;
    let mut b = Budget::new(limits);
    assert_eq!(
        same_kind(&node, &kind, &r, &mut b),
        Err(TreeError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    let mut b = budget();
    b.cancel();
    assert_eq!(
        same_kind(&node, &kind, &r, &mut b),
        Err(TreeError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[path = "tests/path.rs"]
mod path_tests;
