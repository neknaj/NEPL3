use super::*;
use nepl3_core::{
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::SourceAdmission,
    syntax::*,
};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn caller_depth_is_counted_once_at_the_exact_limit() -> Result<(), String> {
    let value = SentenceValue {
        root: Root::Inline(InlineRef(0)),
        nodes: vec![Kind::Text { text: "x".into() }],
        embeds: vec![],
    };
    let registry = SchemaRegistry::default();
    let prepared = print::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    for prepared_path in [false, true] {
        for limit in [1, 2] {
            let mut limits = b().limits();
            limits.depth = limit;
            let mut budget = Budget::new(limits);
            let result = budget.with_depth(|budget| {
                if prepared_path {
                    prepared.render(&[], budget)
                } else {
                    print::prefix(&value, budget)
                }
            });
            if limit == 2 {
                assert_eq!(result, Ok("text \"x\"".into()));
                assert_eq!(budget.usage().depth, 2);
            } else {
                assert_eq!(result, Err(Error::Stopped(StopReason::DepthLimit)));
            }
            assert_eq!(budget.current_depth(), 0);
        }
    }
    Ok(())
}
fn fixture() -> Result<(SchemaRegistry, SentenceValue), String> {
    let mut registry = SchemaRegistry::default();
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut b()).map_err(err)?;
    let schema = descriptor.reference(&mut b()).map_err(err)?;
    registry
        .register(schema.clone(), descriptor, &mut b())
        .map_err(err)?;
    registry.finalize(&mut b()).map_err(err)?;
    let environment = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest =
        nepl3_wire::environment::environment_digest(&environment, &schema, &registry, &mut b())
            .map_err(err)?;
    // A generated closure tests printing ownership; no guest grammar is implied.
    let closure = ForeignClosure {
        syntax: ForeignSyntax {
            schema: schema.clone(),
            category: "test-inline".into(),
            root: NodeRef(0),
            bundle: SyntaxBundle {
                nodes: vec![SyntaxNode {
                    schema,
                    kind: "NodeRef".into(),
                    fields: vec![],
                    head: None,
                    cover: None,
                    token: None,
                    origin: OriginId(0),
                }],
                root: NodeRef(0),
                sources: vec![],
                environments: vec![],
                tokens: vec![],
                origins: vec![Origin::Synthetic {
                    reason: "print fixture".into(),
                    anchor: None,
                }],
                source_maps: vec![],
            },
            environment: EnvironmentRef { id: 0, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 0,
            digest,
            value: environment,
        },
        owner_sources: vec![],
        owner_origins: vec![],
        owner_source_maps: vec![],
    };
    Ok((
        registry,
        SentenceValue {
            root: Root::Sentence(SentenceRef(1)),
            nodes: vec![
                Kind::ForeignInline {
                    syntax: EmbedRef(0),
                },
                Kind::Sentence {
                    inlines: vec![InlineRef(0), InlineRef(0)],
                },
            ],
            embeds: vec![closure],
        },
    ))
}

#[test]
fn selected_source_is_owner_bound_and_shared_occurrences_are_printed() -> Result<(), String> {
    let (registry, value) = fixture()?;
    let prepared = print::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let source = prepared
        .resolve(EmbedRef(0), "guest 7", &mut b())
        .map_err(err)?;
    assert_eq!(
        prepared.render(&[source], &mut b()).map_err(err)?,
        "sentence cons guest 7 cons guest 7 nil"
    );
    assert_eq!(
        prepared.render(&[], &mut b()),
        Err(Error::AdapterRequired(EmbedRef(0)))
    );
    let duplicates = [
        prepared
            .resolve(EmbedRef(0), "guest 7", &mut b())
            .map_err(err)?,
        prepared
            .resolve(EmbedRef(0), "guest 8", &mut b())
            .map_err(err)?,
    ];
    assert_eq!(
        prepared.render(&duplicates, &mut b()),
        Err(Error::Duplicate(EmbedRef(0)))
    );
    assert!(matches!(
        prepared.resolve(EmbedRef(1), "guest 7", &mut b()),
        Err(Error::Embed(EmbedRef(1)))
    ));
    let other = value.clone();
    let other_prepared =
        print::prepare(&other, &registry, &mut b(), &mut SourceAdmission::default())
            .map_err(err)?;
    let other_source = other_prepared
        .resolve(EmbedRef(0), "guest 7", &mut b())
        .map_err(err)?;
    assert_eq!(
        prepared.render(&[other_source], &mut b()),
        Err(Error::WrongScope)
    );
    let mut invalid = value.clone();
    invalid.embeds[0].syntax.environment.id = 1;
    assert!(
        print::prepare(
            &invalid,
            &registry,
            &mut b(),
            &mut SourceAdmission::default()
        )
        .is_err()
    );
    Ok(())
}

#[test]
fn prepared_print_enforces_current_limits_without_partial_output() -> Result<(), String> {
    let (registry, value) = fixture()?;
    let prepared = print::prepare(&value, &registry, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    let resolved = [prepared
        .resolve(EmbedRef(0), "guest 7", &mut b())
        .map_err(err)?];
    let mut measured = b();
    let expected = prepared.render(&resolved, &mut measured).map_err(err)?;
    let usage = measured.usage();
    for (axis, amount, reason) in [
        (0, usage.work, StopReason::WorkLimit),
        (1, usage.allocation_units, StopReason::AllocationLimit),
        (2, usage.output_bytes, StopReason::OutputLimit),
        (3, usage.depth, StopReason::DepthLimit),
    ] {
        for limit in [amount - 1, amount] {
            let mut limits = b().limits();
            match axis {
                0 => limits.work = limit,
                1 => limits.allocation_units = limit,
                2 => limits.output_bytes = limit,
                _ => limits.depth = limit,
            }
            let mut budget = Budget::new(limits);
            let result = prepared.render(&resolved, &mut budget);
            if limit == amount {
                assert_eq!(result.map_err(err)?, expected);
            } else {
                assert_eq!(result, Err(Error::Stopped(reason)));
            }
        }
    }
    let mut cancelled = b();
    cancelled.cancel();
    assert_eq!(
        prepared.render(&resolved, &mut cancelled),
        Err(Error::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
