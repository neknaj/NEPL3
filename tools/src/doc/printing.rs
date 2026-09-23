//! Selected Doc/Math/Sentence printers; each language owns its meaning.
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
    DocLower(nepl3_doc_core::lower::DocumentLowerError<E>),
    DocShape(nepl3_doc_core::check::ShapeError),
    DocPortable(nepl3_doc_core::portable::PortableError<E>),
    DocPrint(nepl3_doc_core::print::PrintFailure),
}
impl<E> From<StopReason> for Error<E> {
    fn from(reason: StopReason) -> Self {
        Self::Stopped(reason)
    }
}
/// Selected Sentence annotation printer with optional Math and Doc Inline forms.
/// The host supplies the compiled Sentence package from its resolved parse
/// profile. Foreign heads are taken from that package's declarations. Doc Inline
/// guests can contain the selected Math language; unsupported guests fail.
/// Recursive calls retain caller limits and use an additional depth ceiling of 64.
pub struct SentenceGuestPrinter<'a, C> {
    pub registry: &'a SchemaRegistry,
    pub sentence_package: &'a nepl3_engine::package::LanguagePackage,
    pub math_surface: Option<&'a SchemaRef>,
    pub doc_surface: Option<&'a SchemaRef>,
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
                + self.sentence_package.schema.package.len()
                + guest.syntax.category.len()) as u64
                + 64,
        )?;
        if guest.syntax.schema != self.sentence_package.schema
            || guest.syntax.category != "Sentence"
        {
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
        let mut forms = Vec::new();
        let count =
            usize::from(self.math_surface.is_some()) + usize::from(self.doc_surface.is_some());
        b.charge(
            Resource::AllocationUnits,
            (count * core::mem::size_of::<lower::ForeignInlineForm<'_>>()) as u64,
        )?;
        forms
            .try_reserve_exact(count)
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        if let Some(surface) = self.math_surface {
            forms.push(lower::ForeignInlineForm {
                kind: "Form:InlineMath",
                guest_schema: surface,
                guest_category: "Expr",
            });
        }
        if let Some(surface) = self.doc_surface {
            forms.push(lower::ForeignInlineForm {
                kind: "Form:DocumentInline",
                guest_schema: surface,
                guest_category: "Inline",
            });
        }
        let sentence = lower::presentation::sentence_with_foreign(
            &input,
            &self.sentence_package.schema,
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
            let (prefix, source) = b.with_depth_at_least(base.saturating_add(depth), |b| {
                if selected(closure, self.math_surface, "Expr", b)? {
                    let spelling = foreign_spelling(
                        self.registry,
                        self.sentence_package,
                        "Form:InlineMath",
                        "Math",
                        "Expr",
                        b,
                    )?;
                    self.math(closure, b).map(|source| (spelling, source))
                } else if selected(closure, self.doc_surface, "Inline", b)? {
                    let spelling = foreign_spelling(
                        self.registry,
                        self.sentence_package,
                        "Form:DocumentInline",
                        "Doc",
                        "Inline",
                        b,
                    )?;
                    self.document_inline(closure, b)
                        .map(|source| (spelling, source))
                } else {
                    Err(Error::Selection)
                }
            })?;
            // Serialize the selected one-field Inline form around owner output.
            let len = source
                .len()
                .checked_add(prefix.len().saturating_add(1))
                .ok_or_else(|| b.stop(StopReason::AllocationLimit))?;
            b.charge(Resource::Work, len as u64)?;
            b.charge(Resource::AllocationUnits, len as u64)?;
            b.charge(Resource::OutputBytes, len as u64)?;
            let mut inline = String::with_capacity(len);
            inline.push_str(prefix);
            inline.push(' ');
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

    fn math(
        &mut self,
        closure: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<String, Error<C::Error>> {
        if !selected(closure, self.math_surface, "Expr", b)? {
            return Err(Error::Selection);
        }
        closure
            .validate(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let input = closure
            .syntax
            .bundle
            .validate_with_sources(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let math = nepl3_math_core::lower::expression(
            &input,
            &closure.syntax.schema,
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
    }

    fn document_inline(
        &mut self,
        closure: &ForeignClosure,
        b: &mut Budget,
    ) -> Result<String, Error<C::Error>> {
        use nepl3_doc_core::{check, lower, model::GuestLanguage, print};
        closure
            .validate(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let input = closure
            .syntax
            .bundle
            .validate_with_sources(self.registry, b, self.codec.source_admission())
            .map_err(Error::Syntax)?;
        let document = lower::document(
            &input,
            &closure.syntax.schema,
            check::Category::Inline,
            self.registry,
            b,
            self.codec,
        )
        .map_err(Error::DocLower)?;
        let shape = document.value.validate_shape(b).map_err(Error::DocShape)?;
        let depths = print::guest_depths(&shape, b)?;
        let identity =
            print::identity(&document, self.registry, self.codec, b).map_err(Error::DocPortable)?;
        b.charge(
            Resource::AllocationUnits,
            (document.value.embeds.len() * core::mem::size_of::<print::PrintedGuest>()) as u64,
        )?;
        let mut guests = Vec::new();
        guests
            .try_reserve_exact(document.value.embeds.len())
            .map_err(|_| b.stop(StopReason::AllocationLimit))?;
        let base = b.current_depth();
        for ((embed, target), depth) in document
            .value
            .embeds
            .iter()
            .zip(&identity.guests)
            .zip(depths)
        {
            let text = b.with_depth_at_least(base.saturating_add(depth), |b| {
                self.math(&embed.closure, b)
            })?;
            guests.push(print::PrintedGuest {
                document_digest: identity.document_digest,
                embed: target.embed,
                guest_digest: target.guest_digest,
                text,
            });
        }
        let mut bindings = Vec::new();
        if !guests.is_empty() {
            let surface = self.math_surface.ok_or(Error::Selection)?;
            b.charge(Resource::Work, surface.package.len() as u64 + 4)?;
            b.charge(
                Resource::AllocationUnits,
                (core::mem::size_of::<print::GuestBinding>() + surface.package.len() + 4) as u64,
            )?;
            bindings
                .try_reserve_exact(1)
                .map_err(|_| b.stop(StopReason::AllocationLimit))?;
            bindings.push(print::GuestBinding {
                schema: surface.clone(),
                category: "Expr".into(),
                language: GuestLanguage::Math,
            });
        }
        let reply = print::print(
            &print::PrintRequest {
                document,
                mode: print::PrintMode::Prefix,
                bindings,
                guests,
            },
            self.registry,
            self.codec,
            b,
        )
        .map_err(Error::DocPortable)?;
        match reply.outcome {
            print::PrintOutcome::Complete { artifact } => Ok(artifact.text),
            print::PrintOutcome::Invalid { error } => Err(Error::DocPrint(error)),
            print::PrintOutcome::Stopped { reason } => Err(Error::Stopped(reason)),
        }
    }
}

/// Obtain the head from the host-selected compiled package. The printer only
/// supports the single foreign `syntax` field consumed by its typed adapter.
fn foreign_spelling<'a, E>(
    registry: &SchemaRegistry,
    package: &'a nepl3_engine::package::LanguagePackage,
    kind: &str,
    alias: &str,
    category: &str,
    b: &mut Budget,
) -> Result<&'a str, Error<E>> {
    let descriptor = registry
        .descriptor(&package.schema)
        .ok_or(Error::Selection)?;
    let mut selected = None;
    for form in &package.forms {
        b.charge(
            Resource::Work,
            (form.kind.schema.package.len() + package.schema.package.len()) as u64 + 64,
        )?;
        if form.kind.schema != package.schema {
            return Err(Error::Selection);
        }
        let name = usize::try_from(form.kind.local_kind)
            .ok()
            .and_then(|id| descriptor.types.get(id))
            .ok_or(Error::Selection)?
            .name
            .as_str();
        b.charge(
            Resource::Work,
            (name.len() + form.category.len() + kind.len() + 6) as u64,
        )?;
        if name != kind || form.category != "Inline" {
            continue;
        }
        let [field] = form.fields.as_slice() else {
            return Err(Error::Selection);
        };
        let read = usize::try_from(field.read.0)
            .ok()
            .and_then(|i| package.reads.get(i));
        let Some(nepl3_engine::package::ReadSpec::Foreign {
            alias: actual_alias,
            category: actual_category,
        }) = read
        else {
            return Err(Error::Selection);
        };
        b.charge(
            Resource::Work,
            (field.name.len()
                + actual_alias.len()
                + actual_category.len()
                + alias.len()
                + category.len()
                + form.spelling.len()
                + form.kind.schema.package.len()
                + package.schema.package.len()) as u64
                + 64,
        )?;
        if field.name != "syntax"
            || actual_alias != alias
            || actual_category != category
            || form.kind.schema != package.schema
            || form.spelling.is_empty()
            || selected.is_some()
        {
            return Err(Error::Selection);
        }
        selected = Some(form.spelling.as_str());
    }
    selected.ok_or(Error::Selection)
}

fn selected(
    closure: &ForeignClosure,
    surface: Option<&SchemaRef>,
    category: &str,
    b: &mut Budget,
) -> Result<bool, StopReason> {
    let Some(surface) = surface else {
        return Ok(false);
    };
    b.charge(
        Resource::Work,
        (closure.syntax.schema.package.len()
            + surface.package.len()
            + closure.syntax.category.len()
            + category.len()) as u64
            + 64,
    )?;
    Ok(&closure.syntax.schema == surface && closure.syntax.category == category)
}
