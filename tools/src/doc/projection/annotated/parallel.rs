//! All-variant parallel viewing without language selection or source flattening.
use super::*;

impl Annotated<'_, '_> {
    pub(super) fn paragraph(&mut self, items: &[FlowRef]) -> Result<(), Error> {
        let mut sentences = Vec::new();
        let mut parallel = false;
        for item in items {
            match self.plain.kind(item.0) {
                DocKind::Sentence { .. } => push(&mut sentences, item.0, self.plain.budget)?,
                DocKind::Parallel { variants } => {
                    if !parallel {
                        // Keep the authored paragraph boundary around its prose
                        // runs and individual parallel groups. Fixed tags only.
                        self.plain.emit("<div class=\"nepl-paragraph\">\n\n")?;
                        parallel = true;
                    }
                    if !sentences.is_empty() {
                        self.sentences(&sentences, Some(""))?;
                        self.plain.emit("\n\n")?;
                        sentences.clear();
                    }
                    self.parallel(variants)?;
                }
                _ => return Err(Error::Unsupported { node: item.0 }),
            }
        }
        if !sentences.is_empty() || !parallel {
            self.sentences(&sentences, Some(""))?;
            self.plain.emit("\n\n")?;
        }
        if parallel {
            self.plain.emit("</div>\n\n")?;
        }
        Ok(())
    }

    fn parallel(&mut self, variants: &[VariantRef]) -> Result<(), Error> {
        self.plain.emit("<div class=\"nepl-parallel\">\n\n")?;
        for variant in variants {
            self.plain.budget.charge(Resource::Work, 1)?;
            let DocKind::Variant { language, sentence } = self.plain.kind(variant.0) else {
                return Err(Error::Unsupported { node: variant.0 });
            };
            // The same bounded RFC 5646 syntax check used for HTML Lang makes
            // the exact original tag safe in the fixed quoted attribute.
            if !nepl3_core::lexical::language::well_formed(language, self.plain.budget)? {
                return Err(Error::Invalid("invalid parallel language".into()));
            }
            self.plain.emit("<div lang=\"")?;
            self.plain.emit(language)?;
            self.plain.emit("\">\n\n**")?;
            self.plain.text(variant.0, language, false)?;
            self.plain.emit("**\n\n")?;
            // Blank lines terminate CommonMark HTML blocks, so the content is
            // still parsed as Markdown, including code, links and footnotes.
            // https://spec.commonmark.org/0.31.2/#html-blocks
            // An explicitly empty variant remains a labelled language cell;
            // it is not silently removed or used to permit empty prose runs.
            if !matches!(self.plain.kind(sentence.0), DocKind::Sentence { inlines } if inlines.is_empty())
            {
                self.sentences(&[sentence.0], Some(""))?;
            }
            self.plain.emit("\n\n</div>\n\n")?;
        }
        self.plain.emit("</div>\n\n")
    }
}
