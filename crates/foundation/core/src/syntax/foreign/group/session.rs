//! Receiving-operation reuse of immutable shared owner checks.
use super::*;
use crate::source::SourceAdmissionScope;

/// One receiving operation's shared owner validation. Keep the same cumulative
/// Budget throughout its lifetime, including nested trusted codec callbacks;
/// replacing/resetting that Budget violates the operation contract. This is
/// not a portable proof or a cache to retain across receiving operations.
/// Guest syntax and association are always checked afresh. Successful owner
/// checks retain only relative depth, replayed at the actual caller depth.
pub struct ForeignGroupValidation<'g, 'r> {
    group: &'g ForeignClosureGroup,
    registry: &'r SchemaRegistry,
    provenance: Vec<Option<u64>>,
    environment: Vec<Option<u64>>,
    admission: Option<SourceAdmissionScope>,
}
impl<'g, 'r> ForeignGroupValidation<'g, 'r> {
    pub fn new(
        group: &'g ForeignClosureGroup,
        registry: &'r SchemaRegistry,
        b: &mut Budget,
    ) -> Result<Self, GroupError> {
        b.poll()?;
        Ok(Self {
            group,
            registry,
            provenance: scratch(group.provenances.len(), None, b)?,
            environment: scratch(group.environments.len(), None, b)?,
            admission: None,
        })
    }

    pub fn group(&self) -> &'g ForeignClosureGroup {
        self.group
    }

    pub fn uses_registry(&self, registry: &SchemaRegistry) -> bool {
        core::ptr::eq(self.registry, registry)
    }

    /// Check the selected closure, without certifying unselected group entries.
    /// Whole-group/containing-language validation remains the caller's task.
    pub fn validate_entry(
        &mut self,
        number: usize,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<ValidatedSyntaxBundle<'g>, SyntaxError> {
        b.poll()?;
        b.charge(Resource::Work, 3)?;
        let view = self.group.entry(number).ok_or(SyntaxError::Reference)?;
        let bundle = validate_syntax(view.syntax, self.registry, b, admission)?;
        association(view.syntax, view.environment)?;
        if self
            .admission
            .as_ref()
            .is_none_or(|scope| !admission.matches_scope(scope))
        {
            b.charge(
                Resource::Work,
                (self.provenance.len() + self.environment.len()) as u64,
            )?;
            self.provenance.fill(None);
            self.environment.fill(None);
            self.admission = admission.scope_with_budget(b)?;
        }
        let entry = &self.group.entries[number];
        let owner = entry.owner.0 as usize;
        let provenance = self.group.environments[owner].provenance.0 as usize;
        let mut checked_provenance = None;
        match self.provenance[provenance] {
            Some(depth) => b.observe_depth(depth)?,
            None => {
                let (_, depth) = b.measure_depth(|b| {
                    validate_provenance(view.origins, view.sources, view.source_maps, b, admission)
                })?;
                if self.admission.is_some() {
                    checked_provenance = Some(depth);
                }
            }
        }
        match self.environment[owner] {
            Some(depth) => b.observe_depth(depth)?,
            None => {
                let (_, depth) = b.measure_depth(|b| {
                    environment(
                        &view.environment.value,
                        view.origins.len(),
                        self.registry,
                        b,
                    )
                })?;
                if self.admission.is_some() {
                    self.environment[owner] = Some(depth);
                }
            }
        }
        b.poll()?;
        if let Some(depth) = checked_provenance {
            self.provenance[provenance] = Some(depth);
        }
        Ok(bundle)
    }
}
