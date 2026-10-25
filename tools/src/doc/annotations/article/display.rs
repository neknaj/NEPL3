//! Occurrence-role planning for a retained Article selection. This is not asset
//! resolution, automatic namespace adoption, or a complete-output proof.
use super::{Selection, namespace::SentenceOccurrenceId};
use nepl3_core::budget::{Budget, Resource, StopReason};
use nepl3_doc_core::check::ShapeError;
use nepl3_doc_html::{ParallelMode, RenderOptions, visibility};

#[derive(Debug)]
pub enum Error {
    Stopped(StopReason),
    Shape(ShapeError),
    Visibility(visibility::Error),
    Selection,
}
impl From<StopReason> for Error {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Borrowing the exact immutable selection and options keeps numeric occurrence
/// IDs contextual. Owners, including alt-only and hidden owners, remain retained.
pub struct Plan<'s, 'a> {
    selection: &'s Selection<'a>,
    options: &'s RenderOptions,
    adopted: Vec<SentenceOccurrenceId>,
    visible: Option<Vec<SentenceOccurrenceId>>,
}
impl<'s, 'a> Plan<'s, 'a> {
    pub fn selection(&self) -> &'s Selection<'a> {
        self.selection
    }
    pub fn options(&self) -> &'s RenderOptions {
        self.options
    }
    /// All non-alt occurrences, including hidden language variants. These are
    /// proposed imports; namespace resolution must still validate their labels.
    pub fn adoption(&self) -> &[SentenceOccurrenceId] {
        &self.adopted
    }
    /// Actual requested display order, retaining original SentenceOccurrenceIds.
    pub fn insertion(&self) -> &[SentenceOccurrenceId] {
        self.visible.as_deref().unwrap_or(&self.adopted)
    }
}
fn match_occurrences(
    input: &Selection<'_>,
    occurrences: &[visibility::SentenceOccurrence],
    b: &mut Budget,
) -> Result<Vec<SentenceOccurrenceId>, Error> {
    let mut output = Vec::new();
    let mut next = 0;
    for (index, sentence) in input.sentences().iter().enumerate() {
        b.charge(Resource::Work, 1)?;
        let Some(wanted) = occurrences.get(next) else {
            break;
        };
        if wanted.structural_index != sentence.occurrence as u64 {
            continue;
        }
        let original = input
            .structure()
            .get(sentence.occurrence)
            .ok_or(Error::Selection)?;
        let owner = input
            .owners()
            .get(sentence.owner.0)
            .ok_or(Error::Selection)?;
        if wanted.node != original.node
            || wanted.depth != original.depth
            || wanted.syntax != owner.embed()
        {
            return Err(Error::Selection);
        }
        b.charge(
            Resource::AllocationUnits,
            2 * core::mem::size_of::<SentenceOccurrenceId>() as u64,
        )?;
        output
            .try_reserve(1)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        output.push(SentenceOccurrenceId(index));
        next += 1;
    }
    if next != occurrences.len() {
        return Err(Error::Selection);
    }
    Ok(output)
}
/// Plan namespace adoption independently of display insertion. Alt occurrences
/// are excluded by structural role, never by owner/EmbedRef: an owner shared
/// with title, body or caption remains selected at those non-alt occurrences.
/// This operation does not lower owners again or mutate the structural selection.
/// Consumers must use both lists with this same selection, resolve the proposed
/// namespace and check complete insertion; filtering is not permission to skip
/// required members, invalid sources or final HTML validation.
pub fn plan<'s, 'a>(
    input: &'s Selection<'a>,
    options: &'s RenderOptions,
    b: &mut Budget,
) -> Result<Plan<'s, 'a>, Error> {
    b.poll()?;
    let shape = input
        .document()
        .value
        .validate_shape(b)
        .map_err(|error| match error {
            ShapeError::Stopped(reason) => Error::Stopped(reason),
            error => Error::Shape(error),
        })?;
    let all = visibility::sentence_occurrences(
        &shape,
        &RenderOptions {
            parallel: ParallelMode::Rows,
        },
        b,
    )
    .map_err(|error| match error {
        visibility::Error::Stopped(reason) => Error::Stopped(reason),
        error => Error::Visibility(error),
    })?;
    let adopted = match_occurrences(input, &all, b)?;
    let visible = if matches!(options.parallel, ParallelMode::Single { .. }) {
        let selected =
            visibility::sentence_occurrences(&shape, options, b).map_err(|error| match error {
                visibility::Error::Stopped(reason) => Error::Stopped(reason),
                error => Error::Visibility(error),
            })?;
        Some(match_occurrences(input, &selected, b)?)
    } else {
        None
    };
    b.poll()?;
    Ok(Plan {
        selection: input,
        options,
        adopted,
        visible,
    })
}
