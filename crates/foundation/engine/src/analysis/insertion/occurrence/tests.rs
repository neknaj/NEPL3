use super::*;
use alloc::vec;
use nepl3_core::{
    budget::Limits,
    origin::OriginId,
    source::{Digest, SourceId, SourceSnapshot},
    value::SchemaRef,
};

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10000,
        source_bytes: 10000,
        nodes: 100,
        depth: 100,
        allocation_units: 10000,
        ..Limits::default()
    })
}
enum OriginKind {
    Direct,
    Synthetic,
    Generated,
}
fn check_geometry(start: u64, end: u64, kind: OriginKind) -> Result<Span, InsertionError> {
    let mut b = budget();
    let old = SourceSnapshot::new(
        SourceId("input".into()),
        0,
        "memory:input".into(),
        b"let 123".to_vec(),
        &mut b,
    )?;
    let new = SourceSnapshot::new(
        SourceId("input".into()),
        1,
        "memory:input".into(),
        b"let f123".to_vec(),
        &mut b,
    )?;
    let cover = new.span(start, end)?;
    let origin = match kind {
        OriginKind::Direct => Origin::Direct(cover.clone()),
        OriginKind::Synthetic => Origin::Synthetic {
            reason: "unsupported".into(),
            anchor: Some(cover.clone()),
        },
        OriginKind::Generated => Origin::Generated {
            operation: nepl3_core::value::OperationRef {
                schema: SchemaRef {
                    package: "fixture".into(),
                    revision: 1,
                    digest: Digest::of(b"fixture"),
                },
                name: "generate".into(),
            },
            callsite: Some(cover.clone()),
            inputs: vec![],
        },
    };
    let mut store = nepl3_core::source::SourceStore::default();
    store.insert(new.clone())?;
    let valid = nepl3_core::origin::OriginGraph::validate_origins(
        core::slice::from_ref(&origin),
        &store,
        &mut b,
    );
    assert!(valid.is_ok(), "valid origin fixture: {valid:?}");
    // This unit test exercises only the private span predicate. It does not mint
    // an execution proof or bypass public observe's prepared/retained gates.
    let bundle = SyntaxBundle {
        sources: vec![],
        nodes: vec![SyntaxNode {
            schema: SchemaRef {
                package: "fixture".into(),
                revision: 1,
                digest: Digest::of(b"fixture"),
            },
            kind: "fixture".into(),
            fields: vec![],
            head: Some(cover.clone()),
            cover: Some(cover),
            origin: OriginId(0),
            token: None,
        }],
        origins: vec![origin],
        root: NodeRef(0),
        environments: vec![],
        tokens: vec![],
        source_maps: vec![],
    };
    direct_cover(
        &bundle,
        NodeRef(0),
        &new,
        &TextEdit {
            span: old.span(4, 4)?,
            expected_digest: Digest::of(b""),
            replacement: "f".into(),
        },
        &mut budget(),
    )
}
#[test]
fn positive_direct_cover_is_wholly_inside_inserted_bytes() -> Result<(), InsertionError> {
    let span = check_geometry(4, 5, OriginKind::Direct)?;
    assert_eq!((span.start(), span.end()), (4, 5));
    assert_eq!(
        check_geometry(4, 8, OriginKind::Direct),
        Err(InsertionError::UnsupportedOrigin)
    );
    assert_eq!(
        check_geometry(3, 5, OriginKind::Direct),
        Err(InsertionError::UnsupportedOrigin)
    );
    assert_eq!(
        check_geometry(4, 4, OriginKind::Direct),
        Err(InsertionError::UnsupportedOrigin)
    );
    Ok(())
}
#[test]
fn an_anchor_inside_the_insertion_does_not_authorize_synthetic_provenance() {
    assert_eq!(
        check_geometry(4, 5, OriginKind::Synthetic),
        Err(InsertionError::UnsupportedOrigin)
    );
}

#[test]
fn generated_callsite_inside_the_insertion_is_not_direct_attribution() {
    assert_eq!(
        check_geometry(4, 5, OriginKind::Generated),
        Err(InsertionError::UnsupportedOrigin)
    );
}
