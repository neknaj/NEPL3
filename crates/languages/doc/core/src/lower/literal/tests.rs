use super::*;
use nepl3_core::{budget::Limits, source::SourceId};

fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 1_000_000,
        allocation_units: 1_000_000,
        ..Limits::default()
    })
}
fn source(uri: &str, text: &str) -> Result<SourceSnapshot, LowerError> {
    SourceSnapshot::new(
        SourceId("owner".into()),
        0,
        uri.into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| SyntaxError::Source(e).into())
}
fn document(source: SourceSnapshot) -> DocumentSyntax {
    DocumentSyntax {
        value: DocValue {
            root: DocRoot::Sentence(SentenceRef(0)),
            nodes: vec![DocNode {
                kind: DocKind::Sentence { inlines: vec![] },
                origin: None,
                span: None,
                locations: vec![],
            }],
            embeds: vec![],
        },
        sources: vec![source],
        origins: vec![],
        views: vec![],
        source_maps: vec![],
    }
}
fn presentation(source: SourceSnapshot) -> Presentation {
    Presentation {
        sources: vec![source],
        origins: vec![],
        maps: vec![],
    }
}

#[test]
fn independently_decoded_sources_keep_equality_conflict_and_exact_stops() -> Result<(), LowerError>
{
    let owner = source("memory:owner", "literal source")?;
    let equal = source("memory:owner", "literal source")?;
    let mut output = document(owner.clone());
    let mut measured = budget();
    append(
        vec![presentation(equal.clone())],
        &mut output,
        &mut measured,
    )?;
    assert_eq!(output.sources, vec![owner.clone()]);
    let work = measured.usage().work;
    for (limit, succeeds) in [(work, true), (work - 1, false)] {
        let mut limits = budget().limits();
        limits.work = limit;
        let mut b = Budget::new(limits);
        let mut output = document(owner.clone());
        let result = append(vec![presentation(equal.clone())], &mut output, &mut b);
        if succeeds {
            result?;
        } else {
            assert_eq!(result, Err(LowerError::Stopped(StopReason::WorkLimit)));
            assert_eq!(b.poll(), Err(StopReason::WorkLimit));
        }
        assert_eq!(output.sources, vec![owner.clone()]);
    }
    // Identical identity/text but a different URI must not be admitted by a
    // digest-only equality shortcut.
    let conflicting = source("memory:other", "literal source")?;
    assert_eq!(owner.identity(), conflicting.identity());
    assert_eq!(
        append(vec![presentation(conflicting)], &mut output, &mut budget()),
        Err(LowerError::Syntax(SyntaxError::Source(
            SourceError::IdentityConflict
        )))
    );
    let mut cancelled = budget();
    cancelled.stop(StopReason::Cancelled);
    assert_eq!(
        append(vec![presentation(owner)], &mut output, &mut cancelled),
        Err(LowerError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[cfg(target_has_atomic = "ptr")]
#[test]
fn shared_owner_comparison_is_independent_of_document_bytes() -> Result<(), LowerError> {
    let mut used = Vec::new();
    for length in [16, 100_000] {
        let owner = source("memory:owner", &"x".repeat(length))?;
        let mut output = document(owner.clone());
        let presentations = (0..100).map(|_| presentation(owner.clone())).collect();
        let mut b = budget();
        append(presentations, &mut output, &mut b)?;
        assert_eq!(output.sources, vec![owner]);
        used.push(b.usage().work);
    }
    assert_eq!(used[0], used[1]);
    Ok(())
}
