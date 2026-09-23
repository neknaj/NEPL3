//! Explicit host-provided guest source. Digests bind assertions to input data;
//! they do not prove that the guest text has the same meaning as its syntax.
use super::*;
use crate::portable::{self, PortableError};
use nepl3_core::{
    schema::SchemaRegistry,
    value_codec::{FoundationCodecError, FoundationValueCodec},
};

pub const SYNTAX_DOMAIN: &[u8] = b"NEPL3.Math.Print.Syntax.v1\0";
pub const GUEST_DOMAIN: &[u8] = b"NEPL3.Math.Print.Guest.v1\0";
fn boundary<E: FoundationCodecError>(e: E) -> PortableError<E> {
    match e.stop_reason() {
        Some(s) => PortableError::Stopped(s),
        None => PortableError::Foundation(e),
    }
}

/// Prepare exact canonical identities under the current budget and source
/// admission. Execution recomputes these; this result transfers no paid work.
pub fn identity<C: FoundationValueCodec>(
    syntax: &MathSyntax,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<MathPrintIdentity, PortableError<C::Error>> {
    let raw = portable::to_value(syntax, r, c, b)?;
    let syntax_digest = c
        .canonical_value_digest(SYNTAX_DOMAIN, &raw, b)
        .map_err(boundary)?;
    b.charge(
        Resource::AllocationUnits,
        (syntax.value.embeds.len() as u64).saturating_mul(32),
    )?;
    let mut guests = Vec::new();
    guests
        .try_reserve_exact(syntax.value.embeds.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    for guest in &syntax.value.embeds {
        b.charge(Resource::Work, 1)?;
        let raw = c.encode_foreign_closure(guest, b).map_err(boundary)?;
        guests.push(
            c.canonical_value_digest(GUEST_DOMAIN, &raw, b)
                .map_err(boundary)?,
        );
    }
    Ok(MathPrintIdentity {
        syntax_digest,
        guests,
    })
}

struct Replies<'a> {
    closures: &'a [ForeignClosure],
    text: Vec<Option<&'a str>>,
}
enum ReplyError {
    Stopped(StopReason),
    UnknownClosure,
}
impl From<StopReason> for ReplyError {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
impl GuestPrinter for Replies<'_> {
    type Error = ReplyError;
    fn print(&mut self, closure: &ForeignClosure, b: &mut Budget) -> Result<String, Self::Error> {
        // Only references borrowed from the checked request are admitted here.
        // O(embeds) lookup per printed occurrence, with no portable pointer ID.
        for (index, candidate) in self.closures.iter().enumerate() {
            b.charge(Resource::Work, 1)?;
            if core::ptr::eq(closure, candidate) {
                if let Some(text) = self.text[index] {
                    b.charge(Resource::Work, text.len() as u64)?;
                    b.charge(Resource::AllocationUnits, text.len() as u64)?;
                    let mut copy = String::new();
                    copy.try_reserve_exact(text.len())
                        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
                    copy.push_str(text);
                    return Ok(copy);
                }
                return Err(ReplyError::UnknownClosure);
            }
        }
        Err(ReplyError::UnknownClosure)
    }
}

/// Print an explicit request. Boundary-invalid syntax is an Err; accepted
/// request failures and resource stops are typed results. No partial text is
/// returned. The caller's Budget retains complete usage and a sticky stop.
/// Identity/slot preparation is linear in encoded input and supplied guests.
/// Guest dispatch costs O(printed guest occurrences * embeds); output traversal
/// and text copies are additionally metered. Slot storage is O(embeds).
pub fn execute<C: FoundationValueCodec>(
    request: &MathPrintRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<MathPrintResult, PortableError<C::Error>> {
    let result = run(request, r, c, b);
    if let Err(reason) = b.poll() {
        return Ok(MathPrintResult::Stopped { reason });
    }
    match result {
        Err(PortableError::Stopped(reason)) => Ok(MathPrintResult::Stopped {
            reason: b.stop(reason),
        }),
        other => other,
    }
}
fn run<C: FoundationValueCodec>(
    request: &MathPrintRequest,
    r: &SchemaRegistry,
    c: &mut C,
    b: &mut Budget,
) -> Result<MathPrintResult, PortableError<C::Error>> {
    let id = identity(&request.syntax, r, c, b)?;
    let invalid = |failure| Ok(MathPrintResult::Invalid { failure });
    let closures = &request.syntax.value.embeds;
    b.charge(
        Resource::AllocationUnits,
        (closures.len() as u64).saturating_mul(core::mem::size_of::<Option<&str>>() as u64),
    )?;
    let mut text = Vec::new();
    text.try_reserve_exact(closures.len())
        .map_err(|_| b.stop(StopReason::AllocationLimit))?;
    text.resize(closures.len(), None);
    for (entry, guest) in request.guests.iter().enumerate() {
        b.charge(Resource::Work, 65)?;
        let index = usize::try_from(guest.embed.0).ok();
        if guest.syntax_digest != id.syntax_digest
            || index.and_then(|i| id.guests.get(i)) != Some(&guest.guest_digest)
        {
            return invalid(MathPrintFailure::InvalidGuestIdentity {
                entry: entry as u64,
            });
        }
        let index = index.ok_or(PortableError::Shape)?;
        if text[index].is_some() {
            return invalid(MathPrintFailure::DuplicateGuest { embed: guest.embed });
        }
        b.charge(Resource::Work, guest.text.len() as u64)?;
        if guest.text.trim().is_empty() {
            return invalid(MathPrintFailure::EmptyGuest { embed: guest.embed });
        }
        text[index] = Some(guest.text.as_str());
    }
    for (index, closure) in closures.iter().enumerate() {
        let embed = EmbedRef(index as u64);
        let Some(selected) = &request.sentence_schema else {
            return invalid(MathPrintFailure::MissingBinding { embed });
        };
        b.charge(
            Resource::Work,
            (selected.package.len() as u64)
                .saturating_add(closure.syntax.schema.package.len() as u64)
                .saturating_add(closure.syntax.category.len() as u64)
                .saturating_add(64),
        )?;
        if selected != &closure.syntax.schema || closure.syntax.category != "Sentence" {
            return invalid(MathPrintFailure::GuestCategory { embed });
        }
        if text[index].is_none() {
            return invalid(MathPrintFailure::UnresolvedGuest { embed });
        }
    }
    let shape = request
        .syntax
        .value
        .validate_shape(b)
        .map_err(PortableError::Expression)?;
    let result = super::prefix(&shape, &mut Replies { closures, text }, b);
    b.poll()?;
    match result {
        Ok(artifact) => Ok(MathPrintResult::Complete { artifact }),
        Err(PrintError::Stopped(s)) => Err(PortableError::Stopped(s)),
        Err(PrintError::Guest {
            error: ReplyError::Stopped(reason),
            ..
        }) => Err(PortableError::Stopped(reason)),
        Err(PrintError::Guest {
            error: ReplyError::UnknownClosure,
            ..
        }) => Err(PortableError::Shape),
        Err(PrintError::UnprintableName { node }) => {
            invalid(MathPrintFailure::UnprintableName { node })
        }
        Err(PrintError::EmptyGuest { embed }) => invalid(MathPrintFailure::EmptyGuest { embed }),
        Err(PrintError::InvalidState) => Err(PortableError::Shape),
    }
}
