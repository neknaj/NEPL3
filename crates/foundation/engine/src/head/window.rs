use super::{HeadError, ProjectedHead, ProjectedSpan, ProjectedToken, SourceWindow};
use nepl3_core::{
    budget::{Budget, Resource},
    source::{SourceAdmission, SourceSnapshot, Span},
    view::Token,
};
impl ProjectedSpan {
    pub fn capture(span: &Span, budget: &mut Budget) -> Result<Self, HeadError> {
        let id = span.snapshot_ref();
        budget.charge(Resource::Work, id.source.0.len() as u64 + 34)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Self>() as u64 + id.source.0.len() as u64,
        )?;
        Ok(Self {
            source: nepl3_core::source::SourceRef {
                source_id: id.source.clone(),
                revision: id.revision,
                digest: id.digest,
            },
            start: span.start(),
            end: span.end(),
        })
    }
    pub(crate) fn same_snapshot(
        &self,
        other: &Self,
        budget: &mut Budget,
    ) -> Result<bool, HeadError> {
        budget.charge(
            Resource::Work,
            (self.source.source_id.0.len() as u64)
                .saturating_add(other.source.source_id.0.len() as u64)
                .saturating_add(34),
        )?;
        Ok(self.source == other.source)
    }
}
impl SourceWindow {
    /// Host construction proves the bytes came from this exact snapshot/range.
    pub fn capture(
        source: &SourceSnapshot,
        start: u64,
        end: u64,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, HeadError> {
        source.check_range(start, end)?;
        admission.admit_existing(source, budget)?;
        let text = source.slice_range(start, end)?;
        budget.charge(Resource::Work, text.len() as u64)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Self>() as u64
                + source.identity().source.0.len() as u64
                + text.len() as u64,
        )?;
        Ok(Self {
            span: ProjectedSpan {
                source: source.reference(),
                start,
                end,
            },
            bytes: text.as_bytes().into(),
        })
    }
    /// Receiving-side shape check. It does not claim to authenticate the full
    /// snapshot hash or recover bytes outside the supplied range.
    pub fn validate(&self, budget: &mut Budget) -> Result<(), HeadError> {
        budget.charge(
            Resource::Work,
            self.span.source.source_id.0.len() as u64 + self.bytes.len() as u64 + 34,
        )?;
        if self.span.source.source_id.0.is_empty()
            || self.span.end.checked_sub(self.span.start) != Some(self.bytes.len() as u64)
            || core::str::from_utf8(&self.bytes).is_err()
        {
            return Err(HeadError::Projection);
        }
        Ok(())
    }
    /// Resolves a claimed range using only this window. No full source snapshot
    /// is required, and positions outside the window grant no lookup authority.
    pub fn slice<'a>(
        &'a self,
        range: &ProjectedSpan,
        budget: &mut Budget,
    ) -> Result<Option<&'a str>, HeadError> {
        if !self.span.same_snapshot(range, budget)?
            || range.start < self.span.start
            || range.end > self.span.end
        {
            return Ok(None);
        }
        let start = range
            .start
            .checked_sub(self.span.start)
            .ok_or(HeadError::Projection)?;
        let end = range
            .end
            .checked_sub(self.span.start)
            .ok_or(HeadError::Projection)?;
        budget.charge(Resource::Work, self.bytes.len() as u64)?;
        let text = core::str::from_utf8(&self.bytes).map_err(|_| HeadError::Projection)?;
        let start = usize::try_from(start).map_err(|_| HeadError::Projection)?;
        let end = usize::try_from(end).map_err(|_| HeadError::Projection)?;
        text.get(start..end).map(Some).ok_or(HeadError::Projection)
    }
}
impl ProjectedHead {
    pub fn capture(
        token: &Token,
        source: &SourceSnapshot,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Self, HeadError> {
        budget.charge(
            Resource::Work,
            token.head.snapshot_ref().source.0.len() as u64 + 33,
        )?;
        source.slice(&token.head)?;
        let window = SourceWindow::capture(
            source,
            token.head.start(),
            token.head.end(),
            budget,
            admission,
        )?;
        Ok(Self {
            token: ProjectedToken::capture(token, source, budget)?,
            window,
        })
    }
}
impl ProjectedToken {
    pub(crate) fn capture(
        token: &Token,
        source: &SourceSnapshot,
        budget: &mut Budget,
    ) -> Result<Self, HeadError> {
        budget.charge(
            Resource::Work,
            (token.head.snapshot_ref().source.0.len() + token.kind.schema.package.len()) as u64
                + 33,
        )?;
        source.slice(&token.head)?;
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Self>() as u64 + token.kind.schema.package.len() as u64,
        )?;
        let payload = token.payload.clone_with_budget(budget)?;
        Ok(Self {
            kind: token.kind.clone(),
            head: ProjectedSpan::capture(&token.head, budget)?,
            payload,
        })
    }
}
