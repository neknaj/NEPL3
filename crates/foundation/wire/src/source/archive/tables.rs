//! Shared typed source-field mechanics for opt-in syntax/group archive codecs.
use super::*;

#[derive(Default)]
pub(crate) struct Capture {
    union: SourceStore,
    memberships: Vec<Vec<SourceRef>>,
}
impl Capture {
    pub(crate) fn take(
        &mut self,
        sources: &[SourceSnapshot],
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let ordered = ordered_sources(sources, b)?;
        let mut members = Vec::new();
        for (reference, source) in ordered {
            admission.admit_existing(source, b)?;
            self.union.insert_ref_with_budget(source, b)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<SourceRef>() as u64 + reference.source.0.len() as u64,
            )?;
            members.push(source.reference());
        }
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Vec<SourceRef>>() as u64,
        )?;
        self.memberships.push(members);
        Ok(NdfValue::List(Vec::new()))
    }
    pub(crate) fn finish(
        self,
        source_schema: &SchemaRef,
        foundation: &SchemaRef,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<NdfValue, WireError> {
        let ordered = ordered_sources(self.union.snapshots(), b)?;
        let mut sources = Vec::new();
        for (_, source) in ordered {
            sources.push(source.clone_with_budget(b)?);
        }
        to_value(
            &sources,
            &self.memberships,
            source_schema,
            foundation,
            admission,
            b,
        )
    }
}

pub(crate) struct Restore {
    members: alloc::vec::IntoIter<Vec<SourceSnapshot>>,
}
impl Restore {
    pub(crate) fn new(
        value: &NdfValue,
        source_schema: &SchemaRef,
        foundation: &SchemaRef,
        admission: &mut SourceAdmission,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        Ok(Self {
            members: from_value(value, source_schema, foundation, admission, b)?.into_iter(),
        })
    }
    pub(crate) fn take(
        &mut self,
        value: &NdfValue,
        b: &mut Budget,
    ) -> Result<(Vec<SourceSnapshot>, SourceStore), WireError> {
        b.charge(Resource::Work, 1)?;
        if !list(value)?.is_empty() {
            return Err(WireError::NonCanonical);
        }
        let snapshots = self.members.next().ok_or(WireError::InvalidLength)?;
        let mut local = SourceStore::default();
        for source in &snapshots {
            local.insert_ref_with_budget(source, b)?;
        }
        Ok((snapshots, local))
    }
    pub(crate) fn finish(mut self, b: &mut Budget) -> Result<(), WireError> {
        if self.members.next().is_some() {
            return Err(WireError::InvalidLength);
        }
        b.poll()?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
