//! Provenance DAGs and conservative reversible source mappings.
use crate::{
    budget::{Budget, Resource, StopReason},
    source::{SnapshotId, SourceError, SourceStore, Span},
    value::OperationRef,
};
use alloc::{collections::BTreeMap, string::String, vec::Vec};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginId(pub u64);
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Origin {
    Direct(Span),
    Composite(Vec<OriginId>),
    Generated {
        operation: OperationRef,
        callsite: Option<Span>,
        inputs: Vec<OriginId>,
    },
    Synthetic {
        reason: String,
        anchor: Option<Span>,
    },
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OriginError {
    Stopped(StopReason),
    Source(SourceError),
    Reference,
    Cycle,
    EmptyReason,
    Ambiguous,
    Irreversible,
    Unmapped,
}
impl From<StopReason> for OriginError {
    fn from(e: StopReason) -> Self {
        Self::Stopped(e)
    }
}
impl From<SourceError> for OriginError {
    fn from(e: SourceError) -> Self {
        Self::Source(e)
    }
}
#[derive(Debug, Default)]
pub struct OriginGraph {
    origins: Vec<Origin>,
    // Immutable after insertion; indices share the origins table's ID space.
    heights: Vec<u64>,
}
impl OriginGraph {
    /// External tables may use arbitrary ordering; all references and cycles are checked before acceptance.
    pub fn from_origins(
        origins: Vec<Origin>,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<Self, OriginError> {
        let (heights, _) = Self::checked_heights(&origins, sources, budget)?;
        Ok(Self { origins, heights })
    }
    pub fn validate_origins(
        origins: &[Origin],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), OriginError> {
        Self::checked_heights(origins, sources, budget).map(|_| ())
    }
    pub(crate) fn validation_depth(
        origins: &[Origin],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<u64, OriginError> {
        Self::checked_heights(origins, sources, budget).map(|(_, depth)| depth)
    }
    fn checked_heights(
        origins: &[Origin],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(Vec<u64>, u64), OriginError> {
        let mut maximum_depth = 0;
        budget.charge(Resource::AllocationUnits, origins.len() as u64)?;
        let mut state = alloc::vec![0u8; origins.len()];
        budget.charge(
            Resource::AllocationUnits,
            (origins.len() as u64).saturating_mul(8),
        )?;
        let mut heights = alloc::vec![0u64; origins.len()];
        for root in 0..origins.len() {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<(usize, bool, u64)>() as u64,
            )?;
            let mut stack = alloc::vec![(root, false, 1u64)];
            while let Some((index, exiting, depth)) = stack.pop() {
                budget.charge(Resource::Work, 1)?;
                budget.observe_depth(depth)?;
                if exiting {
                    let mut height = 1;
                    for parent in parents(&origins[index]) {
                        height = height.max(
                            heights[parent.0 as usize]
                                .checked_add(1)
                                .ok_or(StopReason::DepthLimit)?,
                        );
                    }
                    heights[index] = height;
                    maximum_depth = maximum_depth.max(height);
                    state[index] = 2;
                    continue;
                }
                match state[index] {
                    2 => {
                        budget.observe_depth(
                            depth
                                .checked_add(heights[index] - 1)
                                .ok_or(StopReason::DepthLimit)?,
                        )?;
                        continue;
                    }
                    1 => return Err(OriginError::Cycle),
                    _ => {}
                }
                budget.charge(Resource::Nodes, 1)?;
                let origin = &origins[index];
                validate_spans(origin, sources)?;
                state[index] = 1;
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<(usize, bool, u64)>() as u64,
                )?;
                stack.push((index, true, depth));
                for parent in parents(origin).iter().rev() {
                    let parent = usize::try_from(parent.0).map_err(|_| OriginError::Reference)?;
                    if parent >= origins.len() {
                        return Err(OriginError::Reference);
                    }
                    budget.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<(usize, bool, u64)>() as u64,
                    )?;
                    stack.push((
                        parent,
                        false,
                        depth.checked_add(1).ok_or(StopReason::DepthLimit)?,
                    ));
                }
            }
        }
        Ok((heights, maximum_depth))
    }
    /// Append-only native construction accepts only existing parents, so cannot introduce cycles.
    /// Parent height lookup is O(P) for P immediate parent edges, with no ancestor
    /// traversal. Appending storage is amortized O(1); cached heights use O(N)
    /// space for N origins. Source-span validation is additional work.
    pub fn push(
        &mut self,
        origin: Origin,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<OriginId, OriginError> {
        budget.poll()?;
        validate_spans(&origin, sources)?;
        let mut height = 1u64;
        for parent in parents(&origin) {
            budget.charge(Resource::Work, 1)?;
            let index = usize::try_from(parent.0).map_err(|_| OriginError::Reference)?;
            height = height.max(
                self.heights
                    .get(index)
                    .ok_or(OriginError::Reference)?
                    .checked_add(1)
                    .ok_or_else(|| budget.stop(StopReason::DepthLimit))?,
            );
        }
        budget.observe_depth(height)?;
        budget.charge(Resource::Nodes, 1)?;
        budget.charge(
            Resource::AllocationUnits,
            (core::mem::size_of::<Origin>() + core::mem::size_of::<u64>()) as u64,
        )?;
        self.origins
            .try_reserve(1)
            .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
        self.heights
            .try_reserve(1)
            .map_err(|_| budget.stop(StopReason::AllocationLimit))?;
        let id = OriginId(self.origins.len() as u64);
        self.origins.push(origin);
        self.heights.push(height);
        Ok(id)
    }
    pub fn get(&self, id: OriginId) -> Option<&Origin> {
        usize::try_from(id.0).ok().and_then(|i| self.origins.get(i))
    }
    pub fn origins(&self) -> &[Origin] {
        &self.origins
    }
}
fn parents(origin: &Origin) -> &[OriginId] {
    match origin {
        Origin::Composite(inputs) | Origin::Generated { inputs, .. } => inputs,
        _ => &[],
    }
}
fn validate_spans(origin: &Origin, sources: &SourceStore) -> Result<(), OriginError> {
    let span = match origin {
        Origin::Direct(span) => Some(span),
        Origin::Composite(_) => None,
        Origin::Generated {
            operation,
            callsite,
            ..
        } => {
            if operation.name.is_empty() {
                return Err(OriginError::EmptyReason);
            }
            callsite.as_ref()
        }
        Origin::Synthetic { reason, anchor } => {
            if reason.is_empty() {
                return Err(OriginError::EmptyReason);
            }
            anchor.as_ref()
        }
    };
    if let Some(span) = span {
        sources
            .get_ref(span.snapshot_ref())
            .ok_or(SourceError::MissingSnapshot)?
            .slice(span)?;
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingKind {
    Exact,
    Transformed,
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Mapping {
    pub source: Span,
    pub target: Span,
    pub kind: MappingKind,
}
#[derive(Debug, Default)]
pub struct SourceMap {
    mappings: Vec<Mapping>,
}
/// Borrowed proof of source geometry and an acyclic pointwise mapping relation.
pub struct ValidatedSourceMap<'a> {
    mappings: &'a [Mapping],
    additional: &'a [Mapping],
    validated_sources: Option<&'a SourceStore>,
}
impl SourceMap {
    /// Borrows a mapping table whose entries were admitted through `insert`.
    pub fn validated(&self) -> ValidatedSourceMap<'_> {
        ValidatedSourceMap {
            mappings: &self.mappings,
            additional: &[],
            validated_sources: None,
        }
    }
    pub fn validate_mappings<'a>(
        mappings: &'a [Mapping],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<ValidatedSourceMap<'a>, OriginError> {
        Self::validate_mapping_parts(mappings, &[], sources, budget)
    }
    /// Validate the full ordered union of two borrowed tables without cloning
    /// mappings or their source identities. Neither part is assumed valid:
    /// source geometry and cycles spanning both parts are checked together.
    pub fn validate_mapping_parts<'a>(
        mappings: &'a [Mapping],
        additional: &'a [Mapping],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<ValidatedSourceMap<'a>, OriginError> {
        Self::checked_mapping_parts(mappings, additional, sources, budget).map(|(proof, _)| proof)
    }
    pub(crate) fn validation_depth(
        mappings: &[Mapping],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<u64, OriginError> {
        Self::checked_mapping_parts(mappings, &[], sources, budget).map(|(_, depth)| depth)
    }
    pub(crate) fn checked_mapping_parts<'a>(
        mappings: &'a [Mapping],
        additional: &'a [Mapping],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(ValidatedSourceMap<'a>, u64), OriginError> {
        budget.charge(Resource::Work, 1)?;
        let mapped = ValidatedSourceMap {
            mappings,
            additional,
            validated_sources: None,
        };
        for mapping in mapped.iter() {
            budget.charge(Resource::Work, 1)?;
            let source = sources
                .get_ref(mapping.source.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?
                .slice(&mapping.source)?;
            let target = sources
                .get_ref(mapping.target.snapshot_ref())
                .ok_or(SourceError::MissingSnapshot)?
                .slice(&mapping.target)?;
            if mapping.kind == MappingKind::Exact {
                budget.charge(Resource::Work, source.len().min(target.len()) as u64)?;
                if source != target {
                    return Err(OriginError::Irreversible);
                }
            }
        }
        let depth = check_map_cycles(mapped.iter(), budget)?;
        Ok((mapped, depth))
    }
}
impl<'a> ValidatedSourceMap<'a> {
    /// Bind source closure validation to an immutable store borrow. The bound
    /// proof cannot outlive that store; unbound proofs retain revalidation.
    pub fn bind_sources(
        mut self,
        sources: &'a SourceStore,
        budget: &mut Budget,
    ) -> Result<Self, OriginError> {
        self.validate_sources(sources, budget)?;
        self.validated_sources = Some(sources);
        Ok(self)
    }
    fn iter(&self) -> impl Iterator<Item = &Mapping> + Clone {
        self.mappings.iter().chain(self.additional)
    }
    /// Rechecks the declaration closure when this proof is reused with another source store.
    pub fn validate_sources(
        &self,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), OriginError> {
        budget.charge(Resource::Work, 1)?;
        if self
            .validated_sources
            .is_some_and(|original| core::ptr::eq(original, sources))
        {
            return Ok(());
        }
        for mapping in self.iter() {
            for span in [&mapping.source, &mapping.target] {
                budget.charge(Resource::Work, 1)?;
                sources
                    .get_ref(span.snapshot_ref())
                    .ok_or(SourceError::MissingSnapshot)?
                    .slice(span)?;
            }
        }
        Ok(())
    }
    /// Every byte/anchor must trace back into `parent`; all incoming paths are required.
    /// Once a point reaches the parent, its earlier provenance is irrelevant to containment.
    pub fn contains(
        &self,
        parent: &Span,
        child: &Span,
        budget: &mut Budget,
    ) -> Result<bool, OriginError> {
        budget.charge(Resource::Work, 1)?;
        if parent.contains(child) {
            return Ok(true);
        }
        if self.direct_cover(parent, child, budget)? {
            budget.observe_depth(2)?;
            budget.charge(Resource::Nodes, 2)?;
            return Ok(true);
        }
        for offset in 0..point_count(child) {
            let mut pending = Vec::new();
            push_point(&mut pending, point(child, offset), false, 1, budget)?;
            while let Some((current, _, depth)) = pending.pop() {
                budget.charge(Resource::Work, 1)?;
                budget.charge(Resource::Nodes, 1)?;
                budget.observe_depth(depth)?;
                if point_within(current, parent) {
                    continue;
                }
                let mut found = false;
                for mapping in self.iter() {
                    budget.charge(Resource::Work, 1)?;
                    if !point_on(current, &mapping.target) {
                        continue;
                    }
                    found = true;
                    let next_depth = depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
                    if mapping.kind == MappingKind::Exact {
                        push_point(
                            &mut pending,
                            point(&mapping.source, current.offset - mapping.target.start()),
                            false,
                            next_depth,
                            budget,
                        )?;
                    } else {
                        for offset in 0..point_count(&mapping.source) {
                            budget.charge(Resource::Work, 1)?;
                            push_point(
                                &mut pending,
                                point(&mapping.source, offset),
                                false,
                                next_depth,
                                budget,
                            )?;
                        }
                    }
                }
                if !found {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    /// Prove one whole range without allocating one traversal per byte. This
    /// succeeds only for a unique incoming mapping covering the entire child.
    /// Overlapping alternatives, holes and multi-hop paths use the pointwise
    /// validator, which still requires every reverse path to reach the parent.
    fn direct_cover(
        &self,
        parent: &Span,
        child: &Span,
        budget: &mut Budget,
    ) -> Result<bool, OriginError> {
        let anchor = child.start() == child.end();
        let mut candidate: Option<&Mapping> = None;
        for mapping in self.iter() {
            budget.charge(Resource::Work, 1)?;
            let target = &mapping.target;
            if target.snapshot_ref() != child.snapshot_ref()
                || (target.start() == target.end()) != anchor
                || if anchor {
                    target.start() != child.start()
                } else {
                    target.end() <= child.start() || child.end() <= target.start()
                }
            {
                continue;
            }
            if candidate.is_some() {
                return Ok(false);
            }
            candidate = Some(mapping);
        }
        let Some(mapping) = candidate else {
            return Ok(false);
        };
        if !mapping.target.contains(child) || mapping.source.snapshot_ref() != parent.snapshot_ref()
        {
            return Ok(false);
        }
        if mapping.kind == MappingKind::Transformed {
            return Ok(parent.contains(&mapping.source));
        }
        // Exact mappings have equal lengths, already checked by construction.
        // Translate only the requested subrange, without cloning a Span/ID.
        let start = mapping.source.start() + (child.start() - mapping.target.start());
        let end = mapping.source.start() + (child.end() - mapping.target.start());
        Ok(parent.start() <= start && end <= parent.end())
    }
}
fn point_on(point: Point<'_>, span: &Span) -> bool {
    point.snapshot == span.snapshot_ref()
        && point.anchor == (span.start() == span.end())
        && point.offset >= span.start()
        && point.offset - span.start() < point_count(span)
}
fn point_within(point: Point<'_>, span: &Span) -> bool {
    point.snapshot == span.snapshot_ref()
        && point.offset >= span.start()
        && if point.anchor {
            point.offset <= span.end()
        } else {
            point.offset < span.end()
        }
}
impl SourceMap {
    pub fn insert(
        &mut self,
        mapping: Mapping,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), OriginError> {
        let source = sources
            .get_ref(mapping.source.snapshot_ref())
            .ok_or(SourceError::MissingSnapshot)?
            .slice(&mapping.source)?;
        let target = sources
            .get_ref(mapping.target.snapshot_ref())
            .ok_or(SourceError::MissingSnapshot)?
            .slice(&mapping.target)?;
        if mapping.kind == MappingKind::Exact {
            budget.charge(Resource::Work, source.len().min(target.len()) as u64)?;
            if source != target {
                return Err(OriginError::Irreversible);
            }
        }
        check_map_cycles(
            self.mappings.iter().chain(core::iter::once(&mapping)),
            budget,
        )?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Mapping>() as u64,
        )?;
        self.mappings.push(mapping);
        Ok(())
    }
    /// Returns one direct inverse only when a unique exact mapping covers the requested range.
    /// Composite chains must be explicitly traversed and rechecked by the caller.
    pub fn inverse(&self, target: &Span, sources: &SourceStore) -> Result<Span, OriginError> {
        let mut found = None;
        for mapping in &self.mappings {
            if mapping.target.snapshot_ref() != target.snapshot_ref() {
                continue;
            }
            let overlaps = if target.start() == target.end() {
                mapping.target.start() <= target.start() && target.start() <= mapping.target.end()
            } else {
                mapping.target.start() < target.end() && target.start() < mapping.target.end()
            };
            if !overlaps {
                continue;
            }
            if found.is_some() {
                return Err(OriginError::Ambiguous);
            }
            found = Some(mapping);
        }
        let mapping = found.ok_or(OriginError::Unmapped)?;
        if mapping.kind != MappingKind::Exact || !mapping.target.contains(target) {
            return Err(OriginError::Irreversible);
        }
        let start = mapping
            .source
            .start()
            .checked_add(target.start() - mapping.target.start())
            .ok_or(OriginError::Irreversible)?;
        let end = mapping
            .source
            .start()
            .checked_add(target.end() - mapping.target.start())
            .ok_or(OriginError::Irreversible)?;
        Ok(sources
            .get_ref(mapping.source.snapshot_ref())
            .ok_or(SourceError::MissingSnapshot)?
            .span(start, end)?)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
struct Point<'a> {
    snapshot: &'a SnapshotId,
    offset: u64,
    anchor: bool,
}
fn point_count(span: &Span) -> u64 {
    (span.end() - span.start()).max(1)
}
fn point(span: &Span, offset: u64) -> Point<'_> {
    Point {
        snapshot: span.snapshot_ref(),
        offset: span.start() + offset,
        anchor: span.start() == span.end(),
    }
}
fn push_point<'a>(
    stack: &mut Vec<(Point<'a>, bool, u64)>,
    point: Point<'a>,
    exiting: bool,
    depth: u64,
    budget: &mut Budget,
) -> Result<(), OriginError> {
    budget.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<(Point, bool, u64)>() as u64,
    )?;
    stack.push((point, exiting, depth));
    Ok(())
}

fn check_map_cycles<'a>(
    input: impl Iterator<Item = &'a Mapping> + Clone,
    budget: &mut Budget,
) -> Result<u64, OriginError> {
    let (acyclic, mut maximum_depth) = snapshot_dag(input.clone(), budget)?;
    if acyclic {
        return Ok(maximum_depth);
    }
    // Exact edges preserve byte displacement. Transformed fragments relate every source
    // byte to every target byte. Empty fragments use a distinct insertion-anchor vertex.
    // This finite graph can be expensive; exhaustion returns Stopped, never a false cycle.
    let mappings = || input.clone();
    let mut state: BTreeMap<Point, u8> = BTreeMap::new();
    for mapping in mappings() {
        for offset in 0..point_count(&mapping.source) {
            budget.charge(Resource::Work, 1)?;
            let root = point(&mapping.source, offset);
            if state.get(&root) == Some(&2) {
                continue;
            }
            let mut stack = Vec::new();
            push_point(&mut stack, root, false, 1, budget)?;
            while let Some((current, exiting, depth)) = stack.pop() {
                budget.charge(Resource::Work, 1)?;
                budget.observe_depth(depth)?;
                maximum_depth = maximum_depth.max(depth);
                if exiting {
                    state.insert(current, 2);
                    continue;
                }
                match state.get(&current) {
                    Some(2) => continue,
                    Some(1) => return Err(OriginError::Cycle),
                    _ => {}
                }
                budget.charge(Resource::Nodes, 1)?;
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<(Point, u8)>() as u64,
                )?;
                state.insert(current, 1);
                push_point(&mut stack, current, true, depth, budget)?;
                for edge in mappings() {
                    budget.charge(Resource::Work, 1)?;
                    if current.snapshot != edge.source.snapshot_ref()
                        || current.anchor != (edge.source.start() == edge.source.end())
                    {
                        continue;
                    }
                    let Some(displacement) = current.offset.checked_sub(edge.source.start()) else {
                        continue;
                    };
                    if displacement >= point_count(&edge.source) {
                        continue;
                    }
                    let next_depth = depth.checked_add(1).ok_or(StopReason::DepthLimit)?;
                    if edge.kind == MappingKind::Exact {
                        push_point(
                            &mut stack,
                            point(&edge.target, displacement),
                            false,
                            next_depth,
                            budget,
                        )?;
                    } else {
                        for offset in 0..point_count(&edge.target) {
                            budget.charge(Resource::Work, 1)?;
                            push_point(
                                &mut stack,
                                point(&edge.target, offset),
                                false,
                                next_depth,
                                budget,
                            )?;
                        }
                    }
                }
            }
        }
    }
    Ok(maximum_depth)
}

/// A pointwise cycle implies a snapshot cycle. Proving the coarse graph acyclic
/// is sufficient, but a coarse cycle is inconclusive: retain the exact fallback
/// for disjoint ranges, self-snapshot shifts and insertion anchors.
fn snapshot_dag<'a>(
    input: impl Iterator<Item = &'a Mapping>,
    budget: &mut Budget,
) -> Result<(bool, u64), OriginError> {
    let mut nodes: Vec<&'a SnapshotId> = Vec::new();
    let mut ordered: Vec<usize> = Vec::new();
    // Linked adjacency lists retain duplicate edges without scanning unrelated
    // edges at each vertex. These are private indices, not source identities.
    let mut outgoing: Vec<Option<usize>> = Vec::new();
    let mut edges = Vec::new();
    // Successive fragments often share one endpoint. Reuse only its index;
    // compare the complete identity on every hit, within this one graph build.
    let mut recent: [Option<usize>; 2] = [None, None];
    // Position of the last ordered lookup, local to this graph construction.
    // Repeated endpoint hits do not move it. Any insertion replaces it with
    // the inserted position, so index shifts cannot leave a stale position.
    let mut ordered_hint: Option<usize> = None;
    for mapping in input {
        budget.charge(Resource::Work, 1)?;
        let mut ids = [0usize; 2];
        for (slot, span) in [&mapping.source, &mapping.target].into_iter().enumerate() {
            let identity = span.snapshot_ref();
            let mut found = None;
            if let Some(index) = recent[slot] {
                let prior = nodes[index];
                if prior.compare_with_budget(identity, budget)? == core::cmp::Ordering::Equal {
                    found = Some(index);
                }
            }
            let (mut low, mut high) = (0, ordered.len());
            if found.is_none()
                && let Some(at) = ordered_hint
            {
                let prior = nodes[ordered[at]];
                let neighbor = match prior.compare_with_budget(identity, budget)? {
                    core::cmp::Ordering::Equal => {
                        found = Some(ordered[at]);
                        None
                    }
                    core::cmp::Ordering::Less => {
                        low = at + 1;
                        (low < high).then_some(low)
                    }
                    core::cmp::Ordering::Greater => {
                        high = at;
                        high.checked_sub(1)
                    }
                };
                // Consecutive source fragments often occupy adjacent ordered
                // positions. Check that boundary before searching the rest;
                // the complete snapshot identity still decides every hit.
                if let Some(next) = neighbor {
                    let prior = nodes[ordered[next]];
                    match prior.compare_with_budget(identity, budget)? {
                        core::cmp::Ordering::Equal => {
                            found = Some(ordered[next]);
                            ordered_hint = Some(next);
                        }
                        core::cmp::Ordering::Less => low = next + 1,
                        core::cmp::Ordering::Greater => high = next,
                    }
                }
            }
            while found.is_none() && low < high {
                let mid = low + (high - low) / 2;
                let index = ordered[mid];
                let prior = nodes[index];
                match prior.compare_with_budget(identity, budget)? {
                    core::cmp::Ordering::Equal => {
                        found = Some(index);
                        ordered_hint = Some(mid);
                        break;
                    }
                    core::cmp::Ordering::Less => low = mid + 1,
                    core::cmp::Ordering::Greater => high = mid,
                }
            }
            ids[slot] = match found {
                Some(index) => index,
                None => {
                    budget.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<&SnapshotId>() as u64,
                    )?;
                    budget.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<usize>() as u64,
                    )?;
                    budget.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of::<Option<usize>>() as u64,
                    )?;
                    budget.charge(Resource::Work, (ordered.len() - low) as u64)?;
                    let index = nodes.len();
                    nodes.push(identity);
                    outgoing.push(None);
                    ordered.insert(low, index);
                    ordered_hint = Some(low);
                    index
                }
            };
            recent[slot] = Some(ids[slot]);
        }
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(usize, Option<usize>)>() as u64,
        )?;
        let edge = edges.len();
        edges.push((ids[1], outgoing[ids[0]]));
        outgoing[ids[0]] = Some(edge);
    }
    budget.charge(
        Resource::AllocationUnits,
        (nodes.len() as u64)
            .saturating_mul((core::mem::size_of::<usize>() + core::mem::size_of::<u64>()) as u64),
    )?;
    let mut incoming = alloc::vec![0usize;nodes.len()];
    let mut depth = alloc::vec![1u64;nodes.len()];
    for (target, _) in &edges {
        budget.charge(Resource::Work, 1)?;
        incoming[*target] = incoming[*target]
            .checked_add(1)
            .ok_or(StopReason::AllocationLimit)?;
    }
    let mut ready = Vec::new();
    for (index, count) in incoming.iter().enumerate() {
        budget.charge(Resource::Work, 1)?;
        if *count == 0 {
            budget.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<usize>() as u64,
            )?;
            ready.push(index);
        }
    }
    let mut visited = 0usize;
    let mut maximum_depth = 0;
    while let Some(current) = ready.pop() {
        budget.charge(Resource::Nodes, 1)?;
        budget.observe_depth(depth[current])?;
        maximum_depth = maximum_depth.max(depth[current]);
        visited += 1;
        let mut next = outgoing[current];
        while let Some(edge) = next {
            budget.charge(Resource::Work, 1)?;
            let (target, following) = edges[edge];
            next = following;
            incoming[target] -= 1;
            depth[target] = depth[target].max(
                depth[current]
                    .checked_add(1)
                    .ok_or(StopReason::DepthLimit)?,
            );
            if incoming[target] == 0 {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<usize>() as u64,
                )?;
                ready.push(target);
            }
        }
    }
    Ok((visited == nodes.len(), maximum_depth))
}
