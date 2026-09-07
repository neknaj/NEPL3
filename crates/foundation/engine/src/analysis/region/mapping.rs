//! Display projections retain ambiguity and partial correspondence. They do not
//! grant the unique inverse required by rename.
use super::*;
use nepl3_core::{
    origin::{Mapping, MappingKind},
    syntax::canonical::BundleMappings,
};

pub(super) fn project(
    at: &Span,
    requested: &SourceRef,
    map: &BundleMappings<'_>,
    declared: &[Mapping],
    b: &mut Budget,
) -> Result<Vec<(Span, RegionMapping)>, RegionError> {
    let mut edges = Vec::new();
    for edge in declared {
        b.charge(Resource::Work, 1)?;
        push(&mut edges, edge, b)?;
    }
    let mut pending = Vec::new();
    push(&mut pending, (span(at, b)?, RegionMapping::Direct, 1u64), b)?;
    let mut visited: Vec<(Span, RegionMapping)> = Vec::new();
    let mut out = Vec::new();
    while let Some((at, quality, depth)) = pending.pop() {
        b.charge(Resource::Nodes, 1)?;
        b.observe_depth(depth)?;
        let mut seen = false;
        for (prior, q) in &visited {
            if same(prior, &at, b)?
                && prior.start() == at.start()
                && prior.end() == at.end()
                && *q == quality
            {
                seen = true;
                break;
            }
        }
        if seen {
            continue;
        }
        push(&mut visited, (span(&at, b)?, quality), b)?;
        b.charge(
            Resource::Work,
            (requested.source_id.0.len() + at.snapshot_ref().source.0.len()) as u64 + 40,
        )?;
        if requested.source_id == at.snapshot_ref().source
            && requested.revision == at.snapshot_ref().revision
            && requested.digest == at.snapshot_ref().digest
        {
            push(&mut out, (span(&at, b)?, quality), b)?;
        }
        for (next, q) in inverse(&at, &edges, map, b)? {
            let q = match (quality, q) {
                (RegionMapping::Transformed, _) | (_, RegionMapping::Transformed) => {
                    RegionMapping::Transformed
                }
                (RegionMapping::ExactFragment, _) | (_, RegionMapping::ExactFragment) => {
                    RegionMapping::ExactFragment
                }
                _ => RegionMapping::Exact,
            };
            push(&mut pending, (next, q, depth.saturating_add(1)), b)?;
        }
    }
    let quality = |q: RegionMapping| match q {
        RegionMapping::Direct => 0,
        RegionMapping::Exact => 1,
        RegionMapping::ExactFragment => 2,
        RegionMapping::Transformed => 3,
    };
    for i in 1..out.len() {
        let mut j = i;
        while j > 0 {
            b.charge(Resource::Work, 1)?;
            let a = &out[j - 1];
            let c = &out[j];
            if (a.0.start(), a.0.end(), quality(a.1)) <= (c.0.start(), c.0.end(), quality(c.1)) {
                break;
            }
            out.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(out)
}
fn same(a: &Span, c: &Span, b: &mut Budget) -> Result<bool, RegionError> {
    b.charge(
        Resource::Work,
        (a.snapshot_ref().source.0.len() + c.snapshot_ref().source.0.len()) as u64 + 40,
    )?;
    Ok(a.snapshot_ref() == c.snapshot_ref())
}
fn inverse(
    at: &Span,
    edges: &[&Mapping],
    map: &BundleMappings<'_>,
    b: &mut Budget,
) -> Result<Vec<(Span, RegionMapping)>, RegionError> {
    let mut groups: Vec<(&Span, i128)> = Vec::new();
    let mut out = Vec::new();
    for edge in edges {
        if !same(&edge.target, at, b)?
            || edge.target.start() >= at.end()
            || at.start() >= edge.target.end()
        {
            continue;
        }
        if edge.kind == MappingKind::Transformed {
            push(
                &mut out,
                (span(&edge.source, b)?, RegionMapping::Transformed),
                b,
            )?;
            continue;
        }
        let delta = i128::from(edge.source.start()) - i128::from(edge.target.start());
        let mut found = false;
        for (source, d) in &groups {
            if *d == delta && same(source, &edge.source, b)? {
                found = true;
                break;
            }
        }
        if !found {
            push(&mut groups, (&edge.source, delta), b)?;
        }
    }
    for (source, delta) in groups {
        let mut ranges = Vec::new();
        for edge in edges {
            if edge.kind != MappingKind::Exact
                || !same(&edge.target, at, b)?
                || !same(&edge.source, source, b)?
                || i128::from(edge.source.start()) - i128::from(edge.target.start()) != delta
            {
                continue;
            }
            let start = at.start().max(edge.target.start());
            let end = at.end().min(edge.target.end());
            if start < end {
                push(&mut ranges, (start, end), b)?;
            }
        }
        // Source table order and Exact segmentation are not presentation rank.
        for i in 1..ranges.len() {
            let mut j = i;
            while j > 0 {
                b.charge(Resource::Work, 1)?;
                if ranges[j - 1] <= ranges[j] {
                    break;
                }
                ranges.swap(j - 1, j);
                j -= 1;
            }
        }
        let mut merged: Vec<(u64, u64)> = Vec::new();
        for (start, end) in ranges {
            b.charge(Resource::Work, 1)?;
            if let Some(prior) = merged.last_mut()
                && start <= prior.1
            {
                prior.1 = prior.1.max(end);
            } else {
                push(&mut merged, (start, end), b)?;
            }
        }
        for (start, end) in merged {
            let quality = if start == at.start() && end == at.end() {
                RegionMapping::Exact
            } else {
                RegionMapping::ExactFragment
            };
            let source = check::source(map, source, b)?;
            let start =
                u64::try_from(i128::from(start) + delta).map_err(|_| RegionError::Mapping)?;
            let end = u64::try_from(i128::from(end) + delta).map_err(|_| RegionError::Mapping)?;
            push(
                &mut out,
                (source.span_with_budget(start, end, b)?, quality),
                b,
            )?;
        }
    }
    Ok(out)
}
