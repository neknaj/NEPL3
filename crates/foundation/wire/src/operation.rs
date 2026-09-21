//! Typed Invoke/Continuation boundaries using the registered foundation schema.
//! These codecs check transport structure and supplied content identities.
//! Operation dispatch, authorization and continuation registration belong to hosts.
use crate::{WireError, boundary::typed::Codec, boundary::*, source::*, view::*};
use alloc::vec::Vec;
use nepl3_core::{
    budget::{Budget, Limits},
    operation::{Continuation, Invoke},
    schema::{SchemaError, SchemaRegistry},
    source::{SourceAdmission, SourceStore},
    syntax::{ResourceContent, validate_resources},
    value::{NdfValue, OperationRef, SchemaRef},
};

impl Codec for OperationRef {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "OperationRef",
            [self.schema.value(s, b)?, text(&self.name, b)?],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "OperationRef", 2)?;
        Ok(Self {
            schema: Codec::from(&f[0], s, store, b)?,
            name: copied(&f[1], b)?,
        })
    }
}
impl Codec for Limits {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "Limits",
            [
                self.source_bytes,
                self.work,
                self.depth,
                self.nodes,
                self.allocation_units,
                self.output_bytes,
                self.diagnostics,
                self.events,
            ]
            .map(NdfValue::U64),
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        _: &SourceStore,
        _: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "Limits", 8)?;
        Ok(Self {
            source_bytes: as_u64(&f[0])?,
            work: as_u64(&f[1])?,
            depth: as_u64(&f[2])?,
            nodes: as_u64(&f[3])?,
            allocation_units: as_u64(&f[4])?,
            output_bytes: as_u64(&f[5])?,
            diagnostics: as_u64(&f[6])?,
            events: as_u64(&f[7])?,
        })
    }
}
impl Codec for Continuation {
    fn value(&self, s: &SchemaRef, b: &mut Budget) -> Result<NdfValue, WireError> {
        record(
            s,
            "Continuation",
            [
                self.provider.value(s, b)?,
                NdfValue::U64(self.parent_request),
                bytes(&self.snapshot_digest.0, b)?,
                self.state.value(s, b)?,
            ],
            b,
        )
    }
    fn from(
        v: &NdfValue,
        s: &SchemaRef,
        store: &SourceStore,
        b: &mut Budget,
    ) -> Result<Self, WireError> {
        let f = fields(v, s, "Continuation", 4)?;
        Ok(Self {
            provider: Codec::from(&f[0], s, store, b)?,
            parent_request: as_u64(&f[1])?,
            snapshot_digest: as_digest(&f[2])?,
            state: Codec::from(&f[3], s, store, b)?,
        })
    }
}

fn schema(registry: &SchemaRegistry) -> Result<&SchemaRef, WireError> {
    if !registry.is_finalized() {
        return Err(SchemaError::Unfinalized.into());
    }
    registry
        .selected("nepl3.foundation", 1)
        .ok_or(SchemaError::UnknownSchema.into())
}
/// Encode a structurally checked request. Supplied limits remain request data;
/// this conversion consumes the caller's existing budget.
pub fn encode_invoke(
    value: &Invoke,
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    b.poll()?;
    let s = schema(registry)?;
    validate_resources(&value.resources, b)?;
    let value = record(
        s,
        "Invoke",
        [
            NdfValue::U64(value.request_id),
            value.operation.value(s, b)?,
            value.input.value(s, b)?,
            value.environment.value(s, b)?,
            sources_value(&value.sources, s, admission, b)?,
            sequence(&value.resources, b, |r, b| {
                record(
                    s,
                    "ResourceContent",
                    [text(&r.id, b)?, bytes(&r.digest.0, b)?, bytes(&r.bytes, b)?],
                    b,
                )
            })?,
            value.limits.value(s, b)?,
        ],
        b,
    )?;
    crate::encode_checked(&value, &expected("Invoke"), registry, b)
}
/// Decode a request after schema, source identity and resource-content checks.
/// The selected operation must additionally validate its input/environment
/// semantics; supplied Limits do not reset the caller's conversion budget.
pub fn decode_invoke(
    input: &[u8],
    registry: &SchemaRegistry,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Invoke, WireError> {
    b.poll()?;
    let s = schema(registry)?;
    let value = crate::decode_checked(input, &expected("Invoke"), registry, b)?;
    let f = fields(value.value(), s, "Invoke", 7)?;
    let empty = SourceStore::default();
    let value = Invoke {
        request_id: as_u64(&f[0])?,
        operation: Codec::from(&f[1], s, &empty, b)?,
        input: Codec::from(&f[2], s, &empty, b)?,
        environment: Codec::from(&f[3], s, &empty, b)?,
        sources: sources_from(&f[4], s, admission, b)?,
        resources: collect(list(&f[5])?, b, |v, b| {
            let f = fields(v, s, "ResourceContent", 3)?;
            Ok(ResourceContent {
                id: copied(&f[0], b)?,
                digest: as_digest(&f[1])?,
                bytes: bytes_from(&f[2], b)?,
            })
        })?,
        limits: Codec::from(&f[6], s, &empty, b)?,
    };
    validate_resources(&value.resources, b)?;
    Ok(value)
}
pub fn encode_continuation(
    value: &Continuation,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    b.poll()?;
    let value = value.value(schema(registry)?, b)?;
    crate::encode_checked(&value, &expected("Continuation"), registry, b)
}
/// Decode the saved data. Call `Continuation::check_binding` against host-saved
/// expectations before using it, and verify the operation-specific state.
pub fn decode_continuation(
    input: &[u8],
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<Continuation, WireError> {
    b.poll()?;
    let s = schema(registry)?;
    let value = crate::decode_checked(input, &expected("Continuation"), registry, b)?;
    <Continuation as Codec>::from(value.value(), s, &SourceStore::default(), b)
}
