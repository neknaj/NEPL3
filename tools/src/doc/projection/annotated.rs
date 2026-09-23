//! Explicit all-notes Markdown viewing profile. This is not a Doc roundtrip
//! encoding or proof of legacy platform anchor/accessibility compatibility.
use super::*;
use nepl3_core::source::Digest;
use serde::Deserialize;
mod blocks;
pub mod host;
pub mod pages;

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
    /// Structural insertion boundary after the article heading and separator.
    pub(crate) title_end: usize,
}

impl Artifact {
    /// Add repository provenance at the boundary recorded by the renderer.
    /// Canonical registry paths use the portable ASCII path alphabet.
    pub(crate) fn source_link(&mut self, href: &str, budget: &mut Budget) -> Result<(), Error> {
        budget.charge(Resource::Work, href.len() as u64 + 1)?;
        if href.is_empty()
            || !href
                .bytes()
                .all(|c| c.is_ascii_alphanumeric() || b"/._-".contains(&c))
        {
            return Err(Error::Invalid("invalid canonical source link".into()));
        }
        let size = href
            .len()
            .checked_mul(2)
            .and_then(|n| n.checked_add(128))
            .and_then(|n| n.checked_add(self.markdown.len()))
            .ok_or_else(|| budget.stop(StopReason::AllocationLimit))?;
        budget.charge(Resource::Work, size as u64)?;
        budget.charge(Resource::AllocationUnits, size as u64)?;
        let link = format!("[正本（NEPL3d）](<{href}>)\n\n");
        budget.charge(Resource::OutputBytes, link.len() as u64)?;
        if self.markdown.len() + link.len() > 1024 * 1024 {
            return Err(Error::OutputLimit);
        }
        self.markdown.reserve_exact(link.len());
        self.markdown.insert_str(self.title_end, &link);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_link_stops_before_mutating_the_artifact() {
        for reason in [
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::OutputLimit,
        ] {
            let mut limits = crate::doc::source::budget().limits();
            match reason {
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::OutputLimit => limits.output_bytes = 0,
                _ => unreachable!(),
            }
            let mut budget = Budget::new(limits);
            let mut artifact = Artifact {
                markdown: "# T\n\nBody\n".into(),
                title_end: 5,
                document_digest: Digest::of(b"fixture"),
            };
            assert!(
                matches!(artifact.source_link("../source.nepld", &mut budget), Err(Error::Stopped(actual)) if actual == reason)
            );
            assert_eq!(artifact.markdown, "# T\n\nBody\n");
            assert!(budget.poll().is_err());
        }
    }
}

/// Render one checked Article with external links. Cross-page links, assets
/// and foreign operations still require a page-set/host preparation path.
/// Ruby uses inline HTML `ruby`/`rt`, Anno displays `base{note1/note2}`; nested
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
        check_pending(requirement, budget)?;
    }
    let contents = Contents::prepare(document, registry, codec, budget)?;
    Ok(render_resolved(
        document,
        &contents,
        budget,
        aliases,
        &[],
        plan.document_digest,
    )?
    .0)
}

pub(super) fn check_pending(
    requirement: &prepare::DocRequirement,
    budget: &mut Budget,
) -> Result<(), Error> {
    budget.charge(Resource::Work, 1)?;
    match requirement {
        prepare::DocRequirement::Foreign {
            kind: EmbedKind::Sentence | EmbedKind::SentenceInline,
            ..
        } => {}
        _ => return Err(Error::NeedsResolution),
    }
    Ok(())
}

// Private: callers have either inspected this document or resolved its exact
// borrowed PageSet. A decoded plan/digest is never an admission proof.
fn render_resolved(
    document: &DocumentSyntax,
    contents: &Contents,
    budget: &mut Budget,
    aliases: &[Alias],
    links: &[(u64, String)],
    document_digest: Digest,
) -> Result<(Artifact, Vec<String>), Error> {
    budget.poll()?;
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
            contents,
            budget,
            output: String::new(),
        },
        aliases,
        links,
        emitted: Vec::new(),
    };
    let DocKind::Article { title, body, .. } = writer.plain.kind(root.0) else {
        return Err(Error::Unsupported { node: root.0 });
    };
    writer.aliases(None)?;
    writer.plain.emit("# ")?;
    writer.sentences(&[title.0], None)?;
    writer.plain.emit("\n\n")?;
    let title_end = writer.plain.output.len();
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
    Ok((
        Artifact {
            title_end,
            markdown: writer.plain.output,
            document_digest,
        },
        writer.emitted,
    ))
}

struct Annotated<'a, 'b> {
    plain: Writer<'a, 'b>,
    aliases: &'a [Alias],
    links: &'a [(u64, String)],
    emitted: Vec<String>,
}
#[derive(Clone, Copy)]
enum Piece<'a> {
    Text(Position, &'a str),
    Code(Position, &'a str),
    Break(Position),
    Tag(&'static str),
    /// Ruby base/reading boundaries separate adjacent code spans.
    RubyTag(&'static str),
    LinkStart(Position),
    LinkEnd(Position, &'a str),
}
enum Task<'a> {
    Node(u64, u64),
    Sentence(EmbedRef, u64, u64),
    Piece(Piece<'a>),
}
pub(super) fn push<T>(items: &mut Vec<T>, item: T, budget: &mut Budget) -> Result<(), Error> {
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
        self.inline(sentences, continuation, false)
    }
    fn inline(
        &mut self,
        sentences: &[u64],
        continuation: Option<&str>,
        table_cell: bool,
    ) -> Result<(), Error> {
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
                        DocKind::Sentence { syntax } => {
                            let (_, root) = self.plain.contents.sentence(*syntax)?;
                            push(
                                &mut stack,
                                Task::Sentence(*syntax, root, next),
                                self.plain.budget,
                            )?;
                        }
                        DocKind::Link { label, .. } => {
                            let mut href = None;
                            for (actual, value) in self.links {
                                self.plain.budget.charge(Resource::Work, 1)?;
                                if *actual == node {
                                    href = Some(value.as_str());
                                    break;
                                }
                            }
                            let uri = href.ok_or(Error::NeedsResolution)?;
                            let sentence = self.plain.contents.get(*label)?;
                            let nepl3_sentence_core::model::Root::Inline(root) =
                                sentence.value.root
                            else {
                                return Err(Error::NeedsResolution);
                            };
                            push(
                                &mut stack,
                                Task::Piece(Piece::LinkEnd(Position::Doc(node), uri)),
                                self.plain.budget,
                            )?;
                            push(
                                &mut stack,
                                Task::Sentence(*label, root.0, next),
                                self.plain.budget,
                            )?;
                            push(
                                &mut stack,
                                Task::Piece(Piece::LinkStart(Position::Doc(node))),
                                self.plain.budget,
                            )?;
                        }
                        _ => return Err(Error::Unsupported { node }),
                    }
                }
                Task::Sentence(embed, node, depth) => {
                    self.plain.budget.observe_depth(depth)?;
                    self.plain.budget.charge(Resource::Work, 1)?;
                    let next = depth
                        .checked_add(1)
                        .ok_or_else(|| self.plain.budget.stop(StopReason::DepthLimit))?;
                    let sentence = self.plain.contents.get(embed)?;
                    let kind = &sentence.value.nodes[node as usize];
                    let position = Position::Sentence { embed, node };
                    match kind {
                        SentenceKind::Sentence { inlines } | SentenceKind::Concat { inlines } => {
                            for child in inlines.iter().rev() {
                                push(
                                    &mut stack,
                                    Task::Sentence(embed, child.0, next),
                                    self.plain.budget,
                                )?;
                            }
                        }
                        SentenceKind::Text { text } if text.is_empty() => {}
                        SentenceKind::Text { text } => {
                            push(&mut pieces, Piece::Text(position, text), self.plain.budget)?
                        }
                        SentenceKind::Code { text } => {
                            push(&mut pieces, Piece::Code(position, text), self.plain.budget)?
                        }
                        SentenceKind::Break => {
                            push(&mut pieces, Piece::Break(position), self.plain.budget)?
                        }
                        SentenceKind::Ruby { base, reading } => {
                            for task in [
                                Task::Piece(Piece::RubyTag("</rt></ruby>")),
                                Task::Sentence(embed, reading.0, next),
                                Task::Piece(Piece::RubyTag("<rt>")),
                                Task::Sentence(embed, base.0, next),
                                Task::Piece(Piece::Tag("<ruby>")),
                            ] {
                                push(&mut stack, task, self.plain.budget)?;
                            }
                        }
                        SentenceKind::InlineAnno { base, notes } => {
                            push(
                                &mut stack,
                                Task::Piece(Piece::Text(position, "}")),
                                self.plain.budget,
                            )?;
                            for (index, note) in notes.iter().enumerate().rev() {
                                push(
                                    &mut stack,
                                    Task::Sentence(embed, note.0, next),
                                    self.plain.budget,
                                )?;
                                if index != 0 {
                                    push(
                                        &mut stack,
                                        Task::Piece(Piece::Text(position, "/")),
                                        self.plain.budget,
                                    )?;
                                }
                            }
                            push(
                                &mut stack,
                                Task::Piece(Piece::Text(position, "{")),
                                self.plain.budget,
                            )?;
                            push(
                                &mut stack,
                                Task::Sentence(embed, base.0, next),
                                self.plain.budget,
                            )?;
                        }
                        SentenceKind::Strong { inline } | SentenceKind::Emphasis { inline } => {
                            let strong = matches!(kind, SentenceKind::Strong { .. });
                            push(
                                &mut stack,
                                Task::Piece(Piece::Tag(if strong { "</strong>" } else { "</em>" })),
                                self.plain.budget,
                            )?;
                            push(
                                &mut stack,
                                Task::Sentence(embed, inline.0, next),
                                self.plain.budget,
                            )?;
                            push(
                                &mut stack,
                                Task::Piece(Piece::Tag(if strong { "<strong>" } else { "<em>" })),
                                self.plain.budget,
                            )?;
                        }
                        SentenceKind::ExternalLink { uri, label } => {
                            if !nepl3_markup::html::external_uri(uri, self.plain.budget)? {
                                return Err(position.text());
                            }
                            push(
                                &mut stack,
                                Task::Piece(Piece::LinkEnd(position, uri)),
                                self.plain.budget,
                            )?;
                            push(
                                &mut stack,
                                Task::Sentence(embed, label.0, next),
                                self.plain.budget,
                            )?;
                            push(
                                &mut stack,
                                Task::Piece(Piece::LinkStart(position)),
                                self.plain.budget,
                            )?;
                        }
                        _ => return Err(position.unsupported()),
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
            // An empty table cell is representable. Do not generalize this to
            // empty headings/paragraphs or silently drop empty decorations.
            if table_cell && pieces.is_empty() {
                return Ok(());
            }
            return Err(Error::Unsupported {
                node: sentences.first().copied().unwrap_or(0),
            });
        }
        if let Some(Piece::Text(position, text)) = visible.first()
            && text.starts_with(char::is_whitespace)
        {
            return Err(position.text());
        }
        if let Some(Piece::Text(position, text)) = visible.last()
            && text.ends_with(char::is_whitespace)
        {
            return Err(position.text());
        }
        for pair in visible.windows(2) {
            self.plain.budget.charge(Resource::Work, 1)?;
            if matches!(pair[0], Piece::Code(_, _))
                && let Piece::Code(node, _) = pair[1]
            {
                return Err(node.unsupported());
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
                return Err(node.unsupported());
            }
        }
        let mut previous_code = false;
        let mut link = None;
        for piece in pieces {
            match piece {
                Piece::Text(node, text) => {
                    self.plain
                        .text(node.node(), text, false)
                        .map_err(|e| node.map(e))?;
                    previous_code = false;
                }
                Piece::Code(node, text) => {
                    if previous_code {
                        return Err(node.unsupported());
                    }
                    if table_cell {
                        self.table_code(node.node(), text)
                            .map_err(|e| node.map(e))?;
                    } else {
                        self.plain
                            .text(node.node(), text, true)
                            .map_err(|e| node.map(e))?;
                    }
                    previous_code = true;
                }
                Piece::Tag(tag) | Piece::RubyTag(tag) => {
                    self.plain.emit(tag)?;
                    previous_code = false;
                }
                Piece::LinkStart(node) => {
                    if link.is_some() {
                        return Err(node.unsupported());
                    }
                    link = Some(node);
                    self.plain.emit("[")?;
                    previous_code = false;
                }
                Piece::LinkEnd(node, uri) => {
                    if link != Some(node) {
                        return Err(node.unsupported());
                    }
                    link = None;
                    self.plain.emit("](<")?;
                    // Destinations also decode Markdown escapes/entities.
                    // Escape punctuation so a literal &amp; stays &amp;.
                    self.plain
                        .text(node.node(), uri, false)
                        .map_err(|e| node.map(e))?;
                    self.plain.emit(">)")?;
                    previous_code = false;
                }
                Piece::Break(node) => {
                    if link.is_some() {
                        return Err(node.unsupported());
                    }
                    self.plain.emit("\\\n")?;
                    self.plain
                        .emit(continuation.ok_or_else(|| node.unsupported())?)?;
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
        let mut previous_list = false;
        for child in blocks {
            self.plain.budget.charge(Resource::Work, 1)?;
            let is_list = matches!(self.plain.kind(child.0), DocKind::List { .. });
            // CommonMark example308: blank lines alone merge same-kind lists.
            // A fixed block comment preserves the boundary without adding
            // visible content or admitting source-controlled raw HTML.
            if previous_list && is_list {
                self.plain.emit("<!-- -->\n\n")?;
            }
            previous_list = is_list;
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
                DocKind::List { kind, items } => self.flat_list(child.0, *kind, items)?,
                DocKind::Table {
                    columns,
                    header,
                    rows,
                } => {
                    self.table(child.0, columns, *header, rows)?;
                }
                _ => return Err(Error::Unsupported { node: child.0 }),
            }
        }
        Ok(())
    }
}
