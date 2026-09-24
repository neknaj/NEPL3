use super::*;
use alloc::{boxed::Box, string::String};
use budget as b;
use nepl3_core::{
    budget::Limits,
    origin::{Origin, OriginId},
    source::{SourceId, SourceSnapshot, SourceStore},
    syntax::{
        Environment, EnvironmentEntry, EnvironmentRef, ForeignClosure, ForeignSyntax, NodeRef,
        SyntaxBundle, SyntaxNode,
    },
    value_codec::FoundationValueCodec,
};
use nepl3_wire::foundation::FoundationCodec;
fn err(e: impl core::fmt::Debug) -> String {
    alloc::format!("{e:?}")
}
#[path = "../../tests/support/closure.rs"]
mod support;
pub(super) use support::closure;

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 100_000,
        output_bytes: 100_000,
        work: 10_000_000,
        nodes: 100_000,
        allocation_units: 10_000_000,
        depth: 100,
        ..Limits::default()
    })
}

#[test]
fn native_structure_proof_reuses_validation_and_keeps_labels() -> Result<(), alloc::string::String>
{
    let err = |e| alloc::format!("{e:?}");
    let mut registry = SchemaRegistry::default();
    let descriptor = crate::schema::descriptor(&mut budget()).map_err(err)?;
    registry
        .register(
            descriptor.reference(&mut budget()).map_err(err)?,
            descriptor,
            &mut budget(),
        )
        .map_err(err)?;
    // Foundation types referenced by the Doc schema remain part of validation.
    let descriptor = nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?;
    registry
        .register(
            descriptor.reference(&mut budget()).map_err(err)?,
            descriptor,
            &mut budget(),
        )
        .map_err(err)?;
    registry.finalize(&mut budget()).map_err(err)?;
    let kinds = vec![
        DocKind::Article {
            language: "ja".into(),
            title: SentenceRef(1),
            body: BodyRef(2),
        },
        DocKind::Sentence {
            syntax: EmbedRef(0),
        },
        DocKind::Body {
            blocks: vec![BlockRef(3)],
        },
        DocKind::Section {
            id: "entry".into(),
            title: SentenceRef(1),
            body: BodyRef(4),
        },
        DocKind::Body { blocks: vec![] },
    ];
    let mut guest = closure(&registry)?;
    guest.syntax.category = "Sentence".into();
    let document = DocumentSyntax {
        value: DocValue {
            root: DocRoot::Article(ArticleRef(0)),
            nodes: kinds
                .into_iter()
                .map(|kind| DocNode {
                    kind,
                    locations: vec![],
                    origin: None,
                    span: None,
                })
                .collect(),
            embeds: vec![DocEmbed {
                kind: EmbedKind::Sentence,
                content: DocContent::Syntax {
                    closure: Box::new(guest),
                },
            }],
        },
        sources: vec![],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    };
    let mut admission = SourceAdmission::default();
    let mut full = budget();
    let ordinary = check(&document, &registry, &mut full, &mut admission)
        .map_err(|e| alloc::format!("{e:?}"))?;
    let proof = document
        .validate_structure(&registry, &mut budget(), &mut admission)
        .map_err(|e| alloc::format!("{e:?}"))?;
    let mut reused = budget();
    let actual = check_structure(&proof, &mut reused).map_err(|e| alloc::format!("{e:?}"))?;
    assert_eq!(actual.definitions(), ordinary.definitions());
    assert_eq!(actual.references(), ordinary.references());
    assert_eq!(actual.definitions()[0].name, "entry");
    assert!(core::ptr::eq(actual.document(), &document));
    assert!(reused.usage().work < full.usage().work);
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert!(matches!(
        check_structure(&proof, &mut stopped),
        Err(LabelError::Stopped(StopReason::WorkLimit))
    ));
    Ok(())
}
