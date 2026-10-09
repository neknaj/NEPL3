//! Typed candidate failure causes. A received stop is data and does not stop
//! the receiver; only codec resource exhaustion affects its Budget.
use super::*;
use core::convert::Infallible;

pub(in crate::portable) fn value<E>(
    error: &CandidateError,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<E>> {
    let s = Schemas::new(r, b)?;
    let value = match error {
        CandidateError::Stopped(reason) => variant(
            s.engine,
            "CandidateError",
            "Stopped",
            [crate::portable::facts::stop_value(*reason, &s, b)?],
            b,
        )?,
        CandidateError::Access(error) => variant(
            s.engine,
            "CandidateError",
            "Access",
            [crate::portable::analysis::access_error_value(*error, r, b)?],
            b,
        )?,
        CandidateError::Binding(error) => variant(
            s.engine,
            "CandidateError",
            "Binding",
            [crate::portable::binding::error::encode(error, r, b)?],
            b,
        )?,
        CandidateError::NoOccurrence => variant(s.engine, "CandidateError", "NoOccurrence", [], b)?,
        CandidateError::NotReference => variant(s.engine, "CandidateError", "NotReference", [], b)?,
        CandidateError::NoStage => variant(s.engine, "CandidateError", "NoStage", [], b)?,
    };
    r.validate(&expected("CandidateError", b)?, &value, b)?;
    Ok(value)
}
pub(in crate::portable) fn read<E>(
    value: &NdfValue,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<CandidateError, PortableError<E>> {
    r.validate(&expected("CandidateError", b)?, value, b)?;
    let s = Schemas::new(r, b)?;
    Ok(match parts(value, s.engine, "CandidateError")? {
        ("Stopped", [reason]) => {
            CandidateError::Stopped(crate::portable::facts::stop_from(reason, &s)?)
        }
        ("Access", [error]) => {
            CandidateError::Access(crate::portable::analysis::access_error_read(error, r, b)?)
        }
        ("Binding", [error]) => {
            CandidateError::Binding(crate::portable::binding::error::decode(error, r, b)?)
        }
        ("NoOccurrence", []) => CandidateError::NoOccurrence,
        ("NotReference", []) => CandidateError::NotReference,
        ("NoStage", []) => CandidateError::NoStage,
        _ => return Err(PortableError::Shape),
    })
}

/// Standalone typed cause metadata; this grants no execution authority.
pub fn to_value(
    error: &CandidateError,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<Infallible>> {
    value(error, r, b)
}
pub fn from_value(
    input: &NdfValue,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<CandidateError, PortableError<Infallible>> {
    read(input, r, b)
}
