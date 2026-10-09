//! Complete canonical projection of a private saved continuation. Not a decoder
//! or authority to restore a new session. Source tables use canonical wire order.
use super::plan::value::{Context, Value};
use super::transform::{
    boundary, reader,
    value::{facts_value, variant},
};
use super::*;
use crate::{model::*, runtime::copy::copy};
use nepl3_core::{budget::Usage, diagnostic::Report, origin::Mapping};

pub(super) fn scope<C: FoundationValueCodec>(
    original: &[SourceSnapshot],
    added: &[SourceSnapshot],
    codec: &mut C,
    b: &mut Budget,
) -> Result<SourceStore, PortableError<C::Error>> {
    let mut sources = SourceStore::default();
    for source in original.iter().chain(added) {
        codec
            .source_admission()
            .admit_existing(source, b)
            .map_err(PortableError::Source)?;
        sources
            .insert_with_budget(copy(source, b)?, b)
            .map_err(PortableError::Source)?;
    }
    Ok(sources)
}

fn values<E>(values: &[NdfValue], b: &mut Budget) -> Result<NdfValue, PortableError<E>> {
    let mut out = Vec::new();
    for value in values {
        b.charge(Resource::Work, 1)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<NdfValue>() as u64,
        )?;
        out.push(value.clone_with_budget(b)?);
    }
    Ok(NdfValue::List(out))
}

fn phase<C: FoundationValueCodec>(
    phase: &FramePhase,
    ctx: &Context<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.charge(Resource::Work, 1)?;
    let s = ctx.reader;
    match phase {
        FramePhase::Enter => variant(s, "FramePhase", "Enter", [], b),
        FramePhase::Seq { next, values: v } => variant(
            s,
            "FramePhase",
            "Seq",
            [NdfValue::U64(*next), values(v, b)?],
            b,
        ),
        FramePhase::Choice {
            next,
            furthest,
            expected,
        } => variant(
            s,
            "FramePhase",
            "Choice",
            [
                NdfValue::U64(*next),
                NdfValue::U64(*furthest),
                expected.encode(ctx, c, b)?,
            ],
            b,
        ),
        FramePhase::Repeat {
            count,
            iteration_start,
            values: v,
        } => variant(
            s,
            "FramePhase",
            "Repeat",
            [
                NdfValue::U64(*count),
                NdfValue::U64(*iteration_start),
                values(v, b)?,
            ],
            b,
        ),
        FramePhase::AwaitChild => variant(s, "FramePhase", "AwaitChild", [], b),
        FramePhase::Then { first, end } => variant(
            s,
            "FramePhase",
            "Then",
            [first.clone_with_budget(b)?, NdfValue::U64(*end)],
            b,
        ),
        FramePhase::Provider { call_id } => {
            variant(s, "FramePhase", "Provider", [NdfValue::U64(*call_id)], b)
        }
    }
}

pub(super) fn checkpoint<C: FoundationValueCodec>(
    cp: &ReaderCheckpoint,
    original: &[SourceSnapshot],
    usage: Usage,
    ctx: &Context<'_>,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.charge(Resource::Work, 1)?;
    // Native collectors may repeat the same unchanged declaration across
    // successive replies. Emit the canonical source table once per identity;
    // SourceAdmission and SourceStore still reject conflicting bytes/locators.
    // This normalizes only the projection, never private rollback storage.
    let declared = scope(&[], &cp.sources, codec, b)?;
    let sources = scope(original, declared.snapshots(), codec, b)?;
    let mut c = codec.scoped_with_mappings(&sources, &cp.source_maps);
    // Reuse the foundation Report adapters for these components. This metered
    // temporary projection does not emit its Usage or modify saved counters.
    let report = Report {
        diagnostics: copy(&cp.diagnostics, b)?,
        events: copy(&cp.events, b)?,
        usage,
        trace_overflow: copy(&cp.trace_overflow, b)?,
    };
    let report = c.encode_report(&report, b).map_err(boundary)?;
    let f = fields(&report, c.foundation_schema(), "Report", 4)?;
    record(
        ctx.reader,
        "ReaderCheckpoint",
        [
            NdfValue::U64(cp.cursor),
            cp.state.clone_with_budget(b)?,
            c.encode_views(&cp.view, b).map_err(boundary)?,
            facts_value(&cp.facts, ctx.reader, &mut c, b)?,
            f[0].clone_with_budget(b)?,
            c.encode_sources(declared.snapshots(), b)
                .map_err(boundary)?,
            c.encode_mappings(&cp.source_maps, b).map_err(boundary)?,
            f[1].clone_with_budget(b)?,
            f[2].clone_with_budget(b)?,
        ],
        b,
    )
}

fn call<C: FoundationValueCodec>(
    call: &ProviderCall,
    ctx: &Context<'_>,
    c: &mut C,
    sources: &SourceStore,
    mappings: &[Mapping],
    registry: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.charge(Resource::Work, 1)?;
    let (tag, session, id, depth, operation, request) = match call {
        ProviderCall::Read {
            session_id,
            call_id,
            depth_base,
            operation,
            request,
        } => (
            "Read",
            session_id,
            call_id,
            depth_base,
            operation,
            request_to_value(request, ctx.reader, c, sources, registry, b)?,
        ),
        ProviderCall::Dependent {
            session_id,
            call_id,
            depth_base,
            operation,
            request,
        } => (
            "Dependent",
            session_id,
            call_id,
            depth_base,
            operation,
            super::dependent::to_value(request, ctx.reader, c, sources, registry, b)?,
        ),
        ProviderCall::Transform {
            session_id,
            call_id,
            depth_base,
            operation,
            request,
        } => (
            "Transform",
            session_id,
            call_id,
            depth_base,
            operation,
            super::transform::request::to_value(
                request, ctx.reader, c, sources, mappings, registry, b,
            )?,
        ),
    };
    variant(
        ctx.reader,
        "ProviderCall",
        tag,
        [
            session.encode(ctx, c, b)?,
            NdfValue::U64(*id),
            NdfValue::U64(*depth),
            operation.encode(ctx, c, b)?,
            request,
        ],
        b,
    )
}

pub(crate) fn value<C: FoundationValueCodec>(
    saved: &ReaderContinuation,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.poll()?;
    let ctx = Context::new(registry, b)?;
    let sources = super::transform::dispatch_sources(saved, &[], b, codec.source_admission())
        .map_err(reader)?;
    let mut c = codec.scoped_with_mappings(&sources, &saved.current.source_maps);
    let report = c.encode_report(&saved.report, b).map_err(boundary)?;
    let usage = fields(&report, c.foundation_schema(), "Report", 4)?[3].clone_with_budget(b)?;
    // The private runtime maintains this invariant. Do not silently substitute
    // one saved value for the other if a future implementation breaks it.
    if saved.usage != saved.report.usage {
        return Err(PortableError::Reader(
            crate::runtime::ReaderError::Continuation,
        ));
    }
    let mut frames = Vec::new();
    for frame in &saved.frames {
        b.charge(Resource::Work, 1)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<NdfValue>() as u64,
        )?;
        frames.push(record(
            ctx.reader,
            "ReaderFrame",
            [
                frame.expression.encode(&ctx, &mut c, b)?,
                NdfValue::U64(frame.start),
                checkpoint(
                    &frame.checkpoint,
                    &saved.request.sources,
                    saved.usage,
                    &ctx,
                    &mut c,
                    b,
                )?,
                phase(&frame.phase, &ctx, &mut c, b)?,
            ],
            b,
        )?);
    }
    b.charge(Resource::AllocationUnits, 32)?;
    let result = record(
        ctx.reader,
        "ReaderContinuation",
        [
            saved.session_id.encode(&ctx, &mut c, b)?,
            NdfValue::U64(saved.depth_base),
            saved.plan_schema.encode(&ctx, &mut c, b)?,
            NdfValue::Bytes(saved.plan_digest.0.to_vec()),
            request_to_value(&saved.request, ctx.reader, &mut c, &sources, registry, b)?,
            NdfValue::List(frames),
            checkpoint(
                &saved.current,
                &saved.request.sources,
                saved.usage,
                &ctx,
                &mut c,
                b,
            )?,
            call(
                &saved.pending,
                &ctx,
                &mut c,
                &sources,
                &saved.current.source_maps,
                registry,
                b,
            )?,
            usage,
            report,
        ],
        b,
    )?;
    validate_named(&result, ctx.reader, "ReaderContinuation", registry, b)?;
    Ok(result)
}

/// Project the pending domain Await envelope from the same private state.
/// Duplicated call/report fields use the already validated projection, not
/// separately supplied values or new source/report admission.
pub(crate) fn await_value<C: FoundationValueCodec>(
    saved: &ReaderContinuation,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let continuation = value(saved, registry, codec, b)?;
    let ctx = Context::new(registry, b)?;
    let f = fields(&continuation, ctx.reader, "ReaderContinuation", 10)?;
    let call = f[7].clone_with_budget(b)?;
    let report = f[9].clone_with_budget(b)?;
    let result = variant(
        ctx.reader,
        "ReadReply",
        "Await",
        [call, continuation, report],
        b,
    )?;
    validate_named(&result, ctx.reader, "ReadReply", registry, b)?;
    Ok(result)
}

#[cfg(test)]
mod tests;
