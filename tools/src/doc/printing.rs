//! Selected Math/Sentence printers; each language owns its meaning.
use nepl3_core::{
    budget::{Budget, Resource, StopReason},
    schema::SchemaRegistry,
    syntax::{ForeignClosure, SyntaxError},
    value::SchemaRef,
    value_codec::FoundationValueCodec,
};
use nepl3_sentence_core::{lower, print};

#[derive(Debug)]
pub enum Error<E> {
    Stopped(StopReason),
    Selection,
    Syntax(SyntaxError),
    Lower(lower::presentation::Error<E>),
    Print(print::Error),
    Shape(nepl3_sentence_core::check::Error),
    MathLower(nepl3_math_core::lower::LowerError),
    MathShape(nepl3_math_core::check::ShapeError),
    MathPrint(Box<nepl3_math_core::print::PrintError<Error<E>>>),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Selected Sentence annotation printer with optional `math` Inline support.
/// The selected surface must declare that concrete form. Recursive calls retain
/// caller limits and use an additional depth ceiling of 64.
pub struct SentenceGuestPrinter<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub surface: &'a SchemaRef,
    pub math_surface: Option<&'a SchemaRef>,
    pub codec: &'a mut C,
}
impl<C: FoundationValueCodec> nepl3_math_core::print::GuestPrinter for SentenceGuestPrinter<'_, C> {
    type Error = Error<C::Error>;
    fn print(&mut self, guest: &ForeignClosure, b: &mut Budget) -> Result<String, Self::Error> {
        let mut ceiling = b.limits();
        ceiling.depth = ceiling.depth.min(64);
        let result = b.with_ceiling(ceiling, |b| self.sentence(guest, b));
        b.poll()?;
        result
    }
}
impl<C: FoundationValueCodec> SentenceGuestPrinter<'_, C> {
    fn sentence(
        &mut self,
        guest: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<String, Error<C::Error>> {
        b.poll()?;
        b.charge(
            Resource::Work,
            (guest.syntax.schema.package.len()
                + self.surface.package.len()
                + guest.syntax.category.len()) as u64
                + 64,
        )?;
        if &guest.syntax.schema != self.surface || guest.syntax.category != "Sentence" {
            return Err(Error::Selection);
        }
        guest
            .validate(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let input = guest
            .syntax
            .bundle
            .validate_with_sources(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let forms = self.math_surface.map(|surface| lower::ForeignInlineForm {
            kind: "Form:InlineMath",
            guest_schema: surface,
            guest_category: "Expr",
        });
        let sentence = lower::presentation::sentence_with_foreign(
            &input,
            self.surface,
            forms.as_slice(),
            self.registry,
            self.codec,
            b,
        )
        .map_err(Error::Lower)?;
        let shape = sentence.value.validate_shape(b).map_err(Error::Shape)?;
        let depths = shape.foreign_depths(b).map_err(Error::Shape)?;
        let prepared = print::prepare(
            &sentence.value,
            self.registry,
            b,
            self.codec.source_admission(),
        )
        .map_err(Error::Print)?;
        b.charge(
            Resource::AllocationUnits,
            (sentence.value.embeds.len() as u64).saturating_mul(
                (core::mem::size_of::<String>()
                    + core::mem::size_of::<print::ResolvedInlineSource<'_, '_>>())
                    as u64,
            ),
        )?;
        let mut sources = Vec::with_capacity(sentence.value.embeds.len());
        let base = b.current_depth();
        for (closure, depth) in sentence.value.embeds.iter().zip(depths) {
            let surface = self.math_surface.ok_or(Error::Selection)?;
            b.charge(
                Resource::Work,
                (closure.syntax.schema.package.len()
                    + surface.package.len()
                    + closure.syntax.category.len()) as u64
                    + 64,
            )?;
            if &closure.syntax.schema != surface || closure.syntax.category != "Expr" {
                return Err(Error::Selection);
            }
            let source = b.with_depth_at_least(base.saturating_add(depth), |b| {
                let input = closure
                    .syntax
                    .bundle
                    .validate_with_sources(self.registry, b, self.codec.source_admission())
                    .map_err(Error::Syntax)?;
                let math = nepl3_math_core::lower::expression(
                    &input,
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
                result.map(|artifact| artifact.text).map_err(|error| {
                    if let Err(reason) = b.charge(
                        Resource::AllocationUnits,
                        core::mem::size_of_val(&error) as u64,
                    ) {
                        return Error::Stopped(reason);
                    }
                    Error::MathPrint(Box::new(error))
                })
            })?;
            // Serialize the selected one-field Inline form around Math output.
            let len = source
                .len()
                .checked_add(5)
                .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
            b.charge(Resource::Work, len as u64)?;
            b.charge(Resource::AllocationUnits, len as u64)?;
            b.charge(Resource::OutputBytes, len as u64)?;
            let mut inline = String::with_capacity(len);
            inline.push_str("math ");
            inline.push_str(&source);
            sources.push(inline);
        }
        let mut resolved = Vec::with_capacity(sources.len());
        for (index, source) in sources.iter().enumerate() {
            resolved.push(
                prepared
                    .resolve(
                        nepl3_sentence_core::model::EmbedRef(index as u64),
                        source,
                        b,
                    )
                    .map_err(Error::Print)?,
            );
        }
        prepared.render(&resolved, b).map_err(Error::Print)
    }
}
