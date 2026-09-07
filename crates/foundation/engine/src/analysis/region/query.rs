//! Queries use the selected structural owner and final binding resolutions.
//! Multiple source correspondences remain explicit occurrence candidates.
use super::*;
use crate::analysis::{
    BoundBindingReply,
    query::{self as binding_query, QueryError, QueryKind, QueryOutcome, QueryRequest},
};
use alloc::boxed::Box;
use nepl3_core::{
    facts::Occurrence,
    syntax::canonical::{BundleMappings, CanonicalError},
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionQueryRequest {
    pub region: RegionRequest,
    pub kind: QueryKind,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionQueryError {
    Region(RegionError),
    Query(QueryError),
}
impl RegionQueryError {
    pub fn stop_reason(&self) -> Option<StopReason> {
        match self {
            Self::Region(v) => v.stop_reason(),
            Self::Query(v) => v.stop_reason(),
        }
    }
}
impl From<RegionError> for RegionQueryError {
    fn from(v: RegionError) -> Self {
        Self::Region(v)
    }
}
impl From<QueryError> for RegionQueryError {
    fn from(v: QueryError) -> Self {
        Self::Query(v)
    }
}
impl From<StopReason> for RegionQueryError {
    fn from(v: StopReason) -> Self {
        Self::Region(v.into())
    }
}
impl From<SourceError> for RegionQueryError {
    fn from(v: SourceError) -> Self {
        Self::Region(v.into())
    }
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegionQueryOutcome {
    Complete {
        region: Option<Box<SourceRegion>>,
        /// One result per matching occurrence, in established issuance order.
        /// No region, region with no occurrence, and unresolved occurrences differ.
        queries: Vec<QueryOutcome>,
    },
    Invalid(RegionQueryError),
    Stopped(StopReason),
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RegionQueryReply {
    pub key: RegionKey,
    pub capability: RegionCapability,
    pub outcome: RegionQueryOutcome,
    pub report: Report,
    pub sources: Vec<SourceSnapshot>,
}
pub fn query(
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    request: &RegionQueryRequest,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> RegionQueryReply {
    let mut sources = Vec::new();
    let outcome = match b.with_depth(|b| run(input, binding, request, &mut sources, b, admission)) {
        Ok(v) => v,
        Err(e) => {
            sources.clear();
            match e.stop_reason() {
                Some(v) => RegionQueryOutcome::Stopped(v),
                None => RegionQueryOutcome::Invalid(e),
            }
        }
    };
    RegionQueryReply {
        key: request.region.key,
        capability: input.capability(),
        outcome,
        report: Report {
            usage: b.usage(),
            ..Report::default()
        },
        sources,
    }
}
fn run(
    input: &PreparedRegionInput<'_, '_, '_>,
    binding: &BoundBindingReply,
    request: &RegionQueryRequest,
    sources: &mut Vec<SourceSnapshot>,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<RegionQueryOutcome, RegionQueryError> {
    let analysis = binding
        .for_source(&request.region.key.analysis, &request.region.source, b)
        .map_err(QueryError::from)?;
    let reply = regions(input, &request.region, b, admission);
    let selected = match reply.outcome {
        RegionOutcome::Complete {
            selection,
            mut regions,
        } => selection.map(|i| regions.swap_remove(i as usize)),
        RegionOutcome::Invalid(e) => return Err(e.into()),
        RegionOutcome::Stopped(e) => return Err(e.into()),
    };
    *sources = reply.sources;
    let Some(region) = selected else {
        return Ok(RegionQueryOutcome::Complete {
            region: None,
            queries: Vec::new(),
        });
    };
    let maps = BundleMappings::new(&input.binding.tree.tree().bundle, b).map_err(|e| match e {
        CanonicalError::Stopped(v) => RegionError::from(v),
        _ => RegionError::Owner,
    })?;
    let owner_index = scope_owner(&region, &maps, b)?;
    let owner = maps
        .entries()
        .get(owner_index)
        .ok_or(RegionError::Owner)?
        .bundle();
    let ledger = &analysis.result().bundle_scopes;
    b.charge(Resource::Work, ledger.len() as u64 + 1)?;
    if ledger.len() != maps.entries().len() {
        return Err(QueryError::Facts.into());
    }
    let owner_ledger = ledger
        .iter()
        .find(|v| v.bundle == owner_index as u64)
        .ok_or(QueryError::Facts)?;
    let root = owner_ledger.scope;
    let mut declared = Vec::new();
    for map in &owner.source_maps {
        push(&mut declared, map.clone_with_budget(b)?, b)?;
    }
    for index in &owner_ledger.custom_source_maps {
        b.charge(Resource::Work, 1)?;
        let map = usize::try_from(*index)
            .ok()
            .and_then(|i| analysis.result().source_maps.get(i))
            .ok_or(QueryError::Facts)?;
        push(&mut declared, map.clone_with_budget(b)?, b)?;
    }
    let request_source = find_source(sources, &request.region.source, b)?;
    let offset = usize::try_from(request.region.offset).map_err(|_| RegionError::Mapping)?;
    let width = request_source
        .text()
        .get(offset..)
        .and_then(|s| s.chars().next())
        .map(char::len_utf8)
        .ok_or(RegionError::Mapping)?;
    let point = request_source.span_with_budget(
        request.region.offset,
        request.region.offset + width as u64,
        b,
    )?;
    let mut queries = Vec::new();
    for occurrence in &analysis.facts().occurrences {
        b.charge(Resource::Nodes, 1)?;
        b.charge(Resource::Work, 1)?;
        let namespace = analysis
            .facts()
            .namespaces
            .get(occurrence.namespace.0 as usize)
            .ok_or(QueryError::Facts)?;
        if namespace.root != root {
            continue;
        }
        let source = span_source(binding.reply().sources(), &occurrence.span, b)?;
        b.charge(Resource::Work, source.identity().source.0.len() as u64 + 42)?;
        b.charge(
            Resource::AllocationUnits,
            source.identity().source.0.len() as u64,
        )?;
        let reference = source.reference();
        let matches =
            b.with_depth_at_least(b.current_depth().saturating_add(region.depth), |b| {
                matches_occurrence(
                    occurrence,
                    &point,
                    &region.logical_span,
                    &reference,
                    &maps,
                    (&declared, binding.reply().sources()),
                    b,
                )
            })?;
        if !matches {
            continue;
        }
        let subrequest = QueryRequest {
            key: request.region.key.analysis,
            source: reference,
            offset: occurrence.span.start(),
            kind: request.kind,
        };
        let answer =
            binding_query::query_selected(binding, &subrequest, Some(occurrence.id), b, admission);
        match answer.outcome {
            QueryOutcome::Invalid(e) => return Err(e.into()),
            QueryOutcome::Stopped(e) => return Err(e.into()),
            outcome => push(&mut queries, outcome, b)?,
        }
        for source in answer.sources {
            add_source(sources, source, b)?;
        }
    }
    sort_sources(sources, b)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<SourceRegion>() as u64,
    )?;
    Ok(RegionQueryOutcome::Complete {
        region: Some(Box::new(region)),
        queries,
    })
}
fn matches_occurrence(
    occurrence: &Occurrence,
    point: &Span,
    logical: &Span,
    source: &SourceRef,
    maps: &BundleMappings<'_>,
    closure: (&[nepl3_core::origin::Mapping], &[SourceSnapshot]),
    b: &mut Budget,
) -> Result<bool, RegionError> {
    let (declared, sources) = closure;
    let points = mapping::correspond(point, source, maps, declared, sources, b)?;
    let areas = mapping::correspond(logical, source, maps, declared, sources, b)?;
    for (point, _) in &points {
        for (area, _) in &areas {
            b.charge(Resource::Work, 1)?;
            let start = point.start().max(area.start()).max(occurrence.span.start());
            let end = point.end().min(area.end()).min(occurrence.span.end());
            if start < end {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
fn scope_owner(
    region: &SourceRegion,
    maps: &BundleMappings<'_>,
    b: &mut Budget,
) -> Result<usize, RegionError> {
    let owner = usize::try_from(region.target.bundle).map_err(|_| RegionError::Owner)?;
    let mapping = maps.entries().get(owner).ok_or(RegionError::Owner)?;
    // A Foreign field is declared by the parent, but the field's contents use
    // the child's lexical root and map closure. Do not replace its region target.
    if let RegionPart::Field { field, .. } = region.target.part {
        let node = region.target.node.ok_or(RegionError::Owner)?;
        let native = mapping
            .order()
            .get(node as usize)
            .ok_or(RegionError::Owner)?;
        let field = mapping.bundle().nodes[*native]
            .fields
            .get(field as usize)
            .ok_or(RegionError::Owner)?;
        if let nepl3_core::syntax::FieldValue::Foreign(guest) = field {
            for (index, candidate) in maps.entries().iter().enumerate() {
                b.charge(Resource::Work, 1)?;
                if core::ptr::eq(candidate.bundle(), &guest.bundle) {
                    return Ok(index);
                }
            }
            return Err(RegionError::Owner);
        }
    }
    Ok(owner)
}
fn find_source<'a>(
    sources: &'a [SourceSnapshot],
    wanted: &SourceRef,
    b: &mut Budget,
) -> Result<&'a SourceSnapshot, RegionError> {
    for source in sources {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() + wanted.source_id.0.len()) as u64 + 42,
        )?;
        if source.identity().source == wanted.source_id
            && source.identity().revision == wanted.revision
            && source.identity().digest == wanted.digest
        {
            return Ok(source);
        }
    }
    Err(SourceError::MissingSnapshot.into())
}
fn span_source<'a>(
    sources: &'a [SourceSnapshot],
    wanted: &Span,
    b: &mut Budget,
) -> Result<&'a SourceSnapshot, RegionError> {
    for source in sources {
        b.charge(
            Resource::Work,
            (source.identity().source.0.len() + wanted.snapshot_ref().source.0.len()) as u64 + 42,
        )?;
        if source.identity() == wanted.snapshot_ref() {
            return Ok(source);
        }
    }
    Err(SourceError::MissingSnapshot.into())
}
fn add_source(
    out: &mut Vec<SourceSnapshot>,
    source: SourceSnapshot,
    b: &mut Budget,
) -> Result<(), RegionError> {
    for prior in out.iter() {
        b.charge(
            Resource::Work,
            (prior.identity().source.0.len() + source.identity().source.0.len()) as u64 + 42,
        )?;
        if prior.identity() == source.identity() {
            return Ok(());
        }
    }
    push(out, source, b)
}
fn sort_sources(out: &mut [SourceSnapshot], b: &mut Budget) -> Result<(), RegionError> {
    for i in 1..out.len() {
        let mut j = i;
        while j > 0 {
            b.charge(
                Resource::Work,
                (out[j - 1].identity().source.0.len() + out[j].identity().source.0.len()) as u64
                    + 42,
            )?;
            if out[j - 1].identity() <= out[j].identity() {
                break;
            }
            out.swap(j - 1, j);
            j -= 1;
        }
    }
    Ok(())
}
