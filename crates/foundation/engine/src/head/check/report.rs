use super::*;
use nepl3_core::{
    diagnostic::validation::*,
    source::{Digest, SourceError},
};

impl HeadCall {
    /// Validates a projected reply using only the saved call's windows. This
    /// does not absorb Usage, restore full-snapshot Spans or authenticate a peer.
    pub fn validate_reply(
        &self,
        reply: &HeadReply,
        profile: &ResolvedParseProfile<'_>,
        budget: &mut Budget,
    ) -> Result<(), HeadError> {
        self.validate_projection(profile, budget)?;
        if reply.report.trace_overflow.is_some()
            && !matches!(reply.outcome, HeadOutcome::Stopped { .. })
        {
            return Err(HeadError::Report);
        }
        budget.charge(
            Resource::Work,
            (self.identity.session_id.len() as u64)
                .saturating_add(reply.identity.session_id.len() as u64)
                .saturating_add(self.identity.operation.name.len() as u64)
                .saturating_add(reply.identity.operation.name.len() as u64)
                .saturating_add(100),
        )?;
        if !schema_eq(
            &self.identity.operation.schema,
            &reply.identity.operation.schema,
            budget,
        )? || reply.identity != self.identity
        {
            return Err(HeadError::Identity);
        }
        match (&self.request, &reply.outcome) {
            (HeadRequest::Shape, HeadOutcome::Shape { shape }) => {
                if let Some(shape) = shape {
                    profile
                        .checked(&self.entry.alias, budget)
                        .map_err(profile_error)?
                        .validate_head_shape(shape, budget)
                        .map_err(package_error)?;
                }
            }
            (
                HeadRequest::ChildContext { shape, index, .. },
                HeadOutcome::ChildContext { context },
            ) => {
                profile
                    .validate_entry(context, budget)
                    .map_err(profile_error)?;
                let field = shape
                    .fields
                    .get(usize::try_from(*index).map_err(|_| HeadError::Shape)?)
                    .ok_or(HeadError::Shape)?;
                let expected = profile
                    .read_entry(&self.entry, field.read, budget)
                    .map_err(profile_error)?;
                budget.charge(
                    Resource::Work,
                    (context.alias.len()
                        + context.category.len()
                        + context.mode.len()
                        + context.package.schema.package.len()) as u64
                        + 100,
                )?;
                if (expected.read.is_some() && *context != expected.entry)
                    || (!expected.foreign && context.alias != self.entry.alias)
                {
                    return Err(HeadError::Context);
                }
            }
            (_, HeadOutcome::Failed { diagnostic }) => {
                // Charge every variable payload before equality. The Failed
                // diagnostic re-exposes an existing report member, never adds one.
                reply.charge_clone(budget)?;
                if !reply
                    .report
                    .diagnostics
                    .iter()
                    .any(|v| v == diagnostic.as_ref())
                {
                    return Err(HeadError::Report);
                }
            }
            (_, HeadOutcome::Stopped { .. }) => {}
            _ => return Err(HeadError::ReplyKind),
        }
        let registry = profile.registry();
        validate_report_usage(
            reply.report.usage,
            reply.report.diagnostics.len(),
            reply.report.events.len(),
            reply.report.trace_overflow.as_ref(),
        )?;
        for diagnostic in &reply.report.diagnostics {
            self.check_diagnostic(diagnostic, registry, budget)?;
        }
        for event in &reply.report.events {
            validate_event_metadata(&event.schema, &event.kind, &event.payload, registry, budget)?;
            if let Some(span) = &event.span {
                self.slice(span, budget)?;
            }
        }
        Ok(())
    }
    fn check_diagnostic(
        &self,
        value: &ProjectedDiagnostic,
        registry: &SchemaRegistry,
        budget: &mut Budget,
    ) -> Result<(), HeadError> {
        validate_diagnostic_metadata(
            &value.schema,
            &value.code,
            &value.stage,
            &value.arguments,
            registry,
            budget,
        )?;
        if let Some(span) = &value.primary {
            self.slice(span, budget)?;
        }
        for related in &value.related {
            registry.validate_typed(&related.arguments, budget)?;
            if let Some(span) = &related.span {
                self.slice(span, budget)?;
            }
        }
        for fix in &value.fixes {
            budget.charge(Resource::Work, 1)?;
            for (i, edit) in fix.edits.iter().enumerate() {
                let text = self.slice(&edit.span, budget)?;
                budget.charge(Resource::Work, text.len() as u64)?;
                if Digest::of(text.as_bytes()) != edit.expected_digest {
                    return Err(SourceError::ExpectedDigest.into());
                }
                for previous in &fix.edits[..i] {
                    let same = previous.span.same_snapshot(&edit.span, budget)?;
                    if previous.span.source.source_id == edit.span.source.source_id && !same {
                        return Err(SourceError::SnapshotMismatch.into());
                    }
                    if same
                        && (previous.span.start == edit.span.start
                            || (previous.span.start < edit.span.end
                                && edit.span.start < previous.span.end))
                    {
                        return Err(SourceError::OverlappingEdits.into());
                    }
                }
            }
        }
        Ok(())
    }
}
