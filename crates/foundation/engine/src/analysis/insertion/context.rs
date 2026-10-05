//! Compare retained original parser inputs with explicitly supplied native inputs.
//! This does not authenticate host currentness or establish editing authority.
use super::quality::StrictNameInsertion;
use crate::{
    parse::{ParseEnvironmentSet, ParseRequest},
    profile::ResolvedParseProfile,
};
use nepl3_core::budget::{Budget, Resource, StopReason};

/// Native inputs selected by the host, not a declaration of external freshness.
pub struct ParseContext<'a> {
    pub profile: &'a ResolvedParseProfile<'a>,
    pub environments: &'a ParseEnvironmentSet<'a>,
    pub request: ParseRequest<'a>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextDifference {
    ProfileInstance,
    EnvironmentInstance,
    Snapshot,
    Range,
    FinalInput,
    Entry,
    ReaderStateCount,
    ReaderState { index: usize },
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ContextError {
    LimitsMismatch,
    Stopped(StopReason),
}
impl From<StopReason> for ContextError {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}
/// Compare the original (pre-edit) seed, not the intentionally changed candidate.
/// `None` means equality of these explicit inputs at this call. Profile and
/// environment comparisons require the same immutable native instances; an
/// independently rebuilt, semantically equivalent instance is conservatively
/// different. Request comparisons include full snapshot metadata/content,
/// entry, bounds, final-input flag and ordered reader states.
///
/// This does not cover dynamically acquired source dependencies, provider host
/// configuration, authority or external workspace state, and is not permission
/// to apply an edit. No proof survives a later mutation of host-owned inputs.
/// Continue the operation's cumulative Budget; equal Limits alone do not prove
/// continuity. Inputs are not cloned; reader-state comparison charges its temporary traversal
/// storage. Original evidence is unchanged.
pub fn compare_original(
    strict: &StrictNameInsertion<'_, '_, '_, '_>,
    supplied: &ParseContext<'_>,
    b: &mut Budget,
) -> Result<Option<ContextDifference>, ContextError> {
    let checked = strict.declaration().reference().insertion().checked();
    if b.limits() != checked.limits() {
        return Err(ContextError::LimitsMismatch);
    }
    b.poll()?;
    let original = checked.original().seed();
    b.charge(Resource::Work, 1)?;
    if !core::ptr::eq(original.profile(), supplied.profile) {
        return Ok(Some(ContextDifference::ProfileInstance));
    }
    b.charge(Resource::Work, 1)?;
    if !core::ptr::eq(original.environments(), supplied.environments) {
        return Ok(Some(ContextDifference::EnvironmentInstance));
    }
    let a = original.request();
    let c = &supplied.request;
    if !a.snapshot.eq_with_budget(c.snapshot, b)? {
        return Ok(Some(ContextDifference::Snapshot));
    }
    b.charge(Resource::Work, 1)?;
    if a.start != c.start || a.limit != c.limit {
        return Ok(Some(ContextDifference::Range));
    }
    b.charge(Resource::Work, 1)?;
    if a.final_input != c.final_input {
        return Ok(Some(ContextDifference::FinalInput));
    }
    if !crate::selection::entry_equal(a.entry, c.entry, b)? {
        return Ok(Some(ContextDifference::Entry));
    }
    b.charge(Resource::Work, 1)?;
    if a.states.len() != c.states.len() {
        return Ok(Some(ContextDifference::ReaderStateCount));
    }
    for (index, (a, c)) in a.states.iter().zip(c.states).enumerate() {
        b.charge(
            Resource::Work,
            (a.alias.len() as u64)
                .saturating_add(c.alias.len() as u64)
                .saturating_add(1),
        )?;
        if a.alias != c.alias || !a.state.equal_with_budget(&c.state, b)? {
            return Ok(Some(ContextDifference::ReaderState { index }));
        }
    }
    Ok(None)
}
