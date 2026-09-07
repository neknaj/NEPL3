use super::{metrics::Metrics, *};
use nepl3_core::schema::SchemaRegistry;
use nepl3_reader::tokenizer::ReservationRequest;

pub(super) struct NativeHost<'a> {
    pub registry: &'a SchemaRegistry,
    pub providers: &'a [ProviderImplementation],
    pub source: &'a SourceSnapshot,
    pub reservation_id: u64,
    pub metrics: &'a mut Metrics,
}
impl ParseHost for NativeHost<'_> {
    fn provider(
        &mut self,
        call: &ProviderCall,
        requirement: &ProviderRequirement,
        budget: &mut Budget,
        admission: &mut SourceAdmission,
    ) -> Result<Option<ProviderReply>, ParseError> {
        let ProviderCall::Read {
            operation, request, ..
        } = call
        else {
            return Ok(None);
        };
        budget.charge(
            Resource::Work,
            self.providers.len() as u64
                * (requirement.provider.len() as u64
                    + operation.name.len() as u64
                    + operation.schema.package.len() as u64
                    + 66),
        )?;
        if !self.providers.iter().any(|provider| {
            provider.provider == requirement.provider
                && provider.revision == requirement.revision
                && provider.implementation_digest == requirement.implementation_digest
                && provider.operations.contains(operation)
        }) {
            return Err(ParseError::Context);
        }
        let declared =
            self.metrics
                .provider_sources
                .measure(budget, |budget| -> Result<_, ParseError> {
                    let mut declared = SourceStore::default();
                    for source in &request.sources {
                        declared.insert(source.clone_with_budget(budget)?)?;
                    }
                    Ok(declared)
                })?;
        let snapshot = declared
            .resolve(&request.snapshot)
            .ok_or(nepl3_core::source::SourceError::MissingSnapshot)?;
        let checked = self.metrics.provider_context.measure(budget, |budget| {
            let mut codec = FoundationCodec::new(self.registry, &declared, admission)
                .map_err(|_| ParseError::Context)?;
            request
                .context
                .check(&mut codec, &declared, self.registry, budget)
                .map_err(|_| ParseError::Context)
        })?;
        let terminal = self.metrics.provider_read.measure(budget, |budget| {
            provider::read(
                operation,
                ReadRequest {
                    snapshot,
                    start: request.start,
                    limit: request.limit,
                    final_input: request.final_input,
                    context: &checked,
                    state: &request.state,
                },
                self.registry,
                &declared,
                budget,
                admission,
            )
        })?;
        Ok(Some(ProviderReply::Read(Box::new(terminal))))
    }
    fn reservation(
        &mut self,
        _: &ReservationRequest,
        _: &mut Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ParseError> {
        self.reservation_id = self
            .reservation_id
            .checked_add(1)
            .ok_or(ParseError::Reference)?;
        Ok(Some(SourceReservation {
            source_id: SourceId(format!(
                "{}:decoded:{}",
                self.source.identity().source.0,
                self.reservation_id
            )),
            revision: self.source.identity().revision,
            uri: format!("memory:grammar-decoded/{}", self.reservation_id),
        }))
    }
}
