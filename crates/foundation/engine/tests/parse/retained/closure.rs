use super::*;
use nepl3_core::{
    budget::Budget,
    origin::{Mapping, MappingKind},
};
use nepl3_reader::{
    model::{ProviderCall, ReadReply},
    runtime::ProviderReply,
};

struct SourceHost {
    inner: StatefulHost,
}
impl ParseHost for SourceHost {
    fn reservation(
        &mut self,
        request: &nepl3_reader::tokenizer::ReservationRequest,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<nepl3_core::source::SourceReservation>, ParseError> {
        self.inner.reservation(request, b, admission)
    }

    fn provider(
        &mut self,
        call: &ProviderCall,
        requirement: &ProviderRequirement,
        b: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ParseError> {
        let mut reply = self.inner.provider(call, requirement, b, admission)?;
        if self.inner.inner.calls == 1 {
            let ProviderCall::Read { request, .. } = call else {
                return Err(ParseError::Context);
            };
            let original = request
                .sources
                .iter()
                .find(|s| s.reference() == request.snapshot)
                .ok_or(ParseError::Reference)?;
            let generated = admission.create(
                SourceId("closure-generated".into()),
                0,
                "memory:closure-generated".into(),
                b"G".to_vec(),
                b,
            )?;
            let mapping = Mapping {
                source: generated.span(0, 1)?,
                target: original.span(0, 1)?,
                kind: MappingKind::Transformed,
            };
            let Some(ProviderReply::Read(reply)) = &mut reply else {
                return Err(ParseError::Context);
            };
            let ReadReply::Matched {
                sources,
                source_maps,
                report,
                ..
            } = reply.as_mut()
            else {
                return Err(ParseError::Context);
            };
            sources.push(generated);
            source_maps.push(mapping);
            report.usage = b.usage();
        }
        Ok(reply)
    }
}

#[test]
fn executed_closure_borrows_provider_sources_without_consuming_proof() -> TestResult {
    for (input, recovered) in [("let x x", false), ("let x", true)] {
        with_options(
            input,
            true,
            false,
            |profile, environments, sources, source, entry| {
                let states = [LanguageReaderState {
                    alias: "Host".into(),
                    state: NdfValue::Bool(false),
                }];
                let mut b = budget();
                let mut admission = SourceAdmission::default();
                let mut session = RetainedParseSession::new(
                    "closure".into(),
                    profile,
                    environments,
                    ParseRequest {
                        snapshot: source,
                        start: 0,
                        limit: input.len() as u64,
                        final_input: true,
                        entry,
                        states: &states,
                    },
                    sources,
                    &mut b,
                )
                .map_err(|e| format!("{e:?}"))?;
                let mut host = SourceHost {
                    inner: StatefulHost {
                        inner: host::Host {
                            action: host::Action::Serve,
                            calls: 0,
                            minimum_depth: 0,
                        },
                    },
                };
                let result = session
                    .read_with_host(&mut b, &mut admission, &mut host)
                    .map_err(|e| format!("{e:?}"))?;
                assert!(result.host_error.is_none());
                let RetainedParseExecution::Continue(proof) = result.execution else {
                    return Err("expected actual completed execution".into());
                };
                assert_eq!(
                    proof.execution().kind() == ExecutionKind::Recovered,
                    recovered
                );
                // The provider source is absent from the initial immutable seed.
                assert!(
                    proof
                        .seed()
                        .sources()
                        .snapshots()
                        .iter()
                        .all(|s| s.identity().source.0 != "closure-generated")
                );
                let execution = proof.execution();
                assert!(
                    execution
                        .sources()
                        .iter()
                        .any(|s| s.identity().source.0 == "closure-generated" && s.text() == "G")
                );
                assert_eq!(execution.source_maps().len(), 1);
                assert_eq!(execution.source_maps()[0].kind, MappingKind::Transformed);
                let mapping = &execution.source_maps()[0];
                assert_eq!(mapping.source.snapshot_ref().source.0, "closure-generated");
                assert_eq!((mapping.source.start(), mapping.source.end()), (0, 1));
                assert_eq!(mapping.target.snapshot_ref(), source.identity());
                assert_eq!((mapping.target.start(), mapping.target.end()), (0, 1));
                let sources_ptr = execution.sources().as_ptr();
                let maps_ptr = execution.source_maps().as_ptr();
                let source_count = execution.sources().len();
                // Repeated access retains the same owned storage, and consuming the
                // proof moves that storage unchanged into the ordinary reply.
                assert_eq!(execution.sources().as_ptr(), sources_ptr);
                assert_eq!(execution.source_maps().as_ptr(), maps_ptr);
                let reply = proof.into_reply();
                assert_eq!(reply.sources.as_ptr(), sources_ptr);
                assert_eq!(reply.source_maps.as_ptr(), maps_ptr);
                assert_eq!(reply.sources.len(), source_count);
                assert_eq!(reply.source_maps.len(), 1);
                Ok(())
            },
        )?;
    }
    Ok(())
}
