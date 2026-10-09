use nepl3_core::{
    budget::*,
    schema::*,
    source::*,
    syntax::{NodeRef, SyntaxError},
    value::{KindRef, NdfValue},
    view::{Token, ViewBundle},
};
use nepl3_engine::parse::ParseArena;
fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        source_bytes: 1000,
        nodes: 1000,
        depth: 100,
        output_bytes: 1000,
        diagnostics: 10,
        events: 10,
    })
}
fn fixture(padding: usize) -> Result<(SchemaRegistry, KindRef, Token), SyntaxError> {
    let mut r = SchemaRegistry::default();
    for i in 0..padding {
        let d = SchemaDescriptor {
            package: format!("padding.{i:02}"),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        r.register(d.reference(&mut budget())?, d, &mut budget())?;
    }
    let d = SchemaDescriptor {
        package: "target".into(),
        revision: 1,
        types: ["Head", "Token"]
            .into_iter()
            .map(|name| NamedType {
                name: name.into(),
                shape: TypeShape::Record { fields: vec![] },
                constraints: vec![],
            })
            .collect(),
        operations: vec![],
    };
    let schema = d.reference(&mut budget())?;
    r.register(schema.clone(), d, &mut budget())?;
    // Head construction historically accepts an unfinalized registry.
    let source = SourceSnapshot::new(
        SourceId("s".into()),
        0,
        "memory:s".into(),
        b"x".to_vec(),
        &mut budget(),
    )?;
    let kind = KindRef {
        schema: schema.clone(),
        local_kind: 0,
    };
    let token = Token {
        kind: KindRef {
            schema,
            local_kind: 1,
        },
        head: source.span(0, 1)?,
        payload: NdfValue::Unit,
        views: ViewBundle {
            elements: vec![],
            roots: vec![],
        },
        leading_trivia: vec![],
    };
    Ok((r, kind, token))
}
#[test]
fn head_kind_lookup_charges_catalog_without_changing_arena_ownership() -> Result<(), SyntaxError> {
    let mut usages = vec![];
    let mut outputs = vec![];
    for padding in [0, 32] {
        let (r, kind, token) = fixture(padding)?;
        let mut arena = ParseArena::default();
        let mut b = budget();
        assert_eq!(arena.head(token.clone(), &kind, &r, &mut b)?, NodeRef(0));
        assert_eq!(arena.nodes[0].kind, "Head");
        assert_eq!(arena.tokens[0], token);
        usages.push(b.usage());
        outputs.push((arena.nodes, arena.tokens, arena.origins));
    }
    let mut expected = usages[0];
    expected.work += 32 * (10 + 6 + 9);
    assert_eq!(usages[1], expected);
    assert_eq!(outputs[0], outputs[1]);
    let (r, kind, token) = fixture(32)?;
    let mut arena = ParseArena::default();
    arena.head(token.clone(), &kind, &r, &mut budget())?;
    let before = (
        arena.nodes.clone(),
        arena.tokens.clone(),
        arena.origins.clone(),
    );
    let mut limits = budget().limits();
    limits.work = usages[0].work;
    let mut stopped = Budget::new(limits);
    assert_eq!(
        arena.head(token, &kind, &r, &mut stopped),
        Err(SyntaxError::Stopped(StopReason::WorkLimit))
    );
    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
    assert_eq!((arena.nodes, arena.tokens, arena.origins), before);
    Ok(())
}
#[test]
fn head_lookup_preserves_identity_errors_and_allocation_priority() -> Result<(), SyntaxError> {
    let (r, kind, token) = fixture(32)?;
    for case in 0..4 {
        let mut invalid = kind.clone();
        match case {
            0 => invalid.schema.package = "missing".into(),
            1 => invalid.schema.revision += 1,
            2 => invalid.schema.digest.0[0] ^= 1,
            _ => invalid.local_kind = u64::MAX,
        }
        let expected = if case == 3 {
            SchemaError::UnknownType
        } else {
            SchemaError::UnknownSchema
        };
        let mut arena = ParseArena::default();
        assert_eq!(
            arena.head(token.clone(), &invalid, &r, &mut budget()),
            Err(SyntaxError::Schema(expected))
        );
        assert!(arena.nodes.is_empty() && arena.tokens.is_empty() && arena.origins.is_empty());
        let mut limits = budget().limits();
        limits.allocation_units = 0;
        assert_eq!(
            arena.head(token.clone(), &invalid, &r, &mut Budget::new(limits)),
            Err(SyntaxError::Stopped(StopReason::AllocationLimit))
        );
        assert!(arena.nodes.is_empty() && arena.tokens.is_empty() && arena.origins.is_empty());
    }
    Ok(())
}
