//! Restricted Markdown compatibility projection for reviewed migration pages.
//! Not a general Doc renderer: annotations, links and nontrivial flow remain
//! explicit unsupported inputs until their preservation contracts are defined.
use nepl3_core::{budget::*, schema::SchemaRegistry, value_codec::FoundationValueCodec};
use nepl3_doc_core::{model::*, prepare};

#[derive(Debug, Eq, PartialEq)]
pub enum Error {
    Stopped(StopReason),
    Invalid(String),
    NeedsResolution,
    Unsupported { node: u64 },
    Text { node: u64 },
    OutputLimit,
}
impl From<StopReason> for Error {
    fn from(value: StopReason) -> Self {
        Self::Stopped(value)
    }
}

/// Produce a view of a validated Article. This does not establish legacy URL
/// compatibility or authorize changing a page's canonical source.
pub fn markdown<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
) -> Result<String, Error>
where
    C::Error: core::fmt::Debug,
{
    let plan = prepare::inspect(document, registry, codec, budget).map_err(|e| match e {
        prepare::PreparationError::Stopped(s) => Error::Stopped(s),
        e => Error::Invalid(format!("{e:?}")),
    })?;
    if !plan.requirements.is_empty() {
        return Err(Error::NeedsResolution);
    }
    let DocRoot::Article(root) = document.value.root else {
        return Err(Error::Unsupported { node: 0 });
    };
    let mut writer = Writer {
        doc: document,
        budget,
        output: String::new(),
    };
    let DocKind::Article { title, body, .. } = writer.kind(root.0) else {
        return Err(Error::Unsupported { node: root.0 });
    };
    writer.emit("# ")?;
    writer.sentence(title.0, None)?;
    writer.emit("\n\n")?;
    writer.body(body.0, 1)?;
    Ok(writer.output)
}

struct Writer<'a, 'b> {
    doc: &'a DocumentSyntax,
    budget: &'b mut Budget,
    output: String,
}
impl<'a> Writer<'a, '_> {
    fn kind(&self, node: u64) -> &'a DocKind {
        &self.doc.value.nodes[node as usize].kind
    }
    fn emit(&mut self, value: &str) -> Result<(), Error> {
        self.budget.charge(Resource::Work, value.len() as u64)?;
        self.budget
            .charge(Resource::OutputBytes, value.len() as u64)?;
        let needed = self
            .output
            .len()
            .checked_add(value.len())
            .ok_or(Error::OutputLimit)?;
        if needed > 1024 * 1024 {
            return Err(Error::OutputLimit);
        }
        if needed > self.output.capacity() {
            let capacity = needed.max(self.output.capacity().saturating_mul(2));
            self.budget
                .charge(Resource::AllocationUnits, capacity as u64)?;
            self.output.reserve_exact(capacity - self.output.len());
        }
        self.output.push_str(value);
        Ok(())
    }
    fn text(&mut self, node: u64, text: &str, code: bool) -> Result<(), Error> {
        self.budget.charge(Resource::Work, text.len() as u64)?;
        if text.chars().any(|c| c.is_control()) || text.is_empty() {
            return Err(Error::Text { node });
        }
        if code {
            let ticks = text.split(|c| c != '`').map(str::len).max().unwrap_or(0) + 1;
            for _ in 0..ticks {
                self.emit("`")?;
            }
            let padded = text.starts_with('`')
                || text.ends_with('`')
                || (text.starts_with(' ') && text.ends_with(' ') && text.chars().any(|c| c != ' '));
            if padded {
                self.emit(" ")?;
            }
            self.emit(text)?;
            if padded {
                self.emit(" ")?;
            }
            for _ in 0..ticks {
                self.emit("`")?;
            }
        } else {
            // All ASCII punctuation is escapable in CommonMark. Escaping it
            // avoids accidental links, entities, block markers or emphasis.
            for c in text.chars() {
                if c.is_ascii_punctuation() {
                    self.emit("\\")?;
                }
                self.emit(c.encode_utf8(&mut [0; 4]))?;
            }
        }
        Ok(())
    }
    fn sentence(&mut self, node: u64, continuation: Option<&str>) -> Result<(), Error> {
        self.budget.charge(Resource::Work, 1)?;
        let DocKind::Sentence { inlines } = self.kind(node) else {
            return Err(Error::Unsupported { node });
        };
        if inlines.is_empty() {
            return Err(Error::Unsupported { node });
        }
        if inlines.first().is_some_and(|r| matches!(self.kind(r.0), DocKind::Text { text } if text.starts_with(char::is_whitespace)))
            || inlines.last().is_some_and(|r| matches!(self.kind(r.0), DocKind::Text { text } if text.ends_with(char::is_whitespace))) {
            return Err(Error::Text { node });
        }
        let mut previous_code = false;
        for (index, child) in inlines.iter().enumerate() {
            let is_code = matches!(self.kind(child.0), DocKind::InlineCode { .. });
            if is_code && previous_code {
                return Err(Error::Unsupported { node: child.0 });
            }
            previous_code = is_code;
            match self.kind(child.0) {
                DocKind::Text { text } => self.text(child.0, text, false)?,
                DocKind::InlineCode { text } => self.text(child.0, text, true)?,
                DocKind::Break => {
                    let Some(indent) = continuation else {
                        return Err(Error::Unsupported { node: child.0 });
                    };
                    // CommonMark drops breaks at block edges; an empty physical
                    // line terminates a paragraph. Refuse those lossy shapes.
                    if index == 0 || index + 1 == inlines.len() {
                        return Err(Error::Unsupported { node: child.0 });
                    }
                    let before = self.kind(inlines[index - 1].0);
                    let after = self.kind(inlines[index + 1].0);
                    if matches!(before, DocKind::Break)
                        || matches!(after, DocKind::Break)
                        || matches!(before, DocKind::Text { text } if text.ends_with(char::is_whitespace))
                        || matches!(after, DocKind::Text { text } if text.starts_with(char::is_whitespace))
                    {
                        return Err(Error::Unsupported { node: child.0 });
                    }
                    self.emit("\\\n")?;
                    self.emit(indent)?;
                }
                _ => return Err(Error::Unsupported { node: child.0 }),
            }
        }
        Ok(())
    }
    fn paragraph(&mut self, node: u64, continuation: &str) -> Result<(), Error> {
        let DocKind::Paragraph { items } = self.kind(node) else {
            return Err(Error::Unsupported { node });
        };
        if items.len() != 1 {
            return Err(Error::Unsupported { node });
        }
        self.sentence(items[0].0, Some(continuation))
    }
    fn raw_code(&mut self, node: u64, hint: Option<&str>, text: &str) -> Result<(), Error> {
        self.budget.charge(Resource::Work, text.len() as u64)?;
        // CommonMark normalizes CR/CRLF and supplies a final newline to a
        // nonempty fenced block. Do not silently change those source bytes.
        if (!text.is_empty() && !text.ends_with('\n'))
            || text
                .chars()
                .any(|c| c.is_control() && c != '\n' && c != '\t')
        {
            return Err(Error::Text { node });
        }
        if let Some(hint) = hint {
            self.budget.charge(Resource::Work, hint.len() as u64)?;
            if hint.is_empty()
                || !hint
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_+.-".contains(&c))
            {
                return Err(Error::Text { node });
            }
        }
        let fence = (text.split(|c| c != '`').map(str::len).max().unwrap_or(0) + 1).max(3);
        for _ in 0..fence {
            self.emit("`")?;
        }
        if let Some(hint) = hint {
            self.emit(hint)?;
        }
        self.emit("\n")?;
        self.emit(text)?;
        for _ in 0..fence {
            self.emit("`")?;
        }
        self.emit("\n\n")
    }
    fn body(&mut self, node: u64, level: usize) -> Result<(), Error> {
        self.budget.charge(Resource::Work, 1)?;
        let DocKind::Body { blocks } = self.kind(node) else {
            return Err(Error::Unsupported { node });
        };
        self.budget.charge(Resource::Work, blocks.len() as u64)?;
        let sections = blocks
            .iter()
            .filter(|c| matches!(self.kind(c.0), DocKind::Section { .. }))
            .count();
        // Markdown cannot close an explicit Doc section before following outer
        // content. The initial projection accepts only sibling root sections,
        // or a section-free article, so it never silently reparents blocks.
        if sections != 0 && (level != 1 || sections != blocks.len()) {
            return Err(Error::Unsupported { node });
        }
        let mut previous_list = false;
        for child in blocks {
            let is_list = matches!(self.kind(child.0), DocKind::List { .. });
            if previous_list && is_list {
                return Err(Error::Unsupported { node: child.0 });
            }
            previous_list = is_list;
            match self.kind(child.0) {
                DocKind::Section { title, body, .. } if level < 6 => {
                    for _ in 0..=level {
                        self.emit("#")?;
                    }
                    self.emit(" ")?;
                    self.sentence(title.0, None)?;
                    self.emit("\n\n")?;
                    self.body(body.0, level + 1)?;
                }
                DocKind::Paragraph { .. } => {
                    self.paragraph(child.0, "")?;
                    self.emit("\n\n")?;
                }
                DocKind::RawCode {
                    language_hint,
                    text,
                } => {
                    self.raw_code(child.0, language_hint.as_deref(), text)?;
                }
                DocKind::List {
                    kind: ListKind::Unordered,
                    items,
                } if !items.is_empty() => {
                    for item in items {
                        let DocKind::ListItem {
                            checked: None,
                            body,
                        } = self.kind(item.0)
                        else {
                            return Err(Error::Unsupported { node: item.0 });
                        };
                        let DocKind::Body { blocks } = self.kind(body.0) else {
                            return Err(Error::Unsupported { node: body.0 });
                        };
                        if blocks.len() != 1 {
                            return Err(Error::Unsupported { node: body.0 });
                        }
                        self.emit("- ")?;
                        self.paragraph(blocks[0].0, "  ")?;
                        self.emit("\n")?;
                    }
                    self.emit("\n")?;
                }
                _ => return Err(Error::Unsupported { node: child.0 }),
            }
        }
        Ok(())
    }
}

pub fn from_source(compiled: &super::source::Compiled, source: &str) -> Result<String, String> {
    use super::source::{budget, err, with_input_route};
    use nepl3_core::source::{SourceAdmission, SourceStore};
    use nepl3_doc_core::{check::Category, lower};
    use nepl3_wire::foundation::FoundationCodec;
    if source.len() as u64 > super::export::MAX_SOURCE_BYTES {
        return Err("SourceLimit".into());
    }
    with_input_route(true, compiled, source, "Article", |tree, profile, b, a| {
        let checked = tree
            .tree()
            .bundle
            .validate_with_sources(profile.registry(), b, a)
            .map_err(err)?;
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let document = lower::document(
            &checked,
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            &mut budget(),
            &mut codec,
        )
        .map_err(err)?;
        markdown(&document, profile.registry(), &mut codec, &mut budget()).map_err(err)
    })
}

pub fn write(input: &std::path::Path, output: &std::path::Path) -> crate::Result<()> {
    use std::io::{Read, Write};
    if output.exists() {
        return Err("output already exists".into());
    }
    let path = input.to_str().ok_or("input path must be UTF-8")?;
    if path.len() > 4096 || path.chars().any(char::is_control) {
        return Err("input path is not representable in projection metadata".into());
    }
    let escaped = path
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('-', "&#45;");
    let mut source = String::new();
    std::fs::File::open(input)?
        .take(super::export::MAX_SOURCE_BYTES + 1)
        .read_to_string(&mut source)?;
    let result = from_source(&super::source::compiled()?, &source)?;
    let digest: String = nepl3_core::source::Digest::of(source.as_bytes())
        .0
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let provenance = format!(
        "<!-- Generated from {escaped}; renderer nepl3-tools.markdown/3; source SHA-256 {digest}. Edit the Doc source. -->\n\n"
    );
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(provenance.as_bytes())?;
    file.write_all(result.as_bytes())?;
    Ok(())
}
