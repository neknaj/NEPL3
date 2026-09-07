//! Budgeted exact coverage, independent of how a correspondence is segmented.
use super::*;
use nepl3_core::origin::Mapping;

pub(super) fn project(
    span: &Span,
    maps: &[Mapping],
    sources: &[SourceSnapshot],
    reverse: bool,
    b: &mut Budget,
) -> Result<Vec<Span>, RenameError> {
    // One group represents a single snapshot and constant byte displacement.
    // Forward projection may have several outputs; inverse callers require one.
    let mut groups: Vec<(&Span, i128)> = Vec::new();
    for map in maps {
        let (from, to) = sides(map, reverse);
        if !same_source(span.snapshot_ref(), from.snapshot_ref(), b)?
            || from.start() >= span.end()
            || span.start() >= from.end()
        {
            continue;
        }
        if map.kind != MappingKind::Exact {
            return Err(RenameError::RenameNotInvertible);
        }
        let offset = i128::from(to.start()) - i128::from(from.start());
        let mut known = false;
        for (other, delta) in &groups {
            if same_source(other.snapshot_ref(), to.snapshot_ref(), b)? && *delta == offset {
                known = true;
            }
        }
        if !known {
            push(&mut groups, (to, offset), b)?;
        }
    }
    let mut result = Vec::new();
    for (target, offset) in groups {
        let mut cursor = span.start();
        while cursor < span.end() {
            let mut next = cursor;
            for map in maps {
                let (from, to) = sides(map, reverse);
                if !same_source(span.snapshot_ref(), from.snapshot_ref(), b)?
                    || !same_source(target.snapshot_ref(), to.snapshot_ref(), b)?
                    || map.kind != MappingKind::Exact
                    || i128::from(to.start()) - i128::from(from.start()) != offset
                {
                    continue;
                }
                if from.start() <= cursor && cursor < from.end() {
                    next = next.max(from.end().min(span.end()));
                }
            }
            if next == cursor {
                return Err(RenameError::RenameNotInvertible);
            }
            cursor = next;
        }
        let mut snapshot = None;
        for source in sources {
            if same_source(source.identity(), target.snapshot_ref(), b)? {
                snapshot = Some(source);
                break;
            }
        }
        let snapshot = snapshot.ok_or(SourceError::MissingSnapshot)?;
        let start = u64::try_from(i128::from(span.start()) + offset)
            .map_err(|_| RenameError::RenameNotInvertible)?;
        let end = u64::try_from(i128::from(span.end()) + offset)
            .map_err(|_| RenameError::RenameNotInvertible)?;
        push(&mut result, snapshot.span_with_budget(start, end, b)?, b)?;
    }
    Ok(result)
}
fn sides(map: &Mapping, reverse: bool) -> (&Span, &Span) {
    if reverse {
        (&map.target, &map.source)
    } else {
        (&map.source, &map.target)
    }
}
