//! Same-execution Reference and built-in Entity birth evidence.
use super::birth::{BirthAccessError, BirthLookup, EntityBirth};
use super::*;

pub struct NamedTrace<'a, 'p> {
    references: ReferenceTrace<'a, 'p>,
    births: Vec<EntityBirth>,
}
impl<'a, 'p> NamedTrace<'a, 'p> {
    pub fn references(&self) -> &ReferenceTrace<'a, 'p> {
        &self.references
    }
    /// Raw publication metadata, including accurate partial execution prefixes.
    pub fn births(&self) -> &[EntityBirth] {
        &self.births
    }
    pub fn entity_birth(
        &self,
        id: EntityId,
        b: &mut Budget,
    ) -> Result<BirthLookup<'_>, BirthAccessError> {
        if b.limits() != self.references.limits {
            return Err(BirthAccessError::LimitsMismatch);
        }
        b.charge(Resource::Work, 1)?;
        let BindingOutcome::Complete(analysis) = &self.references.reply.outcome else {
            return Err(BirthAccessError::Incomplete);
        };
        birth::lookup(analysis.facts(), &self.births, id, b)
    }
    pub(crate) fn into_parts(self) -> (ReferenceTrace<'a, 'p>, Vec<EntityBirth>) {
        (self.references, self.births)
    }
}
pub fn analyze<'a, 'p>(
    analysis_id: &str,
    tree: &'a ValidatedParseTree<'_>,
    profile: &'a ResolvedParseProfile<'p>,
    host: Option<&mut dyn BindingHost>,
    budget: &mut Budget,
    admission: &mut SourceAdmission,
) -> NamedTrace<'a, 'p> {
    let (reply, rows) = super::super::analyze_inner(
        analysis_id,
        tree,
        profile,
        host,
        super::super::CaptureMode::Named,
        budget,
        admission,
    );
    NamedTrace {
        references: ReferenceTrace {
            tree: tree.tree(),
            profile,
            reply,
            rows: rows.references,
            limits: budget.limits(),
        },
        births: rows.births,
    }
}
