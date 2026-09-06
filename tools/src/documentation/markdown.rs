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
    source_bytes: [u64; 2],
    target_class: String,
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
    let mut links = Vec::new();
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
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                heading = Some(out.headings.len());
                out.headings.push(Heading { level: level as u8, text: String::new(), source_bytes: range(&span)?, stable_anchor_status: "needs-explicit-old-anchor-and-page-registry-mapping; heading text alone is not a stable ID".into() });
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
                if let Some(index) = heading {
                    out.headings
                        .get_mut(index)
                        .ok_or("missing heading entry")?
                        .text
                        .push_str(&text);
                }
                for index in &links {
                    out.links
                        .get_mut(*index)
                        .ok_or("missing link entry")?
                        .label
                        .push_str(&text);
                }
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(index) = heading {
                    out.headings
                        .get_mut(index)
                        .ok_or("missing heading entry")?
                        .text
                        .push(' ');
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
