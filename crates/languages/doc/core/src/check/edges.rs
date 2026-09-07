use super::Category;
use crate::model::{DocKind, DocRoot};
pub(crate) fn root(root: DocRoot) -> (u64, Category) {
    match root {
        DocRoot::Article(v) => (v.0, Category::Article),
        DocRoot::Body(v) => (v.0, Category::Body),
        DocRoot::Block(v) => (v.0, Category::Block),
        DocRoot::Flow(v) => (v.0, Category::Flow),
        DocRoot::Sentence(v) => (v.0, Category::Sentence),
        DocRoot::Inline(v) => (v.0, Category::Inline),
        DocRoot::Variant(v) => (v.0, Category::Variant),
        DocRoot::Row(v) => (v.0, Category::Row),
        DocRoot::ListItem(v) => (v.0, Category::ListItem),
        DocRoot::Alignment(v) => (v.0, Category::Alignment),
        DocRoot::ListStyle(v) => (v.0, Category::ListStyle),
        DocRoot::Check(v) => (v.0, Category::Check),
        DocRoot::Target(v) => (v.0, Category::Target),
        DocRoot::Asset(v) => (v.0, Category::Asset),
        DocRoot::OptionalRow(v) => (v.0, Category::OptionalRow),
        DocRoot::OptionalSentence(v) => (v.0, Category::OptionalSentence),
        DocRoot::OptionalText(v) => (v.0, Category::OptionalText),
    }
}
pub(crate) fn accepts(kind: &DocKind, category: Category) -> bool {
    use DocKind::*;
    match category {
        Category::Alignment => matches!(kind, Alignment { .. }),
        Category::ListStyle => matches!(kind, ListStyle { .. }),
        Category::Check => matches!(kind, Check { .. }),
        Category::Target => matches!(kind, Target { .. }),
        Category::Asset => matches!(kind, Asset { .. }),
        Category::OptionalRow => matches!(kind, OptionalRow { .. }),
        Category::OptionalSentence => matches!(kind, OptionalSentence { .. }),
        Category::OptionalText => matches!(kind, OptionalText { .. }),
        Category::Article => matches!(kind, Article { .. }),
        Category::Body => matches!(kind, Body { .. }),
        Category::Block => matches!(
            kind,
            Paragraph { .. }
                | Section { .. }
                | DisplayMath { .. }
                | CircuitFigure { .. }
                | Code { .. }
                | Table { .. }
                | List { .. }
                | RawCode { .. }
                | Image { .. }
        ),
        Category::Flow => {
            matches!(kind, Sentence { .. } | Parallel { .. }) || accepts(kind, Category::Block)
        }
        Category::Sentence => matches!(kind, Sentence { .. }),
        Category::Inline => matches!(
            kind,
            Text { .. }
                | Concat { .. }
                | Ruby { .. }
                | Anno { .. }
                | InlineMath { .. }
                | Anchor { .. }
                | Reference { .. }
                | Emphasis { .. }
                | Strong { .. }
                | Break
                | Link { .. }
                | InlineCode { .. }
                | InlineImage { .. }
        ),
        Category::Variant => matches!(kind, Variant { .. }),
        Category::Row => matches!(kind, Row { .. }),
        Category::ListItem => matches!(kind, ListItem { .. }),
    }
}
/// Indexed access avoids cloning child lists or allocating on every visit.
pub(crate) fn edge(kind: &DocKind, index: usize) -> Option<(u64, Category)> {
    use DocKind::*;
    match kind {
        OptionalRow { row } => {
            if index == 0 {
                row.map(|r| (r.0, Category::Row))
            } else {
                None
            }
        }
        OptionalSentence { sentence } => {
            if index == 0 {
                sentence.map(|r| (r.0, Category::Sentence))
            } else {
                None
            }
        }
        Alignment { .. }
        | ListStyle { .. }
        | Check { .. }
        | Target { .. }
        | Asset { .. }
        | OptionalText { .. } => None,
        Article { title, body, .. } | Section { title, body, .. } => match index {
            0 => Some((title.0, Category::Sentence)),
            1 => Some((body.0, Category::Body)),
            _ => None,
        },
        Body { blocks } => blocks.get(index).map(|v| (v.0, Category::Block)),
        Paragraph { items } => items.get(index).map(|v| (v.0, Category::Flow)),
        Sentence { inlines } | Concat { inlines } => {
            inlines.get(index).map(|v| (v.0, Category::Inline))
        }
        Parallel { variants } => variants.get(index).map(|v| (v.0, Category::Variant)),
        Variant { sentence, .. } => (index == 0).then_some((sentence.0, Category::Sentence)),
        Ruby { base, reading } => match index {
            0 => Some((base.0, Category::Inline)),
            1 => Some((reading.0, Category::Inline)),
            _ => None,
        },
        Anno { base, notes } => {
            if index == 0 {
                Some((base.0, Category::Inline))
            } else {
                notes.get(index - 1).map(|v| (v.0, Category::Inline))
            }
        }
        Anchor { label, .. } | Reference { label, .. } | Link { label, .. } => {
            (index == 0).then_some((label.0, Category::Inline))
        }
        Emphasis { inline } | Strong { inline } => {
            (index == 0).then_some((inline.0, Category::Inline))
        }
        CircuitFigure { caption, .. } => (index == 0).then_some((caption.0, Category::Sentence)),
        Table { header, rows, .. } => match header {
            Some(v) if index == 0 => Some((v.0, Category::Row)),
            Some(_) => rows.get(index - 1).map(|v| (v.0, Category::Row)),
            None => rows.get(index).map(|v| (v.0, Category::Row)),
        },
        Row { cells } => cells.get(index).map(|v| (v.0, Category::Sentence)),
        List { items, .. } => items.get(index).map(|v| (v.0, Category::ListItem)),
        ListItem { body, .. } => (index == 0).then_some((body.0, Category::Body)),
        Image { alt, caption, .. } => match index {
            0 => Some((alt.0, Category::Sentence)),
            1 => caption.map(|v| (v.0, Category::Sentence)),
            _ => None,
        },
        InlineImage { alt, .. } => (index == 0).then_some((alt.0, Category::Sentence)),
        Text { .. }
        | InlineMath { .. }
        | Break
        | DisplayMath { .. }
        | Code { .. }
        | InlineCode { .. }
        | RawCode { .. } => None,
    }
}

pub(crate) fn rewrite(
    kind: &mut DocKind,
    mut map: impl FnMut(u64) -> Result<u64, super::ShapeError>,
) -> Result<(), super::ShapeError> {
    match kind {
        DocKind::OptionalRow { row } => {
            if let Some(row) = row {
                row.0 = map(row.0)?;
            }
        }
        DocKind::OptionalSentence { sentence } => {
            if let Some(sentence) = sentence {
                sentence.0 = map(sentence.0)?;
            }
        }
        DocKind::Alignment { .. }
        | DocKind::ListStyle { .. }
        | DocKind::Check { .. }
        | DocKind::Target { .. }
        | DocKind::Asset { .. }
        | DocKind::OptionalText { .. } => {}
        DocKind::Article { title, body, .. } => {
            title.0 = map(title.0)?;
            body.0 = map(body.0)?;
        }
        DocKind::Body { blocks } => {
            for item in blocks {
                item.0 = map(item.0)?;
            }
        }
        DocKind::Paragraph { items } => {
            for item in items {
                item.0 = map(item.0)?;
            }
        }
        DocKind::Section { title, body, .. } => {
            title.0 = map(title.0)?;
            body.0 = map(body.0)?;
        }
        DocKind::Sentence { inlines } => {
            for item in inlines {
                item.0 = map(item.0)?;
            }
        }
        DocKind::Parallel { variants } => {
            for item in variants {
                item.0 = map(item.0)?;
            }
        }
        DocKind::Variant { sentence, .. } => {
            sentence.0 = map(sentence.0)?;
        }
        DocKind::Text { .. } => {}
        DocKind::Concat { inlines } => {
            for item in inlines {
                item.0 = map(item.0)?;
            }
        }
        DocKind::Ruby { base, reading } => {
            base.0 = map(base.0)?;
            reading.0 = map(reading.0)?;
        }
        DocKind::Anno { base, notes } => {
            base.0 = map(base.0)?;
            for item in notes {
                item.0 = map(item.0)?;
            }
        }
        DocKind::InlineMath { .. } => {}
        DocKind::Anchor { label, .. } => {
            label.0 = map(label.0)?;
        }
        DocKind::Reference { label, .. } => {
            label.0 = map(label.0)?;
        }
        DocKind::Emphasis { inline } => {
            inline.0 = map(inline.0)?;
        }
        DocKind::Strong { inline } => {
            inline.0 = map(inline.0)?;
        }
        DocKind::Break => {}
        DocKind::DisplayMath { .. } => {}
        DocKind::CircuitFigure { caption, .. } => {
            caption.0 = map(caption.0)?;
        }
        DocKind::Code { .. } => {}
        DocKind::Table { header, rows, .. } => {
            if let Some(item) = header {
                item.0 = map(item.0)?;
            }
            for item in rows {
                item.0 = map(item.0)?;
            }
        }
        DocKind::Row { cells } => {
            for item in cells {
                item.0 = map(item.0)?;
            }
        }
        DocKind::List { items, .. } => {
            for item in items {
                item.0 = map(item.0)?;
            }
        }
        DocKind::ListItem { body, .. } => {
            body.0 = map(body.0)?;
        }
        DocKind::Link { label, .. } => {
            label.0 = map(label.0)?;
        }
        DocKind::InlineCode { .. } => {}
        DocKind::RawCode { .. } => {}
        DocKind::Image { alt, caption, .. } => {
            alt.0 = map(alt.0)?;
            if let Some(item) = caption {
                item.0 = map(item.0)?;
            }
        }
        DocKind::InlineImage { alt, .. } => {
            alt.0 = map(alt.0)?;
        }
    }
    Ok(())
}

pub(crate) fn rewrite_root(
    root: &mut DocRoot,
    map: impl FnOnce(u64) -> Result<u64, super::ShapeError>,
) -> Result<(), super::ShapeError> {
    let id = map(self::root(*root).0)?;
    match root {
        DocRoot::Article(v) => v.0 = id,
        DocRoot::Body(v) => v.0 = id,
        DocRoot::Block(v) => v.0 = id,
        DocRoot::Flow(v) => v.0 = id,
        DocRoot::Sentence(v) => v.0 = id,
        DocRoot::Inline(v) => v.0 = id,
        DocRoot::Variant(v) => v.0 = id,
        DocRoot::Row(v) => v.0 = id,
        DocRoot::ListItem(v) => v.0 = id,
        DocRoot::Alignment(v) => v.0 = id,
        DocRoot::ListStyle(v) => v.0 = id,
        DocRoot::Check(v) => v.0 = id,
        DocRoot::Target(v) => v.0 = id,
        DocRoot::Asset(v) => v.0 = id,
        DocRoot::OptionalRow(v) => v.0 = id,
        DocRoot::OptionalSentence(v) => v.0 = id,
        DocRoot::OptionalText(v) => v.0 = id,
    }
    Ok(())
}
