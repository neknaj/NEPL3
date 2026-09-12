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
    Shape(nepl3_doc_core::check::ShapeError),
    MathLower(nepl3_math_core::lower::LowerError),
    MathShape(nepl3_math_core::check::ShapeError),
    MathPrint(Box<nepl3_math_core::print::PrintError<Error<E>>>),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}

/// Explicitly selected Doc Sentence source adapter. This validates/lower/prints
/// the supplied closure, rather than copying its retained source text. Embedded
/// Math inside Doc is enabled only by an explicit `math_surface` selection.
/// This synchronous host adapter imposes a 64-level total depth ceiling on its
/// recursive composition, retaining the caller's stricter limits and sticky
/// stops. Other guest languages still require separate host adapters.
pub struct DocGuestPrinter<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub surface: &'a SchemaRef,
    pub math_surface: Option<&'a SchemaRef>,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> nepl3_math_core::print::GuestPrinter for DocGuestPrinter<'_, C> {
    type Error = Error<C::Error>;
    fn print(&mut self, guest: &ForeignClosure, b: &mut Budget) -> Result<String, Self::Error> {
        let mut ceiling = b.limits();
        ceiling.depth = ceiling.depth.min(64);
        let result = b.with_ceiling(ceiling, |b| self.document(guest, b));
        b.poll()?;
        result
    }
}
impl<C: FoundationValueCodec> DocGuestPrinter<'_, C> {
    fn document(
        &mut self,
        guest: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<String, Error<C::Error>> {
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
        let mut request = print::PrintRequest {
            document,
            mode: print::PrintMode::Prefix,
            bindings: vec![],
            guests: vec![],
        };
        if !request.document.value.embeds.is_empty()
            && let Some(surface) = self.math_surface
        {
            let identity = print::identity(&request.document, self.registry, self.codec, b)
                .map_err(Error::Portable)?;
            let shape = request
                .document
                .value
                .validate_shape(b)
                .map_err(Error::Shape)?;
            let depths = print::guest_depths(&shape, b)?;
            b.charge(
                Resource::AllocationUnits,
                (identity.guests.len() as u64)
                    .saturating_mul(core::mem::size_of::<print::PrintedGuest>() as u64)
                    .saturating_add(core::mem::size_of::<print::GuestBinding>() as u64)
                    .saturating_add(surface.package.len() as u64)
                    .saturating_add(4),
            )?;
            request
                .guests
                .try_reserve_exact(identity.guests.len())
                .map_err(|_| b.stop(StopReason::AllocationLimit))?;
            request
                .bindings
                .try_reserve_exact(1)
                .map_err(|_| b.stop(StopReason::AllocationLimit))?;
            request.bindings.push(print::GuestBinding {
                schema: surface.clone(),
                category: "Expr".into(),
                language: nepl3_doc_core::model::GuestLanguage::Math,
            });
            let base = b.current_depth();
            for (index, target) in identity.guests.iter().enumerate() {
                let closure = &request.document.value.embeds[index].closure;
                b.charge(
                    Resource::Work,
                    (closure.syntax.schema.package.len() as u64)
                        .saturating_add(surface.package.len() as u64)
                        .saturating_add(closure.syntax.category.len() as u64)
                        .saturating_add(64),
                )?;
                if &closure.syntax.schema != surface || closure.syntax.category != "Expr" {
                    return Err(Error::Selection);
                }
                let text = b.with_depth_at_least(base.saturating_add(depths[index]), |b| {
                    closure
                        .validate(self.registry, b, self.codec.source_admission())
                        .map_err(Error::Syntax)?;
                    let syntax = closure
                        .syntax
                        .bundle
                        .validate_with_sources(self.registry, b, self.codec.source_admission())
                        .map_err(Error::Syntax)?;
                    let math = nepl3_math_core::lower::expression(
                        &syntax,
                        surface,
                        nepl3_math_core::check::Category::Expr,
                        self.registry,
                        b,
                        self.codec.source_admission(),
                    )
                    .map_err(Error::MathLower)?;
                    let shape = math.value.validate_shape(b).map_err(Error::MathShape)?;
                    let result = nepl3_math_core::print::prefix(&shape, self, b);
                    b.poll()?;
                    result.map(|artifact| artifact.text).map_err(|e| {
                        if let Err(reason) =
                            b.charge(Resource::AllocationUnits, core::mem::size_of_val(&e) as u64)
                        {
                            return Error::Stopped(reason);
                        }
                        Error::MathPrint(Box::new(e))
                    })
                })?;
                request.guests.push(print::PrintedGuest {
                    document_digest: identity.document_digest,
                    embed: target.embed,
                    guest_digest: target.guest_digest,
                    text,
                });
            }
        }
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
