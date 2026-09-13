use super::*;
use nepl3_core::{
    origin::{Origin, OriginId},
    syntax::*,
};
use nepl3_sentence_core::{
    model::*,
    syntax::{NodeLocation, SentenceSyntax},
};
#[test]
fn doc_bridge_preserves_generated_shared_inline_and_rejects_unselected_foreign()
-> Result<(), String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_sentence_core::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    let location = NodeLocation {
        origin: OriginId(0),
        head: None,
        cover: None,
    };
    let mut input = SentenceSyntax {
        value: SentenceValue {
            root: Root::Inline(InlineRef(1)),
            embeds: vec![],
            nodes: vec![
                Kind::Break,
                Kind::Concat {
                    inlines: vec![InlineRef(0), InlineRef(0)],
                },
            ],
        },
        locations: vec![location.clone(), location.clone()],
        sources: vec![],
        views: vec![],
        source_maps: vec![],
        origins: vec![Origin::Synthetic {
            reason: "generated sentence".into(),
            anchor: None,
        }],
    };
    let out =
        nepl3_tools::doc::sentence::document(&input, &r, &mut b(), &mut SourceAdmission::default())
            .map_err(err)?;
    use nepl3_doc_core::model as d;
    assert_eq!(out.value.root, d::DocRoot::Inline(d::InlineRef(1)));
    assert_eq!(
        out.value.nodes[1].kind,
        d::DocKind::Concat {
            inlines: vec![d::InlineRef(0), d::InlineRef(0)]
        }
    );
    assert!(
        out.value
            .nodes
            .iter()
            .all(|n| n.span.is_none() && n.origin == Some(OriginId(0)))
    );
    let mut budget = b();
    budget.cancel();
    assert_eq!(
        nepl3_tools::doc::sentence::document(
            &input,
            &r,
            &mut budget,
            &mut SourceAdmission::default()
        )
        .err(),
        Some(nepl3_tools::doc::sentence::Error::Stopped(
            StopReason::Cancelled
        ))
    );
    let mut limits = b().limits();
    limits.work = 0;
    assert_eq!(
        nepl3_tools::doc::sentence::document(
            &input,
            &r,
            &mut Budget::new(limits),
            &mut SourceAdmission::default()
        )
        .err(),
        Some(nepl3_tools::doc::sentence::Error::Stopped(
            StopReason::WorkLimit
        ))
    );
    let foundation = r
        .selected("nepl3.foundation", 1)
        .ok_or("foundation")?
        .clone();
    let env = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let digest = nepl3_wire::environment::environment_digest(&env, &foundation, &r, &mut b())
        .map_err(err)?;
    // Structurally valid foreign syntax; no guest language or semantic role is
    // inferred from its category or foundation kind. The bridge must require
    // a selected adapter even after closure validation succeeds.
    let closure = ForeignClosure {
        syntax: ForeignSyntax {
            schema: foundation.clone(),
            category: "test-inline".into(),
            root: NodeRef(0),
            bundle: SyntaxBundle {
                sources: vec![],
                nodes: vec![SyntaxNode {
                    schema: foundation,
                    kind: "NodeRef".into(),
                    fields: vec![],
                    head: None,
                    cover: None,
                    origin: OriginId(0),
                    token: None,
                }],
                origins: input.origins.clone(),
                root: NodeRef(0),
                environments: vec![],
                tokens: vec![],
                source_maps: vec![],
            },
            environment: EnvironmentRef { id: 0, digest },
        },
        owner_environment: EnvironmentEntry {
            id: 0,
            digest,
            value: env,
        },
        owner_origins: vec![],
        owner_sources: vec![],
        owner_source_maps: vec![],
    };
    input.value.nodes = vec![Kind::ForeignInline {
        syntax: EmbedRef(0),
    }];
    input.value.root = Root::Inline(InlineRef(0));
    input.value.embeds = vec![closure];
    input.locations = vec![location];
    input
        .validate(&r, &mut b(), &mut SourceAdmission::default())
        .map_err(err)?;
    assert_eq!(
        nepl3_tools::doc::sentence::document(&input, &r, &mut b(), &mut SourceAdmission::default())
            .err(),
        Some(nepl3_tools::doc::sentence::Error::ForeignAdapterRequired(
            EmbedRef(0)
        ))
    );
    Ok(())
}
