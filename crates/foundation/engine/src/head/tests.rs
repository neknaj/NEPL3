use super::*;
use alloc::{vec, vec::Vec};
use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    origin::OriginId,
    source::{Digest, SourceAdmission, SourceId, SourceSnapshot},
    syntax::{FieldValue, NodeRef, SyntaxNode, TokenRef},
    value::{KindRef, NdfValue, SchemaRef},
    view::{Token, ViewBundle},
};

fn limits() -> Limits {
    Limits {
        source_bytes: 100_000,
        work: 1_000_000,
        depth: 128,
        nodes: 10_000,
        allocation_units: 1_000_000,
        output_bytes: 100_000,
        diagnostics: 100,
        events: 100,
    }
}
fn source(text: &str) -> Result<SourceSnapshot, HeadError> {
    Ok(SourceSnapshot::new(
        SourceId("head-test".into()),
        0,
        "memory:head-test".into(),
        text.as_bytes().to_vec(),
        &mut Budget::new(limits()),
    )?)
}
fn token(
    source: &SourceSnapshot,
    start: u64,
    end: u64,
    payload: NdfValue,
) -> Result<Token, HeadError> {
    Ok(Token {
        // Projection does not manufacture a schema proof. The parser separately
        // validates the token against its actual registered package before this step.
        kind: KindRef {
            schema: SchemaRef {
                package: "fixture".into(),
                revision: 1,
                digest: Digest::of(b"fixture"),
            },
            local_kind: 0,
        },
        head: source.span(start, end)?,
        payload,
        views: ViewBundle {
            elements: Vec::new(),
            roots: Vec::new(),
        },
        leading_trivia: Vec::new(),
    })
}

#[test]
fn window_preserves_exact_utf8_and_requires_admission_in_each_operation() -> Result<(), HeadError> {
    let source = source("a\u{03bb} unread")?;
    let mut b = Budget::new(limits());
    let mut a = SourceAdmission::default();
    let projected = SourceWindow::capture(&source, 1, 3, &mut b, &mut a)?;
    assert_eq!(projected.bytes, "\u{03bb}".as_bytes());
    projected.validate(&mut b)?;
    SourceWindow::capture(&source, 1, 3, &mut b, &mut a)?;
    assert_eq!(b.usage().source_bytes, source.text().len() as u64);
    assert!(matches!(
        SourceWindow::capture(&source, 1, 2, &mut b, &mut a),
        Err(HeadError::Source(
            nepl3_core::source::SourceError::ScalarBoundary
        ))
    ));
    let mut limits = limits();
    limits.source_bytes = 0;
    assert_eq!(
        SourceWindow::capture(
            &source,
            1,
            3,
            &mut Budget::new(limits),
            &mut SourceAdmission::default()
        ),
        Err(HeadError::Stopped(StopReason::SourceLimit))
    );
    let mut forged = projected;
    forged.bytes = vec![0xff, 0xff];
    assert_eq!(forged.validate(&mut b), Err(HeadError::Projection));
    Ok(())
}

#[test]
fn completed_projection_preserves_compound_values_without_unread_siblings_or_tables()
-> Result<(), HeadError> {
    let source = source("choose 1 unread")?;
    let payload = NdfValue::List(vec![NdfValue::Text("Body".into()), NdfValue::U64(1)]);
    let tokens = vec![
        token(&source, 0, 6, NdfValue::Unit)?,
        token(&source, 7, 8, payload.clone())?,
        token(&source, 9, 15, NdfValue::Text("unread".into()))?,
    ];
    let nodes: Vec<_> = tokens
        .iter()
        .enumerate()
        .map(|(i, token)| SyntaxNode {
            schema: token.kind.schema.clone(),
            kind: "Node".into(),
            fields: Vec::new(),
            head: Some(token.head.clone()),
            cover: Some(token.head.clone()),
            origin: OriginId(123),
            token: Some(TokenRef(i as u64)),
        })
        .collect();
    let mut b = Budget::new(limits());
    let mut a = SourceAdmission::default();
    let projected = capture_completed(
        &nodes,
        &tokens,
        core::slice::from_ref(&source),
        &[FieldValue::Child(NodeRef(1))],
        &mut b,
        &mut a,
    )?;
    assert_eq!(projected.roots, vec![ProjectedNodeRef(0)]);
    assert_eq!(projected.nodes.len(), 1);
    assert_eq!(
        projected.nodes[0]
            .token
            .as_ref()
            .ok_or(HeadError::Projection)?
            .payload,
        payload
    );
    assert_eq!(projected.windows.len(), 1);
    assert_eq!(projected.windows[0].bytes, b"1");
    assert_eq!(projected.windows[0].span.start, 7);
    // Input admission counts the immutable source once; the operation payload
    // contains only the completed child's single byte, not its sibling or parent.
    assert_eq!(b.usage().source_bytes, 15);
    let head = ProjectedHead::capture(&tokens[0], &source, &mut b, &mut a)?;
    assert_eq!(head.window.bytes, b"choose");
    Ok(())
}
