//! Typed process envelopes composed before the length-prefixed NDF boundary.
use super::*;
use nepl3_core::operation::ProviderFrame;

/// Encode one process frame. The source closure belongs to the saved request
/// for Reply/Resume; supplied Invoke sources never authorize reply diagnostics.
pub fn encode_frame(
    value: &ProviderFrame,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Vec<u8>, WireError> {
    reply::admit(sources, admission, b)?;
    let s = schema(registry)?;
    let value = match value {
        ProviderFrame::Invoke(call) => variant(
            s,
            "ProviderFrame",
            "Invoke",
            [invoke_value(call, s, admission, b)?],
            b,
        )?,
        ProviderFrame::Resume(resume) => variant(
            s,
            "ProviderFrame",
            "Resume",
            [reply::resume_value(
                resume, s, registry, sources, admission, b,
            )?],
            b,
        )?,
        ProviderFrame::Reply {
            request_id,
            reply: response,
        } => variant(
            s,
            "ProviderFrame",
            "Reply",
            [
                NdfValue::U64(*request_id),
                reply::reply_value(response, s, registry, sources, admission, b)?,
            ],
            b,
        )?,
        ProviderFrame::Cancel { request_id } => variant(
            s,
            "ProviderFrame",
            "Cancel",
            [NdfValue::U64(*request_id)],
            b,
        )?,
        ProviderFrame::Close => variant(s, "ProviderFrame", "Close", [], b)?,
    };
    crate::frame::encode_checked(&value, &expected("ProviderFrame"), registry, b)
}

/// Decode one complete frame, retaining the unconsumed suffix. A host must
/// subsequently validate request state, direction, continuation and permissions.
pub fn decode_frame<'a>(
    input: &'a [u8],
    final_input: bool,
    registry: &SchemaRegistry,
    sources: &SourceStore,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<Option<(ProviderFrame, &'a [u8])>, WireError> {
    let s = schema(registry)?;
    let Some((value, rest)) =
        crate::frame::decode_checked(input, final_input, &expected("ProviderFrame"), registry, b)?
    else {
        return Ok(None);
    };
    reply::admit(sources, admission, b)?;
    let (case, fields) = variant_parts(value.value(), s, "ProviderFrame")?;
    let result = match (case, fields) {
        ("Invoke", [call]) => ProviderFrame::Invoke(invoke_from(call, s, admission, b)?),
        ("Resume", [resume]) => ProviderFrame::Resume(reply::resume_from(
            resume, s, registry, sources, admission, b,
        )?),
        ("Reply", [id, response]) => ProviderFrame::Reply {
            request_id: as_u64(id)?,
            reply: reply::reply_from(response, s, registry, sources, admission, b)?,
        },
        ("Cancel", [id]) => ProviderFrame::Cancel {
            request_id: as_u64(id)?,
        },
        ("Close", []) => ProviderFrame::Close,
        _ => return Err(WireError::InvalidType),
    };
    Ok(Some((result, rest)))
}
