use nepl3_core::{
    budget::{Budget, Resource},
    diagnostic::{Diagnostic, Report, Severity},
    source::{Digest, SourceAdmission, SourceId, SourceReservation},
    value::{NdfValue, Record, TypedValue},
};
use nepl3_engine::{
    parse::{ParseError, ParseHost},
    profile::ProviderRequirement,
};
use nepl3_reader::{
    model::{ProviderCall, ReadReply},
    runtime::ProviderReply,
    tokenizer::ReservationRequest,
};
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Action {
    #[default]
    Serve,
    Closure { order: u8, mode: u8 },
    DeclineSecond,
    CloseAfterDecline,
    FailSecond,
    CancelSecond,
    WrongSecond,
    NestedStopSecond,
    GeneratedFailSecond,
    GeneratedCancelSecond,
}
pub struct Host {
    pub action: Action,
    pub calls: usize,
    pub minimum_depth: u64,
}
impl ParseHost for Host {
    fn provider(
        &mut self,
        call: &ProviderCall,
        requirement: &ProviderRequirement,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ParseError> {
        let ProviderCall::Read {
            operation,
            request,
            depth_base,
            ..
        } = call
        else {
            return Err(ParseError::Context);
        };
        assert_eq!(budget.current_depth(), *depth_base);
        assert!(*depth_base >= self.minimum_depth);
        assert_eq!(requirement.operation, *operation);
        assert_eq!(
            requirement.implementation_digest,
            Digest::of(b"fixture reader implementation manifest")
        );
        self.calls += 1;
        if self.calls == 2 {
            match self.action {
                Action::DeclineSecond | Action::CloseAfterDecline => return Ok(None),
                Action::FailSecond | Action::GeneratedFailSecond => {
                    return Err(ParseError::Context);
                }
                Action::CancelSecond | Action::GeneratedCancelSecond => {
                    budget.cancel();
                    budget.poll()?;
                }
                Action::NestedStopSecond => {
                    return Err(ParseError::Reader(
                        nepl3_reader::runtime::ReaderError::Stopped(
                            nepl3_core::budget::StopReason::Cancelled,
                        ),
                    ));
                }
                Action::WrongSecond => {
                    return Ok(Some(ProviderReply::Read(Box::new(ReadReply::Matched {
                        value: NdfValue::Unit,
                        end: request.start + 1,
                        new_state: NdfValue::Unit,
                        view: nepl3_core::view::ViewBundle {
                            elements: vec![],
                            roots: vec![],
                        },
                        facts: vec![],
                        sources: vec![],
                        source_maps: vec![],
                        report: Report {
                            usage: budget.usage(),
                            ..Report::default()
                        },
                    }))));
                }
                Action::Serve => {}
                Action::Closure { mode: 2, .. } => { budget.cancel(); budget.poll()?; }
                Action::Closure { mode: 3, .. } => return Ok(None),
                Action::Closure { .. } => {}
            }
        }
        let snapshot = request
            .sources
            .iter()
            .find(|source| source.reference() == request.snapshot)
            .ok_or(ParseError::Reference)?;
        let start = request.start as usize;
        let end = snapshot.text()[start..]
            .find(' ')
            .map_or(snapshot.text().len(), |i| start + i);
        let value = &snapshot.text()[start..end];
        budget.charge(Resource::Work, value.len() as u64)?;
        budget.charge(Resource::AllocationUnits, value.len() as u64)?;
        budget.charge(Resource::Diagnostics, 1)?;
        let mut generated = vec![];
        if let Action::Closure { order, mode } = self.action {
            for at in 0..32 {
                let i = match order { 0 => at, 1 => 31-at, _ => (at*13)%32 };
                let name = format!("aux:{}:{:04}", "\u{65e5}\u{672c}".repeat(8), i/2);
                let uri = if mode == 1 && self.calls == 2 && at == 15 { "changed:uri".into() } else { format!("memory:aux:{i}") };
                // Independent owned snapshot can repeat prior identity; admission and
                // actual reader boundary must reject the changed URI on call two.
                let revision = if i%2==0 { 0 } else { u64::MAX };
                let source = if let Some(existing) = request.sources.iter().find(|s|s.identity().source.0==name && s.identity().revision==revision && s.uri()==uri) {
                    existing.clone_with_budget(budget)?
                } else if uri == "changed:uri" {
                    nepl3_core::source::SourceSnapshot::new(SourceId(name), revision, uri, b"abc".to_vec(), budget)?
                } else {
                    admission.create(SourceId(name), revision, uri, b"abc".to_vec(), budget)?
                };
                generated.push(source);
            }
        }
        let mut events = vec![];
        let primary = if self.calls == 1
            && matches!(
                self.action,
                Action::GeneratedFailSecond | Action::GeneratedCancelSecond
            ) {
            let source = admission.create(
                SourceId("host-generated".into()),
                0,
                "memory:host-generated".into(),
                b"diagnostic source".to_vec(),
                budget,
            )?;
            let span = source.span(0, source.text().len() as u64)?;
            generated.push(source);
            budget.charge(Resource::Events, 1)?;
            events.push(nepl3_core::diagnostic::Event {
                schema: operation.schema.clone(),
                kind: "FixtureEvent".into(),
                operation_path: vec![],
                span: Some(span.clone()),
                payload: TypedValue::Record(Record {
                    schema: operation.schema.clone(),
                    kind: "List:Nil".into(),
                    fields: vec![],
                }),
            });
            Some(span)
        } else {
            None
        };
        let diagnostic = Diagnostic {
            schema: operation.schema.clone(),
            code: "FixtureRead".into(),
            stage: "reader".into(),
            severity: Severity::Information,
            arguments: TypedValue::Record(Record {
                schema: operation.schema.clone(),
                kind: "List:Nil".into(),
                fields: vec![],
            }),
            primary,
            related: vec![],
            fixes: vec![],
        };
        Ok(Some(ProviderReply::Read(Box::new(ReadReply::Matched {
            value: NdfValue::Text(value.into()),
            end: end as u64,
            new_state: NdfValue::Unit,
            view: nepl3_core::view::ViewBundle {
                elements: vec![],
                roots: vec![],
            },
            facts: vec![],
            sources: generated,
            source_maps: vec![],
            report: Report {
                diagnostics: vec![diagnostic],
                events,
                usage: budget.usage(),
                ..Report::default()
            },
        }))))
    }
    fn reservation(
        &mut self,
        _: &ReservationRequest,
        _: &mut Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ParseError> {
        Ok(Some(SourceReservation {
            source_id: SourceId("decoded".into()),
            revision: 0,
            uri: "memory:decoded".into(),
        }))
    }
}
