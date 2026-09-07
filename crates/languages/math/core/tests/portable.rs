use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
    value::{Integer, NdfValue},
};
use nepl3_math_core::{model::*, number, portable};
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        allocation_units: 500_000_000,
        nodes: 1_000_000,
        depth: 100_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_math_core::schema::descriptor(&mut b()),
    ] {
        let d = descriptor.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn source_expression() -> Result<MathSyntax, String> {
    let source = SourceSnapshot::new(
        SourceId("math-source".into()),
        7,
        "memory:math-source".into(),
        b"frac 1 2".to_vec(),
        &mut b(),
    )
    .map_err(err)?;
    let cover = source.span(0, 8).map_err(err)?;
    let one = source.span(5, 6).map_err(err)?;
    let two = source.span(7, 8).map_err(err)?;
    let rational =
        |n| number::ratio(&Integer::from(n), &Integer::from(1_i64), &mut b()).map_err(err);
    let node = |kind, origin, span| MathNode {
        kind,
        origin: Some(OriginId(origin)),
        span: Some(span),
        locations: vec![],
    };
    Ok(MathSyntax {
        value: MathValue {
            root: MathRoot::Expr(ExprRef(2)),
            nodes: vec![
                node(
                    MathKind::Number {
                        value: rational(1_i64)?,
                        spelling: Some(one.clone()),
                    },
                    0,
                    one.clone(),
                ),
                node(
                    MathKind::Number {
                        value: rational(2_i64)?,
                        spelling: Some(two.clone()),
                    },
                    1,
                    two.clone(),
                ),
                node(
                    MathKind::Frac {
                        left: ExprRef(0),
                        right: ExprRef(1),
                    },
                    2,
                    cover.clone(),
                ),
            ],
            embeds: vec![],
        },
        sources: vec![source],
        origins: vec![
            Origin::Direct(one),
            Origin::Direct(two),
            Origin::Direct(cover),
        ],
        views: vec![],
        source_maps: vec![],
    })
}
#[test]
fn first_receiver_keeps_fraction_notation_spelling_and_source_once() -> Result<(), String> {
    let r = registry()?;
    let original = source_expression()?;
    let empty = SourceStore::default();
    let (value, bytes) = {
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
        let value = portable::to_value(&original, &r, &mut c, &mut b()).map_err(err)?;
        let bytes = nepl3_wire::encode(&value, &mut b()).map_err(err)?;
        (value, bytes)
    };
    let decoded = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let mut receiver = b();
    let actual = portable::from_value(&decoded, &r, &mut c, &mut receiver).map_err(err)?;
    assert_eq!(actual, original);
    assert!(matches!(actual.value.nodes[2].kind, MathKind::Frac { .. }));
    assert_eq!(receiver.usage().source_bytes, 8);
    assert_eq!(
        portable::to_value(&actual, &r, &mut c, &mut b()).map_err(err)?,
        value
    );
    Ok(())
}
#[test]
fn schema_valid_math_rejects_nonfinite_numbers_and_missing_source_closure() -> Result<(), String> {
    let r = registry()?;
    let original = source_expression()?;
    let empty = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
    let value = portable::to_value(&original, &r, &mut c, &mut b()).map_err(err)?;
    let mut malformed = value.clone();
    let NdfValue::Record(syntax) = &mut malformed else {
        return Err("syntax record".into());
    };
    syntax.fields[1] = NdfValue::List(vec![]);
    let mut ambient = SourceStore::default();
    ambient.insert(original.sources[0].clone()).map_err(err)?;
    let mut a = SourceAdmission::default();
    let mut receiver = FoundationCodec::new(&r, &ambient, &mut a).map_err(err)?;
    assert!(portable::from_value(&malformed, &r, &mut receiver, &mut b()).is_err());
    let mut invalid = original.clone();
    invalid.value.nodes[0].kind = MathKind::Number {
        value: number::ratio(&Integer::from(1_i64), &Integer::from(3_i64), &mut b())
            .map_err(err)?,
        spelling: None,
    };
    assert!(matches!(
        portable::to_value(&invalid, &r, &mut c, &mut b()),
        Err(portable::PortableError::Structure(
            nepl3_math_core::check::StructureError::Shape(
                nepl3_math_core::check::ShapeError::NonFiniteDecimalNumber(0)
            )
        ))
    ));
    for reason in [
        StopReason::SourceLimit,
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::SourceLimit => limits.source_bytes = 0,
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::NodeLimit => limits.nodes = 0,
            StopReason::DepthLimit => limits.depth = 0,
            _ => {}
        }
        let mut budget = Budget::new(limits);
        let mut a = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(&r, &empty, &mut a).map_err(err)?;
        assert!(
            matches!(portable::from_value(&value, &r, &mut receiver, &mut budget), Err(portable::PortableError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(budget.poll(), Err(reason));
    }
    Ok(())
}
