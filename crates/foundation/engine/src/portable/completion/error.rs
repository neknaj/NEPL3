//! Typed candidate failure causes. A received stop is data and does not stop
//! the receiver; only codec resource exhaustion affects its Budget.
use super::*;
use core::convert::Infallible;

pub fn to_value(
    error: &CandidateError,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<NdfValue, PortableError<Infallible>> {
    let s = Schemas::new(r)?;
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
            [crate::portable::analysis::access_error_to_value(
                *error, r, b,
            )?],
            b,
        )?,
        CandidateError::Binding(error) => variant(
            s.engine,
            "CandidateError",
            "Binding",
            [crate::portable::binding::failure_to_value(error, r, b)?],
            b,
        )?,
        CandidateError::NoOccurrence => variant(s.engine, "CandidateError", "NoOccurrence", [], b)?,
        CandidateError::NotReference => variant(s.engine, "CandidateError", "NotReference", [], b)?,
        CandidateError::NoStage => variant(s.engine, "CandidateError", "NoStage", [], b)?,
    };
    r.validate(&expected("CandidateError", b)?, &value, b)?;
    Ok(value)
}
pub fn from_value(
    value: &NdfValue,
    r: &SchemaRegistry,
    b: &mut Budget,
) -> Result<CandidateError, PortableError<Infallible>> {
    r.validate(&expected("CandidateError", b)?, value, b)?;
    let s = Schemas::new(r)?;
    Ok(match parts(value, s.engine, "CandidateError")? {
        ("Stopped", [reason]) => {
            CandidateError::Stopped(crate::portable::facts::stop_from(reason, &s)?)
        }
        ("Access", [error]) => CandidateError::Access(
            crate::portable::analysis::access_error_from_value(error, r, b)?,
        ),
        ("Binding", [error]) => {
            CandidateError::Binding(crate::portable::binding::failure_from_value(error, r, b)?)
        }
        ("NoOccurrence", []) => CandidateError::NoOccurrence,
        ("NotReference", []) => CandidateError::NotReference,
        ("NoStage", []) => CandidateError::NoStage,
        _ => return Err(PortableError::Shape),
    })
}
