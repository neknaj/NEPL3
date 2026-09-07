//! Structural binding-result exchange. Decoding never constructs BindingAnalysis's
//! semantic proof; that requires execution against the selected tree and plans.
use super::{PortableError, boundary, value::*};
use crate::binding::{BindingError, BindingOutcome, BindingProgress, BindingReply, BindingResult};
use alloc::{boxed::Box, vec::Vec};
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    diagnostic::Report,
    facts::FactSet,
    origin::Mapping,
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceSnapshot, SourceStore},
    value::NdfValue,
    value_codec::FoundationValueCodec,
};
mod check;
mod error;
mod value;
use value::*;

/// Encode a typed failure as data. This does not select an operation outcome.
pub fn failure_to_value(
    value: &BindingError,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<core::convert::Infallible>> {
    let value = error::encode(value, registry, b)?;
    registry.validate(&expected("BindingFailure", b)?, &value, b)?;
    Ok(value)
}
pub fn failure_from_value(
    value: &NdfValue,
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<BindingError, PortableError<core::convert::Infallible>> {
    registry.validate(&expected("BindingFailure", b)?, value, b)?;
    error::decode(value, registry, b)
}

/// Owned raw data with checked structural references, not proof of execution or
/// authentication of a remote analysis identity.
#[derive(Debug)]
pub struct DecodedBindingReply {
    pub outcome: DecodedBindingOutcome,
    pub report: Report,
}
#[derive(Debug)]
pub enum DecodedBindingOutcome {
    Complete(Box<BindingResult>),
    Invalid {
        failure: BindingError,
        progress: Box<BindingProgress>,
    },
    Stopped {
        reason: StopReason,
        progress: Box<BindingProgress>,
    },
}
struct Data<'a> {
    facts: Option<&'a FactSet>,
    sources: &'a [SourceSnapshot],
    maps: &'a [Mapping],
    stages: &'a [crate::binding::BindingStage],
    occurrences: &'a [crate::binding::OccurrenceStage],
    open_inputs: &'a [nepl3_core::facts::OccurrenceId],
    exports: &'a [nepl3_core::facts::EntityId],
}
impl<'a> From<&'a BindingResult> for Data<'a> {
    fn from(v: &'a BindingResult) -> Self {
        Self {
            facts: Some(&v.facts),
            sources: &v.sources,
            maps: &v.source_maps,
            stages: &v.stages,
            occurrences: &v.occurrence_stages,
            open_inputs: &v.open_inputs,
            exports: &v.exports,
        }
    }
}
impl<'a> From<&'a BindingProgress> for Data<'a> {
    fn from(v: &'a BindingProgress) -> Self {
        Self {
            facts: v.facts.as_ref(),
            sources: &v.sources,
            maps: &v.source_maps,
            stages: &v.stages,
            occurrences: &v.occurrence_stages,
            open_inputs: &v.open_inputs,
            exports: &v.exports,
        }
    }
}
enum Outcome<'a> {
    Complete(&'a BindingResult),
    Invalid(&'a BindingError, &'a BindingProgress),
    Stopped(StopReason, &'a BindingProgress),
}
impl Outcome<'_> {
    fn data(&self) -> Data<'_> {
        match self {
            Self::Complete(v) => (*v).into(),
            Self::Invalid(_, v) | Self::Stopped(_, v) => (*v).into(),
        }
    }
}
pub fn reply_to_value<C: FoundationValueCodec>(
    reply: &BindingReply,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let outcome = match &reply.outcome {
        BindingOutcome::Complete(v) => Outcome::Complete(v.result()),
        BindingOutcome::Invalid { error, progress } => Outcome::Invalid(error, progress),
        BindingOutcome::Stopped { reason, progress } => Outcome::Stopped(*reason, progress),
    };
    encode(outcome, &reply.report, registry, codec, b)
}
pub fn decoded_to_value<C: FoundationValueCodec>(
    reply: &DecodedBindingReply,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let outcome = match &reply.outcome {
        DecodedBindingOutcome::Complete(v) => Outcome::Complete(v),
        DecodedBindingOutcome::Invalid { failure, progress } => Outcome::Invalid(failure, progress),
        DecodedBindingOutcome::Stopped { reason, progress } => Outcome::Stopped(*reason, progress),
    };
    encode(outcome, &reply.report, registry, codec, b)
}
fn encode<C: FoundationValueCodec>(
    outcome: Outcome<'_>,
    report: &Report,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    if report.trace_overflow.is_some() && !matches!(outcome, Outcome::Stopped(..)) {
        return Err(PortableError::Shape);
    }
    if matches!(&outcome,Outcome::Invalid(error,_) if error::stop_reason(error).is_some()) {
        return Err(PortableError::Shape);
    }
    let s = Schemas::new(registry)?;
    let data = outcome.data();
    let store = check::validate(&data, registry, b, codec.source_admission())?;
    let mut local = codec.scoped(&store);
    let report = local.encode_report(report, b).map_err(boundary)?;
    let encoded = data_value(
        &data,
        matches!(outcome, Outcome::Complete(_)),
        &s,
        &mut local,
        b,
    )?;
    let outcome = match outcome {
        Outcome::Complete(_) => variant(s.engine, "BindingOutcome", "Complete", [encoded], b)?,
        Outcome::Invalid(failure, _) => variant(
            s.engine,
            "BindingOutcome",
            "Invalid",
            [error::encode(failure, registry, b)?, encoded],
            b,
        )?,
        Outcome::Stopped(reason, _) => variant(
            s.engine,
            "BindingOutcome",
            "Stopped",
            [super::facts::stop_value(reason, &s, b)?, encoded],
            b,
        )?,
    };
    let value = record(s.engine, "BindingReply", [outcome, report], b)?;
    registry.validate(&expected("BindingReply", b)?, &value, b)?;
    Ok(value)
}
/// First-time receivers need only the selected schemas and the explicit source
/// declarations in this packet. An ambient store never repairs missing sources.
pub fn reply_from_value<C: FoundationValueCodec>(
    value: &NdfValue,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<DecodedBindingReply, PortableError<C::Error>> {
    registry.validate(&expected("BindingReply", b)?, value, b)?;
    let s = Schemas::new(registry)?;
    let f = fields(value, s.engine, "BindingReply", 2)?;
    let (kind, p) = parts(&f[0], s.engine, "BindingOutcome")?;
    let (complete, index) = match (kind, p.len()) {
        ("Complete", 1) => (true, 0),
        ("Invalid" | "Stopped", 2) => (false, 1),
        _ => return Err(PortableError::Shape),
    };
    let progress = data_from(&p[index], complete, &s, codec, b)?;
    let data = Data::from(&progress);
    let store = check::validate(&data, registry, b, codec.source_admission())?;
    let mut local = codec.scoped(&store);
    let report = local.decode_report(&f[1], b).map_err(boundary)?;
    if report.trace_overflow.is_some() && kind != "Stopped" {
        return Err(PortableError::Shape);
    }
    let outcome = match kind {
        "Complete" => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<BindingResult>() as u64,
            )?;
            DecodedBindingOutcome::Complete(Box::new(BindingResult {
                facts: progress.facts.ok_or(PortableError::Shape)?,
                sources: progress.sources,
                source_maps: progress.source_maps,
                stages: progress.stages,
                occurrence_stages: progress.occurrence_stages,
                open_inputs: progress.open_inputs,
                exports: progress.exports,
            }))
        }
        "Invalid" => {
            let failure = error::decode(&p[0], registry, b)?;
            if error::stop_reason(&failure).is_some() {
                return Err(PortableError::Shape);
            }
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<BindingProgress>() as u64,
            )?;
            DecodedBindingOutcome::Invalid {
                failure,
                progress: Box::new(progress),
            }
        }
        "Stopped" => {
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<BindingProgress>() as u64,
            )?;
            DecodedBindingOutcome::Stopped {
                reason: super::facts::stop_from(&p[0], &s)?,
                progress: Box::new(progress),
            }
        }
        _ => return Err(PortableError::Shape),
    };
    Ok(DecodedBindingReply { outcome, report })
}
