//! Selected Sentence contents for a checked Doc projection. Arena identifiers
//! remain local to the language that owns them.
use super::*;
use nepl3_sentence_core::{lower::ForeignInlineForm, model::Root, syntax::SentenceSyntax};

pub(super) struct Contents<'a>(std::borrow::Cow<'a, [Option<SentenceSyntax>]>);

impl<'a> Contents<'a> {
    pub(super) fn borrowed(slots: &'a [Option<SentenceSyntax>]) -> Self {
        Self(std::borrow::Cow::Borrowed(slots))
    }
    pub(super) fn prepare<C: FoundationValueCodec>(
        document: &DocumentSyntax,
        registry: &SchemaRegistry,
        codec: &mut C,
        budget: &mut Budget,
    ) -> Result<Self, Error>
    where
        C::Error: core::fmt::Debug,
    {
        let shape = document.value.validate_shape(budget).map_err(|e| match e {
            nepl3_doc_core::check::ShapeError::Stopped(reason) => Error::Stopped(reason),
            e => Error::Invalid(format!("{e:?}")),
        })?;
        let depths = nepl3_doc_core::print::guest_depths(&shape, budget)?;
        let mut forms = Vec::new();
        for (package, kind, category) in [
            ("standard.math", "Form:InlineMath", "Expr"),
            ("standard.doc", "Form:DocumentInline", "Inline"),
        ] {
            if let Some(schema) = registry.selected(package, 1) {
                annotated::push(
                    &mut forms,
                    ForeignInlineForm {
                        kind,
                        guest_schema: schema,
                        guest_category: category,
                    },
                    budget,
                )?;
            }
        }
        let base = budget.current_depth();
        let mut contents = Vec::new();
        for (embed, depth) in document.value.embeds.iter().zip(depths) {
            let value = if matches!(embed.kind, EmbedKind::Sentence | EmbedKind::SentenceInline) {
                // Syntax is admitted only against this host's selected surface.
                // Value uses Sentence's portable semantic schema internally.
                let package = match embed.content {
                    DocContent::Syntax { .. } => "nepl3.syntax.sentence",
                    DocContent::Value { .. } => "nepl3.sentence",
                };
                let surface = registry
                    .selected(package, 1)
                    .ok_or_else(|| Error::Invalid(format!("missing {package}")))?;
                let result = budget.with_depth_at_least(base.saturating_add(depth), |b| {
                    nepl3_suite::adapters::document::sentence::lower(
                        embed, surface, &forms, registry, codec, b,
                    )
                });
                budget.poll()?;
                Some(result.map_err(|e| Error::Invalid(format!("{e:?}")))?)
            } else {
                None
            };
            annotated::push(&mut contents, value, budget)?;
        }
        Ok(Self(std::borrow::Cow::Owned(contents)))
    }

    pub(super) fn get(&self, embed: EmbedRef) -> Result<&SentenceSyntax, Error> {
        self.0
            .get(embed.0 as usize)
            .and_then(Option::as_ref)
            .ok_or(Error::NeedsResolution)
    }

    pub(super) fn sentence(&self, embed: EmbedRef) -> Result<(&SentenceSyntax, u64), Error> {
        let syntax = self.get(embed)?;
        let Root::Sentence(root) = syntax.value.root else {
            return Err(Error::NeedsResolution);
        };
        Ok((syntax, root.0))
    }
}
