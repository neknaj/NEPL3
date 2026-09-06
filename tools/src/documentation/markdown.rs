use crate::Result;
use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag, TagEnd};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct Structure {
    elements: BTreeMap<String, u64>,
    headings: Vec<Heading>,
    links: Vec<Link>,
    code_blocks: Vec<Code>,
    inline_code: Vec<InlineCode>,
    html_fragments: Vec<HtmlFragment>,
    gap_ids: BTreeSet<String>,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Heading {
    level: u8,
    text: String,
    inline_segments: Vec<InlineSegment>,
    source_bytes: [u64; 2],
    stable_anchor_status: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Link {
    kind: String,
    destination: String,
    title: String,
    label: String,
    inline_segments: Vec<InlineSegment>,
    source_bytes: [u64; 2],
    target_class: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct InlineSegment {
    content: InlineContent,
    source_bytes: [u64; 2],
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(
    tag = "kind",
    content = "value",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
enum InlineContent {
    Text(String),
    Code(String),
    SoftBreak,
    HardBreak,
    InlineMath(String),
    DisplayMath(String),
    FootnoteReference(String),
}

impl InlineSegment {
    fn append_to(&self, text: &mut String, segments: &mut Vec<Self>) {
        match &self.content {
            InlineContent::Text(value) | InlineContent::Code(value) => text.push_str(value),
            InlineContent::SoftBreak => text.push(' '),
            InlineContent::HardBreak => text.push('\n'),
            InlineContent::InlineMath(value) => {
                text.push('$');
                text.push_str(value);
                text.push('$');
            }
            InlineContent::DisplayMath(value) => {
                text.push_str("$$");
                text.push_str(value);
                text.push_str("$$");
            }
            InlineContent::FootnoteReference(value) => {
                text.push_str("[^");
                text.push_str(value);
                text.push(']');
            }
        }
        segments.push(self.clone());
    }
}

fn inline_segment(
    event: &Event<'_>,
    source: &str,
    span: &std::ops::Range<usize>,
) -> Result<Option<InlineSegment>> {
    let content = match event {
        Event::Text(text) => InlineContent::Text(text.to_string()),
        Event::Code(text) => InlineContent::Code(text.to_string()),
        Event::SoftBreak => InlineContent::SoftBreak,
        Event::HardBreak => InlineContent::HardBreak,
        Event::InlineMath(text) => InlineContent::InlineMath(text.to_string()),
        Event::DisplayMath(text) => InlineContent::DisplayMath(text.to_string()),
        Event::FootnoteReference(label) => InlineContent::FootnoteReference(label.to_string()),
        _ => return Ok(None),
    };
    source
        .get(span.clone())
        .ok_or("inline projection range outside UTF-8 source")?;
    Ok(Some(InlineSegment {
        content,
        source_bytes: range(span)?,
    }))
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct Code {
    info: String,
    source_bytes: [u64; 2],
    raw_source_sha256: String,
    content_sha256: String,
    content_bytes: u64,
    interpretation: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct InlineCode {
    text: String,
    source_bytes: [u64; 2],
    raw_source_sha256: String,
}

#[derive(Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct HtmlFragment {
    kind: String,
    literal: String,
    source_bytes: [u64; 2],
}

pub(super) fn option_names() -> Vec<String> {
    [
        "tables",
        "footnotes",
        "strikethrough",
        "tasklists",
        "gfm-alerts",
        "math",
    ]
    .iter()
    .map(|s| (*s).into())
    .collect()
}

fn range(r: &std::ops::Range<usize>) -> Result<[u64; 2]> {
    Ok([u64::try_from(r.start)?, u64::try_from(r.end)?])
}

pub(super) fn extract(source: &str) -> Result<Structure> {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_GFM
        | Options::ENABLE_MATH;
    let mut out = Structure {
        elements: BTreeMap::new(),
        headings: Vec::new(),
        links: Vec::new(),
        code_blocks: Vec::new(),
        inline_code: Vec::new(),
        html_fragments: Vec::new(),
        gap_ids: BTreeSet::new(),
    };
    let mut heading: Option<usize> = None;
    let mut links: Vec<usize> = Vec::new();
    let mut code: Option<(usize, String)> = None;
    for (event, span) in Parser::new_ext(source, options).into_offset_iter() {
        let feature = match &event {
            Event::Start(Tag::Paragraph) => Some("paragraph"),
            Event::Start(Tag::Heading { .. }) => Some("heading"),
            Event::Start(Tag::Table(_)) => Some("table"),
            Event::Start(Tag::TableCell) => Some("table-cell"),
            Event::Start(Tag::List(_)) => Some("list"),
            Event::Start(Tag::Item) => Some("list-item"),
            Event::Start(Tag::BlockQuote(_)) => Some("blockquote"),
            Event::TaskListMarker(_) => Some("task-checkbox"),
            Event::Start(Tag::FootnoteDefinition(_)) => Some("footnote"),
            Event::FootnoteReference(_) => Some("footnote-reference"),
            Event::Start(Tag::Emphasis) => Some("emphasis"),
            Event::Start(Tag::Strong) => Some("strong"),
            Event::Start(Tag::Strikethrough) => Some("strikethrough"),
            Event::Code(_) => Some("inline-code"),
            Event::InlineMath(_) => Some("inline-math"),
            Event::DisplayMath(_) => Some("display-math"),
            Event::Html(text) | Event::InlineHtml(text) => {
                Some(if text.trim().starts_with("<!--") {
                    "html-comment"
                } else {
                    "raw-html"
                })
            }
            Event::Rule => Some("thematic-break"),
            Event::Start(Tag::CodeBlock(_)) => Some("code-block"),
            Event::Start(Tag::Link { .. }) => Some("link"),
            Event::Start(Tag::Image { .. }) => Some("image"),
            _ => None,
        };
        if let Some(feature) = feature {
            *out.elements.entry(feature.into()).or_insert(0) += 1;
            let gap = match feature {
                "table" | "table-cell" => Some("DG01"),
                "list" | "list-item" | "task-checkbox" | "blockquote" | "thematic-break" => {
                    Some("DG02")
                }
                "link" => Some("DG03"),
                "inline-code" | "code-block" => Some("DG04"),
                "image" | "raw-html" => Some("DG05"),
                "heading" | "footnote" | "footnote-reference" => Some("DG06"),
                "inline-math" | "display-math" => Some("DG07"),
                "strikethrough" => Some("DG08"),
                "html-comment" => Some("DG09"),
                _ => None,
            };
            if let Some(gap) = gap {
                out.gap_ids.insert(gap.into());
            }
        }
        // One projection contract for headings, link labels and image alt text.
        // Keep block-code collection separate: its payload is literal source.
        if code.is_none()
            && (heading.is_some() || !links.is_empty())
            && let Some(segment) = inline_segment(&event, source, &span)?
        {
            if let Some(index) = heading {
                let entry = out.headings.get_mut(index).ok_or("missing heading entry")?;
                segment.append_to(&mut entry.text, &mut entry.inline_segments);
            }
            for index in &links {
                let entry = out.links.get_mut(*index).ok_or("missing link entry")?;
                segment.append_to(&mut entry.label, &mut entry.inline_segments);
            }
        }
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(out.headings.len());
                out.headings.push(Heading { level: level as u8, text: String::new(), inline_segments: Vec::new(), source_bytes: range(&span)?, stable_anchor_status: "needs-explicit-old-anchor-and-page-registry-mapping; heading text alone is not a stable ID".into() });
            }
            Event::End(TagEnd::Heading(_)) => heading = None,
            Event::Start(Tag::Link {
                dest_url, title, ..
            })
            | Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                let kind = if feature == Some("image") {
                    "image"
                } else {
                    "link"
                };
                let class = if dest_url.starts_with('#') {
                    "same-page-fragment"
                } else if dest_url.starts_with("http://") || dest_url.starts_with("https://") {
                    "external-http-url"
                } else {
                    "relative-or-other-uri; resolve-with-explicit-policy"
                };
                links.push(out.links.len());
                out.links.push(Link {
                    kind: kind.into(),
                    destination: dest_url.into_string(),
                    title: title.into_string(),
                    label: String::new(),
                    inline_segments: Vec::new(),
                    source_bytes: range(&span)?,
                    target_class: class.into(),
                });
            }
            Event::End(TagEnd::Link | TagEnd::Image) => {
                links.pop();
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let info = match kind {
                    CodeBlockKind::Fenced(info) => info.into_string(),
                    CodeBlockKind::Indented => String::new(),
                };
                code = Some((out.code_blocks.len(), String::new()));
                out.code_blocks.push(Code { info, source_bytes: range(&span)?, raw_source_sha256: super::digest(source.get(span).ok_or("parser code range outside UTF-8 source")?.as_bytes()), content_sha256: String::new(), content_bytes: 0, interpretation: "Literal display only; info string is a hint, never authorization to evaluate or assume a valid NEPL grammar".into() });
            }
            Event::End(TagEnd::CodeBlock) => {
                if let Some((index, text)) = code.take() {
                    let block = out
                        .code_blocks
                        .get_mut(index)
                        .ok_or("missing code inventory entry")?;
                    block.content_sha256 = super::digest(text.as_bytes());
                    block.content_bytes = u64::try_from(text.len())?;
                }
            }
            Event::Text(text) | Event::Code(text) => {
                if feature == Some("inline-code") {
                    out.inline_code.push(InlineCode {
                        text: text.to_string(),
                        source_bytes: range(&span)?,
                        raw_source_sha256: super::digest(
                            source
                                .get(span.clone())
                                .ok_or("inline code source range invalid")?
                                .as_bytes(),
                        ),
                    });
                }
                if let Some((_, content)) = &mut code {
                    content.push_str(&text);
                }
            }
            Event::Html(text) | Event::InlineHtml(text) => {
                out.html_fragments.push(HtmlFragment {
                    kind: feature.ok_or("missing HTML feature")?.into(),
                    literal: text.into_string(),
                    source_bytes: range(&span)?,
                });
            }
            _ => {}
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_projection_preserves_soft_and_hard_word_boundaries() -> Result<()> {
        let source = "[alpha\nbeta](soft) [gamma  \ndelta](hard)\n";
        let structure = extract(source)?;
        assert_eq!(
            structure.links.first().ok_or("soft link missing")?.label,
            "alpha beta"
        );
        assert_eq!(
            structure.links.get(1).ok_or("hard link missing")?.label,
            "gamma\ndelta"
        );
        Ok(())
    }

    #[test]
    fn heading_and_link_projection_keep_both_math_event_kinds() -> Result<()> {
        let structure = extract("# [Math $x$ and $$y$$](target)\n")?;
        assert_eq!(structure.elements.get("inline-math"), Some(&1));
        assert_eq!(structure.elements.get("display-math"), Some(&1));
        assert_eq!(
            structure.headings.first().ok_or("heading missing")?.text,
            "Math $x$ and $$y$$"
        );
        assert_eq!(
            structure.links.first().ok_or("link missing")?.label,
            "Math $x$ and $$y$$"
        );
        Ok(())
    }

    fn segment_source<'a>(source: &'a str, segment: &InlineSegment) -> Result<&'a str> {
        source
            .get(
                usize::try_from(segment.source_bytes[0])?
                    ..usize::try_from(segment.source_bytes[1])?,
            )
            .ok_or_else(|| "segment is not a valid UTF-8 source range".into())
    }

    #[test]
    fn unicode_crlf_breaks_and_math_keep_original_byte_ranges() -> Result<()> {
        let source = "前置🙂\r\n\r\n[日🙂\r\n本  \r\n語\\\r\n尾 $α$ $$β$$](dest)\r\n";
        let structure = extract(source)?;
        let link = structure.links.first().ok_or("link missing")?;
        assert_eq!(link.label, "日🙂 本\n語\n尾 $α$ $$β$$");
        let breaks: Vec<_> = link
            .inline_segments
            .iter()
            .filter(|s| {
                matches!(
                    s.content,
                    InlineContent::SoftBreak | InlineContent::HardBreak
                )
            })
            .collect();
        assert_eq!(breaks.len(), 3);
        assert!(matches!(breaks[0].content, InlineContent::SoftBreak));
        assert!(matches!(breaks[1].content, InlineContent::HardBreak));
        assert!(matches!(breaks[2].content, InlineContent::HardBreak));
        assert_eq!(segment_source(source, breaks[0])?, "\r\n");
        assert_eq!(segment_source(source, breaks[1])?, "  \r\n");
        assert_eq!(segment_source(source, breaks[2])?, "\\\r\n");
        for segment in &link.inline_segments {
            let raw = segment_source(source, segment)?;
            match &segment.content {
                InlineContent::Text(text) if text == "日🙂" => {
                    assert_eq!(
                        segment.source_bytes[0],
                        u64::try_from(source.find("日🙂").ok_or("fixture text missing")?)?
                    );
                    assert_eq!(segment.source_bytes[1] - segment.source_bytes[0], 7);
                    assert_eq!(raw, "日🙂");
                }
                InlineContent::InlineMath(value) => {
                    assert_eq!(value, "α");
                    assert_eq!(raw, "$α$");
                }
                InlineContent::DisplayMath(value) => {
                    assert_eq!(value, "β");
                    assert_eq!(raw, "$$β$$");
                }
                _ => {}
            }
        }
        Ok(())
    }

    #[test]
    fn nested_image_code_and_math_share_heading_and_link_projection() -> Result<()> {
        let source = "[![日 `x` $α$\n本 $$β$$](badge.svg)](run)\n===\n";
        let structure = extract(source)?;
        let heading = structure.headings.first().ok_or("heading missing")?;
        let outer = structure.links.first().ok_or("outer link missing")?;
        let image = structure.links.get(1).ok_or("image missing")?;
        assert_eq!(heading.text, "日 x $α$ 本 $$β$$");
        assert_eq!(heading.text, outer.label);
        assert_eq!(heading.text, image.label);
        assert_eq!(heading.inline_segments, outer.inline_segments);
        assert_eq!(heading.inline_segments, image.inline_segments);
        assert_eq!(image.kind, "image");
        let code = image
            .inline_segments
            .iter()
            .find(|s| matches!(&s.content, InlineContent::Code(value) if value == "x"))
            .ok_or("typed inline code missing")?;
        assert_eq!(segment_source(source, code)?, "`x`");
        Ok(())
    }

    #[test]
    fn footnote_markers_are_preserved_without_inventing_rendered_numbers() -> Result<()> {
        let source = "# Note [^n]\n\n[^n]: body\n";
        let structure = extract(source)?;
        let heading = structure.headings.first().ok_or("heading missing")?;
        assert_eq!(heading.text, "Note [^n]");
        let marker = heading
            .inline_segments
            .iter()
            .find(|s| matches!(&s.content, InlineContent::FootnoteReference(value) if value == "n"))
            .ok_or("typed footnote marker missing")?;
        assert_eq!(segment_source(source, marker)?, "[^n]");
        Ok(())
    }

    #[test]
    fn hard_break_is_the_same_typed_newline_in_setext_heading_and_link() -> Result<()> {
        let structure = extract("[left  \nright](target)\n===\n")?;
        let heading = structure.headings.first().ok_or("heading missing")?;
        let link = structure.links.first().ok_or("link missing")?;
        assert_eq!(heading.text, "left\nright");
        assert_eq!(heading.text, link.label);
        assert_eq!(heading.inline_segments, link.inline_segments);
        assert!(
            heading
                .inline_segments
                .iter()
                .any(|s| matches!(s.content, InlineContent::HardBreak))
        );
        Ok(())
    }

    #[test]
    fn inline_html_is_retained_separately_and_not_interpreted_by_summary_projection() -> Result<()>
    {
        let source = "# [a<br>b<!--note-->](target)\n";
        let structure = extract(source)?;
        assert_eq!(structure.links.first().ok_or("link missing")?.label, "ab");
        assert_eq!(
            structure
                .html_fragments
                .iter()
                .map(|f| f.literal.as_str())
                .collect::<Vec<_>>(),
            vec!["<br>", "<!--note-->"]
        );
        Ok(())
    }

    #[test]
    fn real_markdown_parser_keeps_badges_nested_links_and_literal_code_separate() -> Result<()> {
        let source = "# 見出し `T`\n\n[![CI](https://example.invalid/badge.svg)](https://example.invalid/run)\n\n- [ ] check\n\n```text\n#![no_std]\n# not a heading\n| not | a table |\n```\n";
        let structure = extract(source)?;
        assert_eq!(structure.elements.get("image"), Some(&1));
        assert_eq!(structure.elements.get("link"), Some(&1));
        assert_eq!(structure.elements.get("task-checkbox"), Some(&1));
        assert_eq!(structure.elements.get("heading"), Some(&1));
        assert!(!structure.elements.contains_key("table"));
        assert_eq!(
            structure.headings.first().ok_or("heading missing")?.text,
            "見出し T"
        );
        assert_eq!(
            structure
                .code_blocks
                .first()
                .ok_or("code missing")?
                .content_sha256,
            super::super::digest(b"#![no_std]\n# not a heading\n| not | a table |\n")
        );
        assert_eq!(structure.links.first().ok_or("link missing")?.label, "CI");
        Ok(())
    }

    #[test]
    fn generic_notation_is_literal_code_and_generated_comments_are_metadata() -> Result<()> {
        let old = extract("List<T> Option<T>\n")?;
        assert_eq!(old.elements.get("raw-html"), Some(&2));
        let fixed = extract("`List<T>` `Option<T>`\n<!-- generated metadata -->\n")?;
        assert!(!fixed.elements.contains_key("raw-html"));
        assert_eq!(fixed.elements.get("inline-code"), Some(&2));
        assert_eq!(fixed.elements.get("html-comment"), Some(&1));
        Ok(())
    }

    #[test]
    fn corrected_foundation_and_math_documents_have_no_accidental_html_tags() -> Result<()> {
        for source in [
            include_str!("../../../doc/spec/02-foundation.md"),
            include_str!("../../../doc/spec/06-math.md"),
        ] {
            assert!(!extract(source)?.elements.contains_key("raw-html"));
        }
        Ok(())
    }
}
