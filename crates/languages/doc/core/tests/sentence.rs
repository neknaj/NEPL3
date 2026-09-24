//! Doc normalization remains a consumer responsibility; literal parsing belongs
//! to Sentence core and its bridge is exercised by the host integration tests.
use budget as b;
use nepl3_core::{
    budget::{Budget, Limits},
    origin::{Origin, OriginId},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceId, SourceSnapshot, SourceStore},
};
use nepl3_core::{
    syntax::{
        Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
        SyntaxBundle, SyntaxNode,
    },
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::model::*;
use nepl3_wire::foundation::FoundationCodec;
#[path = "support/closure.rs"]
mod support;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 100_000,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 1_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}

#[test]
fn normalization_preserves_sentence_ownership_and_known_or_absent_locations() -> Result<(), String>
{
    let mut b = budget();
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut b),
        nepl3_doc_core::schema::descriptor(&mut b),
    ] {
        let descriptor = descriptor.map_err(err)?;
        registry
            .register(
                descriptor.reference(&mut b).map_err(err)?,
                descriptor,
                &mut b,
            )
            .map_err(err)?;
    }
    registry.finalize(&mut b).map_err(err)?;
    let source = SourceSnapshot::new(
        SourceId("sentence".into()),
        1,
        "memory:sentence".into(),
        b"a".to_vec(),
        &mut b,
    )
    .map_err(err)?;
    let span = source.span(0, 1).map_err(err)?;
    let mut guest = support::closure(&registry)?;
    guest.syntax.category = "Sentence".into();
    let input = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Block(BlockRef(0)),
            nodes: vec![
                DocNode {
                    kind: DocKind::Paragraph {
                        items: vec![FlowRef(2), FlowRef(1), FlowRef(2)],
                    },
                    locations: vec![],
                    origin: None,
                    span: None,
                },
                DocNode {
                    kind: DocKind::Sentence {
                        syntax: EmbedRef(0),
                    },
                    locations: vec![],
                    origin: None,
                    span: None,
                },
                DocNode {
                    kind: DocKind::Sentence {
                        syntax: EmbedRef(0),
                    },
                    locations: vec![],
                    origin: Some(OriginId(0)),
                    span: Some(span.clone()),
                },
            ],
            embeds: vec![DocEmbed {
                kind: EmbedKind::Sentence,
                content: DocContent::Syntax {
                    closure: Box::new(guest),
                },
            }],
        },
        sources: vec![source],
        origins: vec![Origin::Direct(span.clone())],
        views: vec![],
        source_maps: vec![],
    };
    let before = input.clone();
    let out = nepl3_doc_core::normalize::document(
        &input,
        &registry,
        &mut b,
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(out.value.root, DocRoot::Block(BlockRef(2)));
    assert_eq!(
        out.value.nodes[2].kind,
        DocKind::Paragraph {
            items: vec![FlowRef(0), FlowRef(1), FlowRef(0)]
        }
    );
    assert_eq!(out.value.nodes[0], input.value.nodes[2]);
    assert_eq!(out.value.nodes[1], input.value.nodes[1]);
    assert_eq!(out.value.embeds, input.value.embeds);
    assert_eq!(out.sources, input.sources);
    assert_eq!(out.origins, input.origins);
    assert_eq!(out.views, input.views);
    assert_eq!(out.source_maps, input.source_maps);
    assert_eq!(input, before);
    let again = nepl3_doc_core::normalize::document(
        &out,
        &registry,
        &mut budget(),
        &mut SourceAdmission::default(),
    )
    .map_err(err)?;
    assert_eq!(again, out);
    for (limits, reason) in [
        (
            Limits {
                work: 0,
                ..budget().limits()
            },
            nepl3_core::budget::StopReason::WorkLimit,
        ),
        (
            Limits {
                allocation_units: 0,
                ..budget().limits()
            },
            nepl3_core::budget::StopReason::AllocationLimit,
        ),
    ] {
        let mut limited = Budget::new(limits);
        assert!(
            matches!(nepl3_doc_core::normalize::document(&input, &registry, &mut limited, &mut SourceAdmission::default()), Err(nepl3_doc_core::check::StructureError::Stopped(actual)) if actual == reason)
        );
        assert_eq!(limited.poll(), Err(reason));
        assert_eq!(input, before);
    }
    Ok(())
}
