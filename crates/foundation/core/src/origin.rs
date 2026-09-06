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
}
impl OriginGraph {
    /// External tables may use arbitrary ordering; all references and cycles are checked before acceptance.
    pub fn from_origins(
        origins: Vec<Origin>,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<Self, OriginError> {
        Self::validate_origins(&origins, sources, budget)?;
        Ok(Self { origins })
    }
    pub fn validate_origins(
        origins: &[Origin],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), OriginError> {
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
        Ok(())
    }
    /// Append-only native construction accepts only existing parents, so cannot introduce cycles.
    pub fn push(
        &mut self,
        origin: Origin,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<OriginId, OriginError> {
        validate_spans(&origin, sources)?;
        for parent in parents(&origin) {
            if parent.0 >= self.origins.len() as u64 {
                return Err(OriginError::Reference);
            }
        }
        let mut height = 1u64;
        for parent in parents(&origin) {
            height = height.max(
                self.height(*parent, budget)?
                    .checked_add(1)
                    .ok_or(StopReason::DepthLimit)?,
            );
        }
        budget.observe_depth(height)?;
        budget.charge(Resource::Nodes, 1)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Origin>() as u64,
        )?;
        let id = OriginId(self.origins.len() as u64);
        self.origins.push(origin);
        Ok(id)
    }
    fn height(&self, id: OriginId, budget: &mut Budget) -> Result<u64, OriginError> {
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<(OriginId, u64)>() as u64,
        )?;
        let mut pending = alloc::vec![(id, 1u64)];
        let mut height = 0;
        while let Some((id, depth)) = pending.pop() {
            budget.charge(Resource::Work, 1)?;
            budget.observe_depth(depth)?;
            height = height.max(depth);
            for parent in parents(self.get(id).ok_or(OriginError::Reference)?) {
                budget.charge(
                    Resource::AllocationUnits,
                    core::mem::size_of::<(OriginId, u64)>() as u64,
                )?;
                pending.push((*parent, depth.checked_add(1).ok_or(StopReason::DepthLimit)?));
            }
        }
        Ok(height)
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
}
impl SourceMap {
    /// Borrows a mapping table whose entries were admitted through `insert`.
    pub fn validated(&self) -> ValidatedSourceMap<'_> {
        ValidatedSourceMap {
            mappings: &self.mappings,
        }
    }
    pub fn validate_mappings<'a>(
        mappings: &'a [Mapping],
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<ValidatedSourceMap<'a>, OriginError> {
        budget.charge(Resource::Work, 1)?;
        for mapping in mappings {
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
        check_map_cycles(mappings.iter(), budget)?;
        Ok(ValidatedSourceMap { mappings })
    }
}
impl ValidatedSourceMap<'_> {
    /// Rechecks the declaration closure when this proof is reused with another source store.
    pub fn validate_sources(
        &self,
        sources: &SourceStore,
        budget: &mut Budget,
    ) -> Result<(), OriginError> {
        budget.charge(Resource::Work, 1)?;
        for mapping in self.mappings {
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
                for mapping in self.mappings {
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
) -> Result<(), OriginError> {
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
    Ok(())
}
