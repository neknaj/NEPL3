//! Native Doc reader dispatch for an explicitly identified host implementation.
//! This services the existing typed engine boundary without exporting a copy of
//! the growing parser state for every synchronous reader call.
use nepl3_core::{
    budget::{Budget, Resource},
    schema::SchemaRegistry,
    source::{Digest, SourceAdmission, SourceReservation, SourceSnapshot, SourceStore},
};
use nepl3_engine::{parse::*, profile::*};
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    model::{ProviderCall, ReadRequest},
    runtime::ProviderReply,
    tokenizer::ReservationRequest,
};
use nepl3_wire::foundation::FoundationCodec;

pub struct NativeHost<'a> {
    registry: &'a SchemaRegistry,
    providers: Vec<ProviderImplementation>,
    reservation_prefix: String,
    reservation_id: u64,
    sources: SourceStore,
}
impl<'a> NativeHost<'a> {
    /// `implementation` identifies the actual executable/worker containing these
    /// providers; the caller must use these registrations when resolving its
    /// Profile. `reservation_prefix` is host-reserved for this operation only.
    pub fn new(
        registry: &'a SchemaRegistry,
        implementation: Digest,
        reservation_prefix: String,
        budget: &mut Budget,
    ) -> Result<Self, ParseError> {
        budget.charge(
            Resource::AllocationUnits,
            4 * core::mem::size_of::<nepl3_core::value::OperationRef>() as u64,
        )?;
        let mut operations = Vec::with_capacity(4);
        for kind in [
            BuiltinReader::Name,
            BuiltinReader::Number,
            BuiltinReader::Trivia,
        ] {
            operations.push(provider::operation(kind, registry, budget)?);
        }
        operations.push(super::reader::signature(registry, budget)?.operation);
        budget.charge(
            Resource::AllocationUnits,
            4 * core::mem::size_of::<ProviderImplementation>() as u64,
        )?;
        let mut providers = Vec::with_capacity(4);
        for operation in operations {
            budget.charge(
                Resource::AllocationUnits,
                (core::mem::size_of::<nepl3_core::value::OperationRef>() + operation.name.len())
                    as u64,
            )?;
            providers.push(ProviderImplementation {
                provider: operation.name.clone(),
                revision: 1,
                implementation_digest: implementation,
                operations: vec![operation],
            });
        }
        Ok(Self {
            registry,
            providers,
            reservation_prefix,
            reservation_id: 0,
            sources: SourceStore::default(),
        })
    }
    pub fn providers(&self) -> &[ProviderImplementation] {
        &self.providers
    }
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
                * (requirement.provider.len()
                    + operation.name.len()
                    + operation.schema.package.len()
                    + 66) as u64,
        )?;
        if requirement.operation != *operation
            || !self.providers.iter().any(|p| {
                p.provider == requirement.provider
                    && p.revision == requirement.revision
                    && p.implementation_digest == requirement.implementation_digest
                    && p.operations.contains(operation)
            })
        {
            return Err(ParseError::Context);
        }
        refresh_sources(&mut self.sources, &request.sources, budget)?;
        let sources = &self.sources;
        let snapshot = sources
            .resolve(&request.snapshot)
            .ok_or(nepl3_core::source::SourceError::MissingSnapshot)?;
        let mut codec = FoundationCodec::new(self.registry, sources, admission).map_err(|_| {
            budget
                .poll()
                .err()
                .map(ParseError::Stopped)
                .unwrap_or(ParseError::Context)
        })?;
        let context = request
            .context
            .check(&mut codec, sources, self.registry, budget)
            .map_err(|_| {
                budget
                    .poll()
                    .err()
                    .map(ParseError::Stopped)
                    .unwrap_or(ParseError::Context)
            })?;
        let read = if operation.schema.package == "nepl3.doc.reader" {
            super::reader::read
        } else {
            provider::read
        };
        budget.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<nepl3_reader::model::ReadReply>() as u64,
        )?;
        let reply = read(
            operation,
            ReadRequest {
                snapshot,
                start: request.start,
                limit: request.limit,
                final_input: request.final_input,
                context: &context,
                state: &request.state,
            },
            self.registry,
            sources,
            budget,
            admission,
        )?;
        Ok(Some(ProviderReply::Read(Box::new(reply))))
    }
    fn reservation(
        &mut self,
        _: &ReservationRequest,
        budget: &mut Budget,
        _: &mut SourceAdmission,
    ) -> Result<Option<SourceReservation>, ParseError> {
        budget.charge(Resource::Work, self.reservation_prefix.len() as u64 + 21)?;
        budget.charge(
            Resource::AllocationUnits,
            (self.reservation_prefix.len() as u64 + 21) * 2 + 7,
        )?;
        self.reservation_id = self
            .reservation_id
            .checked_add(1)
            .ok_or(ParseError::Reference)?;
        let id = format!("{}{}", self.reservation_prefix, self.reservation_id);
        Ok(Some(SourceReservation {
            source_id: nepl3_core::source::SourceId(id.clone()),
            revision: 0,
            uri: format!("memory:{id}"),
        }))
    }
}

/// Retain an index only for an exactly matching ordered request prefix. This is
/// storage reuse, never permission to resolve a source omitted by this request.
fn refresh_sources(
    cached: &mut SourceStore,
    requested: &[SourceSnapshot],
    budget: &mut Budget,
) -> Result<(), ParseError> {
    budget.poll()?;
    let mut prefix = cached.snapshots().len() <= requested.len();
    if prefix {
        for (old, current) in cached.snapshots().iter().zip(requested) {
            if !old.eq_with_budget(current, budget)? {
                prefix = false;
                break;
            }
        }
    }
    if prefix {
        let start = cached.snapshots().len();
        for source in &requested[start..] {
            cached.insert_ref_with_budget(source, budget)?;
        }
    } else {
        let mut replacement = SourceStore::default();
        for source in requested {
            replacement.insert_ref_with_budget(source, budget)?;
        }
        *cached = replacement;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nepl3_core::{budget::Limits, source::SourceId};

    fn budget() -> Budget {
        Budget::new(Limits {
            work: 1_000_000,
            source_bytes: 1_000_000,
            allocation_units: 1_000_000,
            ..Limits::default()
        })
    }
    fn source(id: &str, uri: &str, text: &str) -> SourceSnapshot {
        SourceSnapshot::new(
            SourceId(id.into()),
            0,
            uri.into(),
            text.into(),
            &mut budget(),
        )
        .expect("valid test snapshot")
    }
    #[test]
    fn source_cache_tracks_exact_request_scope_and_rechecks_independent_values() {
        let a = source("a", "memory:a", "\u{65e5}\u{672c}\r\n\u{1f600}");
        let b = source("b", "memory:b", "b");
        let mut store = SourceStore::default();
        refresh_sources(&mut store, &[a.clone(), b.clone()], &mut budget())
            .expect("valid cache transition");
        let mut reuse = Budget::new(Limits {
            work: 2,
            ..Limits::default()
        });
        refresh_sources(&mut store, &[a.clone(), b.clone()], &mut reuse)
            .expect("valid cache transition");
        assert_eq!(reuse.usage().allocation_units, 0);
        refresh_sources(&mut store, core::slice::from_ref(&b), &mut budget())
            .expect("valid cache transition");
        assert!(store.get_ref(a.identity()).is_none());
        assert_eq!(store.snapshots(), &[b.clone()]);
        refresh_sources(&mut store, &[b.clone(), a.clone()], &mut budget())
            .expect("valid cache transition");
        assert_eq!(store.snapshots(), &[b.clone(), a.clone()]);
        let independent = source("b", "memory:changed", "changed");
        refresh_sources(
            &mut store,
            core::slice::from_ref(&independent),
            &mut budget(),
        )
        .expect("valid cache transition");
        assert_eq!(store.snapshots(), &[independent]);
        // Conflicting duplicate declarations fail; neither old cached sources
        // nor an incomplete replacement can satisfy the rejected request.
        assert!(
            refresh_sources(
                &mut store,
                &[b.clone(), source("b", "memory:other", "b")],
                &mut budget()
            )
            .is_err()
        );
        refresh_sources(&mut store, &[a.clone(), b.clone()], &mut budget())
            .expect("valid cache transition");
        assert_eq!(store.snapshots(), &[a, b]);
        refresh_sources(&mut store, &[], &mut budget()).expect("valid cache transition");
        assert!(store.snapshots().is_empty());
        let mut cancelled = budget();
        cancelled.stop(nepl3_core::budget::StopReason::Cancelled);
        assert!(refresh_sources(&mut store, &[], &mut cancelled).is_err());
    }
}
