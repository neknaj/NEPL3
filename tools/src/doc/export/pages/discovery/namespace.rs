//! Expand unique owners into ordered namespace occurrences before resolution.
use super::*;
use nepl3_core::source::SourceAdmission;
use nepl3_doc_core::labels::namespace as labels;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Parent {
    pub member: labels::MemberId,
    /// Index in the parent's Doc Sentence occurrence table.
    pub sentence: usize,
    /// Index in the parent's selected Sentence guest occurrence table.
    pub guest: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Occurrence {
    pub document: DocumentId,
    pub parent: Option<Parent>,
}
pub struct Plan<'c, 'd> {
    input: &'c Collected<'d>,
    inspected: Vec<labels::Member<'c>>,
    occurrences: Vec<Occurrence>,
}
impl<'c, 'd> Plan<'c, 'd> {
    pub fn input(&self) -> &'c Collected<'d> {
        self.input
    }
    pub fn occurrences(&self) -> &[Occurrence] {
        &self.occurrences
    }
    /// Repeated owners deliberately produce repeated member references. The
    /// namespace checker can therefore detect duplicate display identities.
    pub fn member_refs(&self, b: &mut Budget) -> Result<Vec<&labels::Member<'c>>, StopReason> {
        b.poll()?;
        let mut refs = Vec::new();
        for occurrence in &self.occurrences {
            push(&mut refs, &self.inspected[occurrence.document.index()], b)?;
        }
        Ok(refs)
    }
}
#[derive(Debug)]
pub enum Error<'a> {
    Stopped(StopReason),
    Member {
        document: DocumentId,
        error: labels::Error<'a>,
    },
}
impl From<StopReason> for Error<'_> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
// Guests are appended in slot order by collection. Binary bounds avoid scanning
// every guest for every structural Sentence occurrence.
fn bound(
    guests: &[Guest],
    slot: EmbedRef,
    upper: bool,
    b: &mut Budget,
) -> Result<usize, StopReason> {
    let (mut low, mut high) = (0, guests.len());
    while low < high {
        b.charge(Resource::Work, 1)?;
        let middle = low + (high - low) / 2;
        if guests[middle].slot.0 < slot.0 || (upper && guests[middle].slot == slot) {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    Ok(low)
}
/// Inspect each unique Doc once, then expand selected guest occurrences in
/// preorder with an explicit work stack. Rendering and name resolution follow
/// only after the complete occurrence list has been produced.
pub fn inspect<'c, 'd>(
    input: &'c Collected<'d>,
    registry: &SchemaRegistry,
    b: &mut Budget,
    admission: &mut SourceAdmission,
) -> Result<Plan<'c, 'd>, Error<'c>> {
    b.poll()?;
    let result = (|| {
        let mut inspected = Vec::new();
        let base = b.current_depth();
        let original_base = input.members[0].depth;
        for (index, member) in input.members.iter().enumerate() {
            let depth = base.saturating_add(member.depth.saturating_sub(original_base));
            let item = b.with_depth_at_least(depth, |b| {
                labels::inspect(member.document(), registry, b, admission).map_err(|error| {
                    Error::Member {
                        document: DocumentId(index),
                        error,
                    }
                })
            })?;
            push(&mut inspected, item, b)?;
        }
        let mut occurrences = Vec::new();
        let mut pending = Vec::new();
        push(
            &mut pending,
            (
                Occurrence {
                    document: DocumentId(0),
                    parent: None,
                },
                b.current_depth(),
            ),
            b,
        )?;
        while let Some((occurrence, depth)) = pending.pop() {
            b.with_depth_at_least::<_, Error<'c>>(depth.saturating_add(1), |b| {
                let parent = labels::MemberId(occurrences.len() as u64);
                let document = &input.members[occurrence.document.index()];
                push(&mut occurrences, occurrence, b)?;
                for (sentence, slot) in document.occurrences.iter().enumerate().rev() {
                    b.charge(Resource::Work, 1)?;
                    let start = bound(&document.guests, slot.embed, false, b)?;
                    let end = bound(&document.guests, slot.embed, true, b)?;
                    for guest in (start..end).rev() {
                        let item = &document.guests[guest];
                        let next_depth = depth
                            .saturating_add(slot.depth)
                            .saturating_add(item.occurrence.depth);
                        push(
                            &mut pending,
                            (
                                Occurrence {
                                    document: item.document,
                                    parent: Some(Parent {
                                        member: parent,
                                        sentence,
                                        guest,
                                    }),
                                },
                                next_depth,
                            ),
                            b,
                        )?;
                    }
                }
                Ok(())
            })?;
        }
        Ok(Plan {
            input,
            inspected,
            occurrences,
        })
    })();
    b.poll()?;
    result
}
