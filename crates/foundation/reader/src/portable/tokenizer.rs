//! Complete canonical projection of an owned, private tokenizer continuation.
use super::plan::value::{Context, Value};
use super::transform::{boundary, value::variant};
use super::*;
use crate::tokenizer::model::*;
use alloc::boxed::Box;
use nepl3_core::view::{Trivia, TriviaKind};

fn bytes<E>(digest: Digest, b: &mut Budget) -> Result<NdfValue, PortableError<E>> {
    b.charge(Resource::AllocationUnits, 32)?;
    Ok(NdfValue::Bytes(digest.0.to_vec()))
}
fn source_ref<C: FoundationValueCodec>(
    v: &SourceRef,
    c: &C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.charge(Resource::Work, v.source_id.0.len() as u64 + 40)?;
    record(
        c.foundation_schema(),
        "SourceRef",
        [
            text(&v.source_id.0, b)?,
            NdfValue::U64(v.revision),
            bytes(v.digest, b)?,
        ],
        b,
    )
}
fn scope<C: FoundationValueCodec>(
    v: &TokenizationScope,
    ctx: &Context<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    record(
        ctx.reader,
        "TokenizationScope",
        [
            v.operation_id.encode(ctx, c, b)?,
            bytes(v.profile_digest, b)?,
            source_ref(&v.snapshot, c, b)?,
        ],
        b,
    )
}
fn target<C: FoundationValueCodec>(
    v: &TokenTarget,
    ctx: &Context<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.charge(Resource::Work, 1)?;
    match v {
        TokenTarget::Mode => variant(ctx.reader, "TokenTarget", "Mode", [], b),
        TokenTarget::Builtin { reader, token_kind } => variant(
            ctx.reader,
            "TokenTarget",
            "Builtin",
            [reader.encode(ctx, c, b)?, token_kind.encode(ctx, c, b)?],
            b,
        ),
    }
}
fn phase<E>(
    v: TokenizationPhase,
    s: &SchemaRef,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    b.charge(Resource::Work, 1)?;
    let (tag, next) = match v {
        TokenizationPhase::Skip { next } => ("Skip", next),
        TokenizationPhase::Take { next } => ("Take", next),
    };
    variant(s, "TokenizationPhase", tag, [NdfValue::U64(next)], b)
}
fn trivia<C: FoundationValueCodec>(
    items: &[Trivia],
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    let mut out = Vec::new();
    for item in items {
        b.charge(Resource::Work, 1)?;
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<NdfValue>() as u64,
        )?;
        let span = c.encode_span(&item.span, b).map_err(boundary)?;
        let tag = match item.kind {
            TriviaKind::Whitespace => "Whitespace",
            TriviaKind::Comment => "Comment",
            TriviaKind::Bom => "Bom",
            TriviaKind::Skipped => "Skipped",
        };
        let kind = variant(c.foundation_schema(), "TriviaKind", tag, [], b)?;
        out.push(record(c.foundation_schema(), "Trivia", [span, kind], b)?);
    }
    Ok(NdfValue::List(out))
}
fn reservation<C: FoundationValueCodec>(
    v: &ReservationRequest,
    ctx: &Context<'_>,
    c: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.charge(Resource::Work, 1)?;
    record(
        ctx.reader,
        "ReservationRequest",
        [
            v.session_id.encode(ctx, c, b)?,
            NdfValue::U64(v.request_id),
            source_ref(&v.snapshot, c, b)?,
            NdfValue::U64(v.start),
            NdfValue::U64(v.limit),
        ],
        b,
    )
}

pub(crate) fn value<C: FoundationValueCodec>(
    saved: &TokenizationContinuation,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    b.poll()?;
    if saved.usage != saved.report.usage {
        return Err(PortableError::Reader(
            crate::runtime::ReaderError::Continuation,
        ));
    }
    let ctx = Context::new(registry, b)?;
    let sources =
        super::continuation::scope(&saved.request.sources, &saved.current.sources, codec, b)?;
    let mut c = codec.scoped_with_mappings(&sources, &saved.current.source_maps);
    let report = c.encode_report(&saved.report, b).map_err(boundary)?;
    let usage = fields(&report, c.foundation_schema(), "Report", 4)?[3].clone_with_budget(b)?;
    let pending = match &saved.pending {
        TokenizationWait::Reservation { request } => variant(
            ctx.reader,
            "TokenizationWait",
            "Reservation",
            [reservation(request, &ctx, &mut c, b)?],
            b,
        )?,
        TokenizationWait::Provider { continuation } => {
            // The inner projection reconstructs its own request/current/frame
            // scopes. Its saved Usage predates the outer capture and stays so.
            let inner = super::continuation::value(continuation, registry, &mut c, b)?;
            variant(ctx.reader, "TokenizationWait", "Provider", [inner], b)?
        }
    };
    let result = record(
        ctx.reader,
        "TokenizationContinuation",
        [
            scope(&saved.scope, &ctx, &mut c, b)?,
            saved.session_id.encode(&ctx, &mut c, b)?,
            saved.reader_schema.encode(&ctx, &mut c, b)?,
            bytes(saved.reader_plan_digest, b)?,
            bytes(saved.configuration_digest, b)?,
            request_to_value(&saved.request, ctx.reader, &mut c, &sources, registry, b)?,
            saved.mode.encode(&ctx, &mut c, b)?,
            target(&saved.target, &ctx, &mut c, b)?,
            phase(saved.phase, ctx.reader, b)?,
            super::continuation::checkpoint(
                &saved.current,
                &saved.request.sources,
                saved.usage,
                &ctx,
                &mut c,
                b,
            )?,
            trivia(&saved.trivia, &mut c, b)?,
            saved.expected.encode(&ctx, &mut c, b)?,
            NdfValue::U64(saved.furthest),
            pending,
            NdfValue::U64(saved.depth_base),
            usage,
            report,
        ],
        b,
    )?;
    validate_named(&result, ctx.reader, "TokenizationContinuation", registry, b)?;
    Ok(result)
}

/// Project only the pending Await/Reserve reply from its private continuation.
/// Repeated outward fields derive from the already validated outer checkpoint;
/// the nested Reader checkpoint and its older Usage are never substituted.
pub(crate) fn reply_value<C: FoundationValueCodec>(
    saved: &TokenizationContinuation,
    registry: &SchemaRegistry,
    codec: &mut C,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<C::Error>> {
    use super::transform::value::parts;
    let continuation = value(saved, registry, codec, b)?;
    let ctx = Context::new(registry, b)?;
    let f = fields(&continuation, ctx.reader, "TokenizationContinuation", 17)?;
    let cp = fields(&f[9], ctx.reader, "ReaderCheckpoint", 9)?;
    let (wait, pending) = parts(&f[13], ctx.reader, "TokenizationWait")?;
    let (tag, call) = match (wait, pending) {
        ("Provider", [reader]) => {
            let reader = fields(reader, ctx.reader, "ReaderContinuation", 10)?;
            ("Await", reader[7].clone_with_budget(b)?)
        }
        ("Reservation", [request]) => ("Reserve", request.clone_with_budget(b)?),
        _ => return Err(PortableError::Shape),
    };
    let cursor = cp[0].clone_with_budget(b)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<NdfValue>() as u64,
    )?;
    let state = NdfValue::Some(Box::new(cp[1].clone_with_budget(b)?));
    let trivia = f[10].clone_with_budget(b)?;
    let facts = cp[3].clone_with_budget(b)?;
    let sources = cp[5].clone_with_budget(b)?;
    let mappings = cp[6].clone_with_budget(b)?;
    let report = f[16].clone_with_budget(b)?;
    let outcome = variant(
        ctx.reader,
        "TokenizationOutcome",
        tag,
        [call, continuation],
        b,
    )?;
    let result = record(
        ctx.reader,
        "TokenizationReply",
        [
            outcome, cursor, state, trivia, facts, sources, mappings, report,
        ],
        b,
    )?;
    validate_named(&result, ctx.reader, "TokenizationReply", registry, b)?;
    Ok(result)
}
