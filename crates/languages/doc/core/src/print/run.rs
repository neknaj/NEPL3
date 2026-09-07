use super::*;
use output::Output;
use prepare::Guest;

#[derive(Clone, Copy)]
enum List<'a> {
    Blocks(&'a [BlockRef]),
    Flows(&'a [FlowRef]),
    Inlines(&'a [InlineRef]),
    Variants(&'a [VariantRef]),
    Rows(&'a [RowRef]),
    Items(&'a [ListItemRef]),
    Sentences(&'a [SentenceRef]),
}
impl List<'_> {
    fn get(self, index: usize) -> Option<u64> {
        match self {
            Self::Blocks(v) => v.get(index).map(|v| v.0),
            Self::Flows(v) => v.get(index).map(|v| v.0),
            Self::Inlines(v) => v.get(index).map(|v| v.0),
            Self::Variants(v) => v.get(index).map(|v| v.0),
            Self::Rows(v) => v.get(index).map(|v| v.0),
            Self::Items(v) => v.get(index).map(|v| v.0),
            Self::Sentences(v) => v.get(index).map(|v| v.0),
        }
    }
}
enum Action<'a> {
    Node(u64, u64),
    List(List<'a>, usize, u64),
    Quoted(&'a str),
    Number(u64),
    Name(&'a str, u64, DocField),
    Language(&'a str, u64),
    Alignment(Alignment),
    Alignments(&'a [Alignment], usize),
    Style(ListKind),
    Check(Option<bool>),
    Target(&'a LinkTarget),
    Asset(&'a AssetRef),
    OptionalText(&'a Option<alloc::string::String>),
    OptionalNode(Option<u64>, u64),
    Guest(EmbedRef, Option<GuestLanguage>),
    Literal(u64, u64),
    Raw(&'static str),
}
fn push<'a>(
    queue: &mut Vec<Action<'a>>,
    action: Action<'a>,
    b: &mut Budget,
) -> Result<(), Failure> {
    b.charge(Resource::Work, 1)?;
    b.charge(
        Resource::AllocationUnits,
        core::mem::size_of::<Action<'a>>() as u64,
    )?;
    queue.push(action);
    Ok(())
}
fn entry(root: DocRoot) -> PrintEntry {
    match root {
        DocRoot::Article(_) => PrintEntry::Article,
        DocRoot::Body(_) => PrintEntry::Body,
        DocRoot::Block(_) => PrintEntry::Block,
        DocRoot::Flow(_) => PrintEntry::Flow,
        DocRoot::Sentence(_) => PrintEntry::Sentence,
        DocRoot::Inline(_) => PrintEntry::Inline,
        DocRoot::Variant(_) => PrintEntry::Variant,
        DocRoot::Row(_) => PrintEntry::Row,
        DocRoot::ListItem(_) => PrintEntry::ListItem,
        DocRoot::Alignment(_) => PrintEntry::Alignment,
        DocRoot::ListStyle(_) => PrintEntry::ListStyle,
        DocRoot::Check(_) => PrintEntry::Check,
        DocRoot::Target(_) => PrintEntry::LinkTarget,
        DocRoot::Asset(_) => PrintEntry::Asset,
        DocRoot::OptionalRow(_) => PrintEntry::OptionalRow,
        DocRoot::OptionalSentence(_) => PrintEntry::OptionalSentence,
        DocRoot::OptionalText(_) => PrintEntry::OptionalText,
        DocRoot::Guest(_) => PrintEntry::Guest,
        DocRoot::MathGuest(_) => PrintEntry::MathGuest,
        DocRoot::CircuitGuest(_) => PrintEntry::CircuitGuest,
    }
}
fn literal_nodes(value: &DocValue, b: &mut Budget) -> Result<Vec<bool>, Failure> {
    let shape = value.validate_shape(b)?;
    b.charge(Resource::AllocationUnits, value.nodes.len() as u64)?;
    let mut eligible = alloc::vec![false;value.nodes.len()];
    for &id in shape.postorder() {
        b.charge(Resource::Work, 1)?;
        let node = &value.nodes[id];
        let candidate = matches!(
            node.kind,
            DocKind::Text { .. }
                | DocKind::Concat { .. }
                | DocKind::Ruby { .. }
                | DocKind::Anno { .. }
                | DocKind::Sentence { .. }
        );
        let mut valid = candidate;
        let mut i = 0;
        while let Some((child, _)) = crate::check::edges::edge(&node.kind, i) {
            b.charge(Resource::Work, 1)?;
            valid &= eligible[child as usize];
            i += 1;
        }
        eligible[id] = valid;
    }
    Ok(eligible)
}
fn decimal(mut n: u64, out: &mut Output, b: &mut Budget) -> Result<(), Failure> {
    let mut bytes = [0u8; 20];
    let mut offset = 20;
    loop {
        b.charge(Resource::Work, 1)?;
        offset -= 1;
        bytes[offset] = b'0' + (n % 10) as u8;
        n /= 10;
        if n == 0 {
            break;
        }
    }
    // Every byte was constructed as an ASCII digit.
    for (index, byte) in bytes[offset..].iter().enumerate() {
        if index == 0 {
            out.start(b)?;
        }
        let ch = char::from(*byte);
        let mut buf = [0; 4];
        out.append(ch.encode_utf8(&mut buf), b)?;
    }
    Ok(())
}
pub(super) fn print(
    request: &PrintRequest,
    guests: &[Guest<'_>],
    b: &mut Budget,
) -> Result<SourceArtifact, Failure> {
    let value = &request.document.value;
    let eligible = if request.mode == PrintMode::Compact {
        literal_nodes(value, b)?
    } else {
        Vec::new()
    };
    let root = crate::check::edges::root(value.root).0;
    let mut queue = Vec::new();
    let mut out = Output::default();
    let caller = b.current_depth();
    push(&mut queue, Action::Node(root, 1), b)?;
    while let Some(action) = queue.pop() {
        b.charge(Resource::Work, 1)?;
        match action {
            Action::Quoted(s) => out.quoted(s, b)?,
            Action::Raw(s) => out.append(s, b)?,
            Action::Number(n) => decimal(n, &mut out, b)?,
            Action::Name(name, node, field) => {
                if !nepl3_core::lexical::name(name, b)? {
                    return Err(PrintFailure::UnprintableName { node, field }.into());
                }
                out.atom(name, b)?;
            }
            Action::Language(language, node) => {
                if !nepl3_core::lexical::language::well_formed(language, b)? {
                    return Err(PrintFailure::UnprintableLanguage { node }.into());
                }
                out.atom(language, b)?;
            }
            Action::List(list, index, depth) => match list.get(index) {
                None => out.atom("nil", b)?,
                Some(node) => {
                    out.atom("cons", b)?;
                    push(&mut queue, Action::List(list, index + 1, depth), b)?;
                    push(&mut queue, Action::Node(node, depth), b)?;
                }
            },
            Action::Alignment(a) => out.atom(
                match a {
                    Alignment::Default => "default",
                    Alignment::Left => "left",
                    Alignment::Center => "center",
                    Alignment::Right => "right",
                },
                b,
            )?,
            Action::Alignments(values, index) => match values.get(index) {
                None => out.atom("nil", b)?,
                Some(a) => {
                    out.atom("cons", b)?;
                    push(&mut queue, Action::Alignments(values, index + 1), b)?;
                    push(&mut queue, Action::Alignment(*a), b)?;
                }
            },
            Action::Style(style) => match style {
                ListKind::Unordered => out.atom("unordered", b)?,
                ListKind::Ordered { start } => {
                    out.atom("ordered", b)?;
                    push(&mut queue, Action::Number(start), b)?;
                }
            },
            Action::Check(v) => out.atom(
                match v {
                    None => "none",
                    Some(false) => "unchecked",
                    Some(true) => "checked",
                },
                b,
            )?,
            Action::OptionalText(v) => match v {
                None => out.atom("none", b)?,
                Some(text) => {
                    out.atom("some", b)?;
                    push(&mut queue, Action::Quoted(text), b)?;
                }
            },
            Action::OptionalNode(v, d) => match v {
                None => out.atom("none", b)?,
                Some(node) => {
                    out.atom("some", b)?;
                    push(&mut queue, Action::Node(node, d), b)?;
                }
            },
            Action::Target(target) => match target {
                LinkTarget::Page { page, fragment } => {
                    out.atom("page", b)?;
                    push(&mut queue, Action::OptionalText(fragment), b)?;
                    push(&mut queue, Action::Quoted(page), b)?;
                }
                LinkTarget::Relative { path, fragment } => {
                    out.atom("relative", b)?;
                    push(&mut queue, Action::OptionalText(fragment), b)?;
                    push(&mut queue, Action::Quoted(path), b)?;
                }
                LinkTarget::External { uri } => {
                    out.atom("external", b)?;
                    push(&mut queue, Action::Quoted(uri), b)?;
                }
            },
            Action::Asset(asset) => {
                out.atom("asset", b)?;
                out.quoted(&asset.id, b)?;
                match asset.digest {
                    None => out.atom("none", b)?,
                    Some(d) => {
                        out.atom("some", b)?;
                        out.start(b)?;
                        out.append("\"", b)?;
                        for byte in d.0 {
                            let digits = b"0123456789abcdef";
                            for digit in
                                [digits[(byte >> 4) as usize], digits[(byte & 15) as usize]]
                            {
                                let mut buf = [0; 4];
                                out.append(char::from(digit).encode_utf8(&mut buf), b)?;
                            }
                        }
                        out.append("\"", b)?;
                    }
                }
            }
            Action::Guest(id, language) => {
                let guest = usize::try_from(id.0)
                    .ok()
                    .and_then(|i| guests.get(i))
                    .ok_or(PrintFailure::UnresolvedGuest { embed: id })?;
                if language.is_some_and(|l| l != guest.language) {
                    return Err(PrintFailure::GuestCategory { embed: id }.into());
                }
                out.atom(
                    match guest.language {
                        GuestLanguage::Math => "Math",
                        GuestLanguage::Circuit => "Circuit",
                        GuestLanguage::Grammar => "Grammar",
                        GuestLanguage::Doc => "Doc",
                    },
                    b,
                )?;
                out.atom(guest.text, b)?;
            }
            Action::Literal(id, depth) => {
                b.with_depth_at_least(caller.saturating_add(depth), |b| -> Result<(), Failure> {
                    let next = depth.saturating_add(1);
                    match &value.nodes[id as usize].kind {
                        DocKind::Text { text } => out.escaped(text, true, b)?,
                        DocKind::Sentence { inlines } | DocKind::Concat { inlines } => {
                            for child in inlines.iter().rev() {
                                push(&mut queue, Action::Literal(child.0, next), b)?;
                            }
                        }
                        DocKind::Ruby { base, reading } => {
                            out.append("[", b)?;
                            push(&mut queue, Action::Raw("]"), b)?;
                            push(&mut queue, Action::Literal(reading.0, next), b)?;
                            push(&mut queue, Action::Raw("/"), b)?;
                            push(&mut queue, Action::Literal(base.0, next), b)?;
                        }
                        DocKind::Anno { base, notes } => {
                            out.append("{", b)?;
                            push(&mut queue, Action::Raw("}"), b)?;
                            for note in notes.iter().rev() {
                                push(&mut queue, Action::Literal(note.0, next), b)?;
                                push(&mut queue, Action::Raw("/"), b)?;
                            }
                            push(&mut queue, Action::Literal(base.0, next), b)?;
                        }
                        _ => return Err(PrintFailure::UnprintableLiteral { node: id }.into()),
                    }
                    Ok(())
                })?
            }
            Action::Node(id, depth) => {
                b.with_depth_at_least(caller.saturating_add(depth), |b| -> Result<(), Failure> {
                    let next = depth.saturating_add(1);
                    let node = &value.nodes[id as usize];
                    if request.mode == PrintMode::Compact
                        && eligible[id as usize]
                        && matches!(node.kind, DocKind::Sentence { .. })
                    {
                        out.start(b)?;
                        out.append("\"", b)?;
                        push(&mut queue, Action::Raw("\""), b)?;
                        push(&mut queue, Action::Literal(id, depth), b)?;
                        return Ok(());
                    }
                    macro_rules! node {
                        ($v:expr) => {
                            push(&mut queue, Action::Node($v.0, next), b)?
                        };
                    }
                    macro_rules! list {
                        ($variant:ident,$v:expr) => {
                            push(
                                &mut queue,
                                Action::List(self::List::$variant($v), 0, next),
                                b,
                            )?
                        };
                    }
                    use DocKind::*;
                    match &node.kind {
                        Guest { language, syntax } => {
                            push(&mut queue, Action::Guest(*syntax, Some(*language)), b)?
                        }
                        Article {
                            language,
                            title,
                            body,
                        } => {
                            out.atom("article", b)?;
                            node!(body);
                            node!(title);
                            push(&mut queue, Action::Language(language, id), b)?;
                        }
                        Body { blocks } => {
                            out.atom("body", b)?;
                            list!(Blocks, blocks);
                        }
                        Paragraph { items } => {
                            out.atom("paragraph", b)?;
                            list!(Flows, items);
                        }
                        Section {
                            id: name,
                            title,
                            body,
                        } => {
                            out.atom("section", b)?;
                            node!(body);
                            node!(title);
                            push(&mut queue, Action::Name(name, id, DocField::SectionId), b)?;
                        }
                        Sentence { inlines } => {
                            out.atom("sentence", b)?;
                            list!(Inlines, inlines);
                        }
                        Parallel { variants } => {
                            out.atom("parallel", b)?;
                            list!(Variants, variants);
                        }
                        Variant { language, sentence } => {
                            out.atom("variant", b)?;
                            node!(sentence);
                            push(&mut queue, Action::Language(language, id), b)?;
                        }
                        Text { text } => {
                            out.atom("text", b)?;
                            push(&mut queue, Action::Quoted(text), b)?;
                        }
                        Concat { inlines } => {
                            out.atom("concat", b)?;
                            list!(Inlines, inlines);
                        }
                        Ruby { base, reading } => {
                            out.atom("ruby", b)?;
                            node!(reading);
                            node!(base);
                        }
                        Anno { base, notes } => {
                            out.atom("anno", b)?;
                            list!(Inlines, notes);
                            node!(base);
                        }
                        InlineMath { syntax } => {
                            out.atom("math", b)?;
                            push(
                                &mut queue,
                                Action::Guest(*syntax, Some(GuestLanguage::Math)),
                                b,
                            )?;
                        }
                        Anchor { id: name, label } => {
                            out.atom("anchor", b)?;
                            node!(label);
                            push(&mut queue, Action::Name(name, id, DocField::AnchorId), b)?;
                        }
                        Reference { target, label } => {
                            out.atom("ref", b)?;
                            node!(label);
                            push(
                                &mut queue,
                                Action::Name(target, id, DocField::ReferenceTarget),
                                b,
                            )?;
                        }
                        Emphasis { inline } => {
                            out.atom("em", b)?;
                            node!(inline);
                        }
                        Strong { inline } => {
                            out.atom("strong", b)?;
                            node!(inline);
                        }
                        Break => out.atom("break", b)?,
                        DisplayMath { syntax } => {
                            out.atom("display", b)?;
                            push(
                                &mut queue,
                                Action::Guest(*syntax, Some(GuestLanguage::Math)),
                                b,
                            )?;
                        }
                        CircuitFigure { caption, syntax } => {
                            out.atom("circuit", b)?;
                            push(
                                &mut queue,
                                Action::Guest(*syntax, Some(GuestLanguage::Circuit)),
                                b,
                            )?;
                            node!(caption);
                        }
                        Code { syntax } => {
                            out.atom("code", b)?;
                            push(&mut queue, Action::Guest(*syntax, None), b)?;
                        }
                        Table {
                            columns,
                            header,
                            rows,
                        } => {
                            out.atom("table", b)?;
                            list!(Rows, rows);
                            push(
                                &mut queue,
                                Action::OptionalNode(header.map(|v| v.0), next),
                                b,
                            )?;
                            push(&mut queue, Action::Alignments(columns, 0), b)?;
                        }
                        Row { cells } => {
                            out.atom("row", b)?;
                            list!(Sentences, cells);
                        }
                        List { kind, items } => {
                            out.atom("list", b)?;
                            list!(Items, items);
                            push(&mut queue, Action::Style(*kind), b)?;
                        }
                        ListItem { checked, body } => {
                            out.atom("item", b)?;
                            node!(body);
                            push(&mut queue, Action::Check(*checked), b)?;
                        }
                        Link { target, label } => {
                            out.atom("link", b)?;
                            node!(label);
                            push(&mut queue, Action::Target(target), b)?;
                        }
                        InlineCode { text } => {
                            out.atom("code", b)?;
                            push(&mut queue, Action::Quoted(text), b)?;
                        }
                        RawCode {
                            language_hint,
                            text,
                        } => {
                            out.atom("rawcode", b)?;
                            push(&mut queue, Action::Quoted(text), b)?;
                            push(&mut queue, Action::OptionalText(language_hint), b)?;
                        }
                        Image {
                            asset,
                            alt,
                            caption,
                        } => {
                            out.atom("image", b)?;
                            push(
                                &mut queue,
                                Action::OptionalNode(caption.map(|v| v.0), next),
                                b,
                            )?;
                            node!(alt);
                            push(&mut queue, Action::Asset(asset), b)?;
                        }
                        InlineImage { asset, alt } => {
                            out.atom("image", b)?;
                            node!(alt);
                            push(&mut queue, Action::Asset(asset), b)?;
                        }
                        Alignment { alignment } => {
                            push(&mut queue, Action::Alignment(*alignment), b)?
                        }
                        ListStyle { style } => push(&mut queue, Action::Style(*style), b)?,
                        Check { checked } => push(&mut queue, Action::Check(*checked), b)?,
                        Target { target } => push(&mut queue, Action::Target(target), b)?,
                        Asset { asset } => push(&mut queue, Action::Asset(asset), b)?,
                        OptionalRow { row } => {
                            push(&mut queue, Action::OptionalNode(row.map(|v| v.0), next), b)?
                        }
                        OptionalSentence { sentence } => push(
                            &mut queue,
                            Action::OptionalNode(sentence.map(|v| v.0), next),
                            b,
                        )?,
                        OptionalText { text } => push(&mut queue, Action::OptionalText(text), b)?,
                    }
                    Ok(())
                })?
            }
        }
    }
    Ok(SourceArtifact {
        text: out.text,
        entry: entry(value.root),
    })
}
