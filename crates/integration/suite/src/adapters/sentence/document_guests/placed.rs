//! Select document fragments across explicitly placed Sentence owners. This
//! preserves separate source owners and display occurrences; it does not grant
//! an Article namespace or recursively import arbitrary foreign definitions.
use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OwnerId(pub usize);

/// One display occurrence. `depth` is relative to the caller's current depth,
/// not an absolute ceiling or a proof that the caller supplied its real path.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Placement {
    pub owner: OwnerId,
    pub depth: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DocumentKey {
    pub owner: OwnerId,
    pub document: DocumentId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlacedOccurrence {
    pub placement: usize,
    pub embed: EmbedRef,
    pub document: DocumentKey,
}

pub struct PlacedSelection<'a> {
    inputs: &'a [SentenceSyntax],
    placements: &'a [Placement],
    selected: Vec<Selection>,
    occurrences: Vec<PlacedOccurrence>,
}
impl<'a> PlacedSelection<'a> {
    /// Transfer owner-local selections and placement correspondences together.
    pub fn into_parts(
        self,
    ) -> (
        &'a [SentenceSyntax],
        &'a [Placement],
        Vec<Selection>,
        Vec<PlacedOccurrence>,
    ) {
        (
            self.inputs,
            self.placements,
            self.selected,
            self.occurrences,
        )
    }

    pub fn placements(&self) -> &'a [Placement] {
        self.placements
    }

    pub fn input(&self, owner: OwnerId) -> Option<&'a SentenceSyntax> {
        self.inputs.get(owner.0)
    }
    /// Owner-local document tables retain sharing without merging unrelated
    /// Sentence owners solely because their text or embed indices are equal.
    pub fn selections(&self) -> &[Selection] {
        &self.selected
    }
    pub fn document(&self, key: DocumentKey) -> Option<&DocumentSyntax> {
        self.selected
            .get(key.owner.0)?
            .documents
            .get(key.document.index())
    }
    pub fn occurrences(&self) -> &[PlacedOccurrence] {
        &self.occurrences
    }
}

#[derive(Debug)]
pub enum PlacedError<E> {
    Stopped(StopReason),
    MissingOwner(OwnerId),
    UnusedOwner(OwnerId),
    Collection { owner: OwnerId, error: Error<E> },
}
impl<E> From<StopReason> for PlacedError<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Validate each explicitly supplied owner once at its deepest placement, then
/// preserve all selected fragment occurrences in placement/child order. Every
/// owner must be placed. Local Sentence validation includes unselected guests;
/// selection still requires an exact Doc surface identity and Inline category.
/// The caller must perform namespace resolution and output validation afterward.
pub fn collect_placed<'a, C: FoundationValueCodec>(
    inputs: &'a [SentenceSyntax],
    placements: &'a [Placement],
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<PlacedSelection<'a>, PlacedError<C::Error>> {
    b.poll()?;
    let result = collect_inner_placed(inputs, placements, surface, registry, codec, b);
    b.poll()?;
    result
}

fn collect_inner_placed<'a, C: FoundationValueCodec>(
    inputs: &'a [SentenceSyntax],
    placements: &'a [Placement],
    surface: &SchemaRef,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<PlacedSelection<'a>, PlacedError<C::Error>> {
    let mut depths: Vec<Option<u64>> = Vec::new();
    b.charge(Resource::Work, inputs.len() as u64)?;
    b.charge(
        Resource::AllocationUnits,
        (inputs.len() as u64).saturating_mul(core::mem::size_of::<Option<u64>>() as u64),
    )?;
    depths
        .try_reserve_exact(inputs.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    depths.resize(inputs.len(), None);
    let base = b.current_depth();
    for placement in placements {
        b.charge(Resource::Work, 1)?;
        let depth = depths
            .get_mut(placement.owner.0)
            .ok_or(PlacedError::MissingOwner(placement.owner))?;
        let absolute = base
            .checked_add(placement.depth)
            .ok_or_else(|| b.stop(StopReason::DepthLimit))?;
        *depth = Some(depth.map_or(absolute, |old| old.max(absolute)));
    }
    // Reject incomplete ownership selection before invoking any guest lowering.
    for (owner, depth) in depths.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        if depth.is_none() {
            return Err(PlacedError::UnusedOwner(OwnerId(owner)));
        }
    }
    let mut selected = Vec::new();
    for (owner, (input, depth)) in inputs.iter().zip(depths).enumerate() {
        b.charge(Resource::Work, 1)?;
        let owner = OwnerId(owner);
        let depth = depth.ok_or(PlacedError::UnusedOwner(owner))?;
        let selection = b.with_depth_at_least(depth, |b| {
            collect(input, surface, registry, codec, b).map_err(|error| match error {
                Error::Stopped(reason) => PlacedError::Stopped(reason),
                error => PlacedError::Collection { owner, error },
            })
        })?;
        push(&mut selected, selection, b)?;
    }
    let mut occurrences = Vec::new();
    for (placement_index, placement) in placements.iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let selection = selected
            .get(placement.owner.0)
            .ok_or(PlacedError::MissingOwner(placement.owner))?;
        for occurrence in selection.occurrences() {
            push(
                &mut occurrences,
                PlacedOccurrence {
                    placement: placement_index,
                    embed: occurrence.embed,
                    document: DocumentKey {
                        owner: placement.owner,
                        document: occurrence.document,
                    },
                },
                b,
            )?;
        }
    }
    Ok(PlacedSelection {
        inputs,
        placements,
        selected,
        occurrences,
    })
}
