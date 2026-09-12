//! Host composition of Math source printing with the production Doc lowerer and
//! source printer. Neither language core depends on the other.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    syntax::{ForeignClosure, SyntaxError},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_doc_core::{lower, print};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Selection,
    Syntax(SyntaxError),
    Lower(lower::DocumentLowerError<E>),
    Portable(nepl3_doc_core::portable::PortableError<E>),
    Print(print::PrintFailure),
    Entry,
}

/// Explicitly selected Doc Sentence source adapter. This validates/lower/prints
/// the supplied closure, rather than copying its retained source text. Embedded
/// languages inside Doc still require their own selected bindings and replies;
/// the Doc printer rejects missing bindings instead of dropping that content.
pub struct DocGuestPrinter<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub surface: &'a SchemaRef,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> nepl3_math_core::print::GuestPrinter for DocGuestPrinter<'_, C> {
    type Error = Error<C::Error>;
    fn print(&mut self, guest: &ForeignClosure, b: &mut Budget) -> Result<String, Self::Error> {
        b.poll().map_err(Error::Stopped)?;
        b.charge(
            Resource::Work,
            (guest.syntax.schema.package.len() as u64)
                .saturating_add(self.surface.package.len() as u64)
                .saturating_add(guest.syntax.category.len() as u64)
                .saturating_add(64),
        )
        .map_err(Error::Stopped)?;
        if &guest.syntax.schema != self.surface || guest.syntax.category != "Sentence" {
            return Err(Error::Selection);
        }
        guest
            .validate(self.registry, b, self.codec.source_admission())
            .map_err(|e| match e.stop_reason() {
                Some(s) => Error::Stopped(s),
                None => Error::Syntax(e),
            })?;
        let syntax = guest
            .syntax
            .bundle
            .validate_with_sources(self.registry, b, self.codec.source_admission())
            .map_err(|e| match e.stop_reason() {
                Some(s) => Error::Stopped(s),
                None => Error::Syntax(e),
            })?;
        let document = lower::document(
            &syntax,
            self.surface,
            nepl3_doc_core::check::Category::Sentence,
            self.registry,
            b,
            self.codec,
        )
        .map_err(|e| match e {
            lower::DocumentLowerError::Stopped(s) => Error::Stopped(s),
            e => Error::Lower(e),
        })?;
        let request = print::PrintRequest {
            document,
            mode: print::PrintMode::Prefix,
            bindings: vec![],
            guests: vec![],
        };
        let reply = print::print(&request, self.registry, self.codec, b).map_err(|e| match e {
            nepl3_doc_core::portable::PortableError::Stopped(s) => Error::Stopped(s),
            e => Error::Portable(e),
        })?;
        match reply.outcome {
            print::PrintOutcome::Complete { artifact }
                if artifact.entry == print::PrintEntry::Sentence =>
            {
                Ok(artifact.text)
            }
            print::PrintOutcome::Complete { .. } => Err(Error::Entry),
            print::PrintOutcome::Invalid { error } => Err(Error::Print(error)),
            print::PrintOutcome::Stopped { reason } => Err(Error::Stopped(b.stop(reason))),
        }
    }
}
