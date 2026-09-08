//! Explicit all-notes Markdown viewing profile. This is not a Doc roundtrip
//! encoding or proof of legacy platform anchor/accessibility compatibility.
use super::*;
use nepl3_core::source::Digest;
use serde::Deserialize;
pub mod host;

/// An explicitly supplied compatibility anchor; None selects the article.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Alias {
    pub section: Option<String>,
    pub name: String,
}
#[derive(Debug)]
pub struct Artifact {
    pub markdown: String,
    pub document_digest: Digest,
}

/// Render one checked Article with external links. Cross-page links, assets
/// and foreign operations still require a page-set/host preparation path.
/// Ruby is displayed as base[reading], Anno as base{note1/note2}; nested
/// content is walked structurally, including code and semantic decorations.
pub fn render<C: FoundationValueCodec>(
    document: &DocumentSyntax,
    registry: &SchemaRegistry,
    codec: &mut C,
    budget: &mut Budget,
    aliases: &[Alias],
) -> Result<Artifact, Error>
where
    C::Error: core::fmt::Debug,
{
    let plan = prepare::inspect(document, registry, codec, budget).map_err(|e| match e {
        prepare::PreparationError::Stopped(s) => Error::Stopped(s),
        e => Error::Invalid(format!("{e:?}")),
    })?;
    for requirement in &plan.requirements {
        budget.charge(Resource::Work, 1)?;
        match requirement {
            prepare::DocRequirement::Link {
                node,
                target: LinkTarget::External { uri },
            } => {
                if !nepl3_markup::html::external_uri(uri, budget)? {
                    return Err(Error::Text { node: *node });
                }
            }
            _ => return Err(Error::NeedsResolution),
        }
    }
    let DocRoot::Article(root) = document.value.root else {
        return Err(Error::Unsupported { node: 0 });
    };
    for (index, alias) in aliases.iter().enumerate() {
        budget.charge(Resource::Work, alias.name.len() as u64 + 1)?;
        if alias.name.is_empty()
            || !alias
                .name
                .chars()
                .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_'))
        {
            return Err(Error::Invalid("invalid compatibility anchor".into()));
        }
        for prior in &aliases[..index] {
            budget.charge(Resource::Work, (prior.name.len() + alias.name.len()) as u64)?;
            if prior.name == alias.name {
                return Err(Error::Invalid("duplicate compatibility anchor".into()));
            }
        }
        if let Some(id) = &alias.section {
            let mut found = false;
            for entry in &document.value.nodes {
                budget.charge(Resource::Work, id.len() as u64 + 1)?;
                if matches!(&entry.kind, DocKind::Section { id: actual, .. } if actual == id) {
                    found = true;
                }
            }
            if !found {
                return Err(Error::Invalid("missing compatibility section".into()));
            }
        }
    }
    let mut writer = Annotated {
        plain: Writer {
            doc: document,
            budget,
            output: String::new(),
        },
        aliases,
        emitted: Vec::new(),
    };
    let DocKind::Article { title, body, .. } = writer.plain.kind(root.0) else {
        return Err(Error::Unsupported { node: root.0 });
    };
    writer.aliases(None)?;
    writer.plain.emit("# ")?;
    writer.sentences(&[title.0], None)?;
    writer.plain.emit("\n\n")?;
    writer.body(body.0, 1)?;
    // An alias for an unreachable section must not silently disappear.
    for alias in aliases {
        writer
            .plain
            .budget
            .charge(Resource::Work, alias.name.len() as u64 + 1)?;
        let mut found = false;
        for emitted in &writer.emitted {
            writer.plain.budget.charge(
                Resource::Work,
                (emitted.len() + alias.name.len()) as u64 + 1,
            )?;
            if emitted == &alias.name {
                found = true;
                break;
            }
        }
        if !found {
            return Err(Error::Invalid("compatibility anchor not emitted".into()));
        }
    }
    Ok(Artifact {
        markdown: writer.plain.output,
        document_digest: plan.document_digest,
    })
}

struct Annotated<'a, 'b> {
    plain: Writer<'a, 'b>,
    aliases: &'a [Alias],
    emitted: Vec<String>,
}
#[derive(Clone, Copy)]
enum Piece<'a> {
    Text(u64, &'a str),
    Code(u64, &'a str),
    Break(u64),
    Tag(&'static str),
    LinkStart(u64),
    LinkEnd(u64, &'a str),
}
enum Task<'a> {
    Node(u64, u64),
    Piece(Piece<'a>),
}
fn push<T>(items: &mut Vec<T>, item: T, budget: &mut Budget) -> Result<(), Error> {
    budget.charge(Resource::Work, 1)?;
    budget.charge(Resource::Nodes, 1)?;
    if items.len() == items.capacity() {
        let capacity = items.capacity().saturating_mul(2).max(1);
        let bytes = capacity
            .checked_mul(core::mem::size_of::<T>())
            .filter(|n| *n <= isize::MAX as usize)
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        budget.charge(Resource::AllocationUnits, bytes as u64)?;
        items.reserve_exact(capacity - items.len());
    }
    items.push(item);
    Ok(())
}
impl<'a> Annotated<'a, '_> {
    fn anchor(&mut self, name: &str) -> Result<(), Error> {
        for prior in &self.emitted {
            self.plain
                .budget
                .charge(Resource::Work, (prior.len() + name.len()) as u64)?;
            if prior == name {
                return Err(Error::Invalid("duplicate emitted anchor".into()));
            }
        }
        self.plain
            .budget
            .charge(Resource::AllocationUnits, name.len() as u64)?;
        push(&mut self.emitted, name.into(), self.plain.budget)?;
        self.plain.emit("<a name=\"")?;
        self.plain.emit(name)?; // restricted aliases or generated ASCII hex only
        self.plain.emit("\"></a>\n\n")
    }
    fn aliases(&mut self, section: Option<&str>) -> Result<(), Error> {
        for alias in self.aliases {
            self.plain.budget.charge(
                Resource::Work,
                alias.section.as_ref().map_or(1, |s| s.len() as u64 + 1),
            )?;
            if alias.section.as_deref() == section {
                self.anchor(&alias.name)?;
            }
        }
        Ok(())
    }
    fn section_anchor(&mut self, id: &str) -> Result<(), Error> {
        let capacity = id
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(2))
            .ok_or_else(|| self.plain.budget.stop(StopReason::AllocationLimit))?;
        self.plain
            .budget
            .charge(Resource::AllocationUnits, capacity as u64)?;
        self.plain.budget.charge(Resource::Work, capacity as u64)?;
        let mut name = String::with_capacity(capacity);
        name.push_str("n-");
        for byte in id.bytes() {
            name.push(b"0123456789abcdef"[(byte >> 4) as usize] as char);
            name.push(b"0123456789abcdef"[(byte & 15) as usize] as char);
        }
        self.anchor(&name)?;
        self.aliases(Some(id))
    }
    fn sentences(&mut self, sentences: &[u64], continuation: Option<&str>) -> Result<(), Error> {
        let mut pieces = Vec::new();
        let mut stack = Vec::new();
        for &sentence in sentences.iter().rev() {
            push(&mut stack, Task::Node(sentence, 1), self.plain.budget)?;
        }
        while let Some(task) = stack.pop() {
            match task {
                Task::Piece(piece) => push(&mut pieces, piece, self.plain.budget)?,
                Task::Node(node, depth) => {
                    self.plain.budget.observe_depth(depth)?;
                    self.plain.budget.charge(Resource::Work, 1)?;
                    let next = depth
                        .checked_add(1)
                        .ok_or_else(|| self.plain.budget.stop(StopReason::DepthLimit))?;
                    match self.plain.kind(node) {
                        DocKind::Sentence { inlines } | DocKind::Concat { inlines } => {
                            for child in inlines.iter().rev() {
                                push(&mut stack, Task::Node(child.0, next), self.plain.budget)?;
                            }
                        }
                        DocKind::Text { text } if text.is_empty() => {}
                        DocKind::Text { text } => {
                            push(&mut pieces, Piece::Text(node, text), self.plain.budget)?
                        }
                        DocKind::InlineCode { text } => {
                            push(&mut pieces, Piece::Code(node, text), self.plain.budget)?
                        }
                        DocKind::Break => push(&mut pieces, Piece::Break(node), self.plain.budget)?,
                        DocKind::Ruby { base, reading } => {
                            for task in [
                                Task::Piece(Piece::Text(node, "]")),
                                Task::Node(reading.0, next),
                                Task::Piece(Piece::Text(node, "[")),
                                Task::Node(base.0, next),
                            ] {
                                push(&mut stack, task, self.plain.budget)?;
                            }
                        }
                        DocKind::Anno { base, notes } => {
                            push(
                                &mut stack,
                                Task::Piece(Piece::Text(node, "}")),
                                self.plain.budget,
                            )?;
                            for (index, note) in notes.iter().enumerate().rev() {
                                push(&mut stack, Task::Node(note.0, next), self.plain.budget)?;
                                if index != 0 {
                                    push(
                                        &mut stack,
                                        Task::Piece(Piece::Text(node, "/")),
                                        self.plain.budget,
                                    )?;
                                }
                            }
                            push(
                                &mut stack,
                                Task::Piece(Piece::Text(node, "{")),
                                self.plain.budget,
                            )?;
                            push(&mut stack, Task::Node(base.0, next), self.plain.budget)?;
                        }
                        DocKind::Strong { inline } | DocKind::Emphasis { inline } => {
                            let strong = matches!(self.plain.kind(node), DocKind::Strong { .. });
                            push(
                                &mut stack,
                                Task::Piece(Piece::Tag(if strong { "</strong>" } else { "</em>" })),
                                self.plain.budget,
                            )?;
                            push(&mut stack, Task::Node(inline.0, next), self.plain.budget)?;
                            push(
                                &mut stack,
                                Task::Piece(Piece::Tag(if strong { "<strong>" } else { "<em>" })),
                                self.plain.budget,
                            )?;
                        }
                        DocKind::Link {
                            target: LinkTarget::External { uri },
                            label,
                        } => {
                            push(
                                &mut stack,
                                Task::Piece(Piece::LinkEnd(node, uri)),
                                self.plain.budget,
                            )?;
                            push(&mut stack, Task::Node(label.0, next), self.plain.budget)?;
                            push(
                                &mut stack,
                                Task::Piece(Piece::LinkStart(node)),
                                self.plain.budget,
                            )?;
                        }
                        _ => return Err(Error::Unsupported { node }),
                    }
                }
            }
        }
        let mut visible = Vec::new();
        for piece in &pieces {
            if !matches!(
                piece,
                Piece::Tag(_) | Piece::LinkStart(_) | Piece::LinkEnd(_, _)
            ) {
                push(&mut visible, piece, self.plain.budget)?;
            }
        }
        // Boundary validation uses the flattened visible stream, so transparent
        // containers cannot hide whitespace, adjacent code or an edge break.
        if visible.is_empty() {
            return Err(Error::Unsupported {
                node: sentences.first().copied().unwrap_or(0),
            });
        }
        if matches!(visible.first(), Some(Piece::Text(_, s)) if s.starts_with(char::is_whitespace))
            || matches!(visible.last(), Some(Piece::Text(_, s)) if s.ends_with(char::is_whitespace))
        {
            return Err(Error::Text { node: sentences[0] });
        }
        for pair in visible.windows(2) {
            self.plain.budget.charge(Resource::Work, 1)?;
            if matches!(pair[0], Piece::Code(_, _))
                && let Piece::Code(node, _) = pair[1]
            {
                return Err(Error::Unsupported { node: *node });
            }
        }
        for (index, piece) in visible.iter().enumerate() {
            if let Piece::Break(node) = piece
                && (continuation.is_none()
                    || index == 0
                    || index + 1 == visible.len()
                    || matches!(visible[index - 1], Piece::Break(_))
                    || matches!(visible[index + 1], Piece::Break(_))
                    || matches!(visible[index - 1], Piece::Text(_, s) if s.ends_with(char::is_whitespace))
                    || matches!(visible[index + 1], Piece::Text(_, s) if s.starts_with(char::is_whitespace)))
            {
                return Err(Error::Unsupported { node: *node });
            }
        }
        let mut previous_code = false;
        let mut link = None;
        for piece in pieces {
            match piece {
                Piece::Text(node, text) => {
                    self.plain.text(node, text, false)?;
                    previous_code = false;
                }
                Piece::Code(node, text) => {
                    if previous_code {
                        return Err(Error::Unsupported { node });
                    }
                    self.plain.text(node, text, true)?;
                    previous_code = true;
                }
                Piece::Tag(tag) => {
                    self.plain.emit(tag)?;
                    previous_code = false;
                }
                Piece::LinkStart(node) => {
                    if link.is_some() {
                        return Err(Error::Unsupported { node });
                    }
                    link = Some(node);
                    self.plain.emit("[")?;
                    previous_code = false;
                }
                Piece::LinkEnd(node, uri) => {
                    if link != Some(node) {
                        return Err(Error::Unsupported { node });
                    }
                    link = None;
                    self.plain.emit("](<")?;
                    // Destinations also decode Markdown escapes/entities.
                    // Escape punctuation so a literal &amp; stays &amp;.
                    self.plain.text(node, uri, false)?;
                    self.plain.emit(">)")?;
                    previous_code = false;
                }
                Piece::Break(node) => {
                    if link.is_some() {
                        return Err(Error::Unsupported { node });
                    }
                    self.plain.emit("\\\n")?;
                    self.plain
                        .emit(continuation.ok_or(Error::Unsupported { node: 0 })?)?;
                    previous_code = false;
                }
            }
        }
        Ok(())
    }
    fn body(&mut self, node: u64, level: usize) -> Result<(), Error> {
        self.plain.budget.charge(Resource::Work, 1)?;
        let DocKind::Body { blocks } = self.plain.kind(node) else {
            return Err(Error::Unsupported { node });
        };
        let mut section_seen = false;
        for child in blocks {
            self.plain.budget.charge(Resource::Work, 1)?;
            match self.plain.kind(child.0) {
                DocKind::Section { id, title, body } if level < 6 => {
                    section_seen = true;
                    self.section_anchor(id)?;
                    for _ in 0..=level {
                        self.plain.emit("#")?;
                    }
                    self.plain.emit(" ")?;
                    self.sentences(&[title.0], None)?;
                    self.plain.emit("\n\n")?;
                    self.body(body.0, level + 1)?;
                }
                _ if section_seen => return Err(Error::Unsupported { node: child.0 }),
                DocKind::Paragraph { items } => {
                    let mut sentences = Vec::new();
                    for item in items {
                        if !matches!(self.plain.kind(item.0), DocKind::Sentence { .. }) {
                            return Err(Error::Unsupported { node: item.0 });
                        }
                        push(&mut sentences, item.0, self.plain.budget)?;
                    }
                    self.sentences(&sentences, Some(""))?;
                    self.plain.emit("\n\n")?;
                }
                DocKind::RawCode {
                    language_hint,
                    text,
                } => self
                    .plain
                    .raw_code(child.0, language_hint.as_deref(), text)?,
                _ => return Err(Error::Unsupported { node: child.0 }),
            }
        }
        Ok(())
    }
}
