use super::*;
use crate::doc::annotations::{SentenceAnnotationRenderer, article};
use article::render::svg::pages as planned;
use nepl3_doc_core::model::*;
use nepl3_doc_html::{ParallelMode, RenderOptions, assets::SvgInput};
use nepl3_markup::html::{self, HtmlAttribute, HtmlNode};
const A: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const B: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 2 2'><path d='M0 0L2 2'/></svg>";
const ONE: &str = r#"article en Sentence "Shared" body
cons image asset "logo" none "Shared" some Sentence sentence cons document anchor cap text "Caption" nil
cons paragraph cons sentence cons anchor outer text "Root" nil
cons sentence cons image asset "logo" none Sentence "{[漢/かん]/note}" nil
cons parallel cons variant en Sentence sentence cons document anchor enlabel text "English" cons math frac 1 2 nil cons variant ja Sentence sentence cons document anchor jalabel text "Japanese" cons math frac 3 4 nil nil
cons Sentence sentence cons document link page "two" some "guest" text "Next" nil
cons code Math frac 5 6 cons sentence cons math Math frac 7 8 nil
cons Sentence sentence cons document math Math frac 9 10 nil nil nil"#;
const TWO: &str = r#"article en Sentence "Two" body
cons image asset "logo" none "Second image" none
cons paragraph cons Sentence sentence cons document anchor guest text "Target" cons document anchor cap text "Local cap" cons document link relative "../one.nepld" some "cap" text "Back" cons math frac 11 12 nil
cons code Math frac 13 14 cons sentence cons math Math frac 15 16 nil nil nil"#;
fn fixture(
    c: &Compiled,
    one: &str,
    two: &str,
    shared: bool,
) -> Result<(PageSet, Vec<code::PreparedPageCode>), String> {
    let mut set = PageSet {
        pages: Vec::new(),
        files: Vec::new(),
    };
    let mut codes = Vec::new();
    for (id, source, route, text) in [
        ("one", "docs/one.nepld", "first/index.html", one),
        ("two", "docs/nested/two.nepld", "other/deep.html", two),
    ] {
        let (document, code) = crate::doc::source::with_named_input(
            true,
            c,
            text,
            id,
            "Article",
            |tree, profile, b, _| {
                let store = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(profile.registry(), &store, &mut admission)
                    .map_err(err)?;
                let document = nepl3_suite::adapters::sentence::literal::lower(
                    tree.syntax(),
                    &c.doc.package.schema,
                    nepl3_doc_core::check::Category::Article,
                    profile.registry(),
                    b,
                    &mut codec,
                )
                .map_err(err)?;
                let code = code::PreparedPageCode::prepare(&document, tree, profile, b)?;
                Ok((document, code))
            },
        )?;
        set.pages.push(PageDocument {
            registration: PageRegistration {
                id: id.into(),
                source: source.into(),
                route: route.into(),
            },
            document,
        });
        codes.push(code);
    }
    if shared {
        let doc = &mut set.pages[0].document;
        let DocRoot::Article(root) = doc.value.root else {
            return Err("root".into());
        };
        let DocKind::Article { title, .. } = doc.value.nodes[root.0 as usize].kind else {
            return Err("article".into());
        };
        let mut old = None;
        for node in &mut doc.value.nodes {
            if let DocKind::Image { alt, .. } = &mut node.kind {
                old = Some(*alt);
                *alt = title;
            }
        }
        let old = old.ok_or("image")?;
        for node in &mut doc.value.nodes {
            if let DocKind::Paragraph { items } = &mut node.kind {
                // Preserve every original node/owner and create a real shared
                // title/alt/body occurrence, rather than aliasing by digest.
                items.push(FlowRef(old.0));
                items.push(FlowRef(title.0));
            }
        }
    }
    Ok((set, codes))
}
fn host<'a, C: FoundationValueCodec>(
    c: &'a Compiled,
    codec: &'a mut C,
) -> Result<SentenceAnnotationRenderer<'a, C>, String> {
    Ok(SentenceAnnotationRenderer {
        registry: &c.doc.registry,
        surface: &c.others.get(3).ok_or("Sentence")?.schema,
        math_surface: Some(&c.others.first().ok_or("Math")?.schema),
        doc_surface: Some(&c.doc.package.schema),
        codec,
    })
}
#[test]
fn planned_pages_svg_retains_real_owners_routes_code_math_and_occurrence_gaps() -> Result<(), String>
{
    let c = compiled()?;
    let (set, codes) = fixture(&c, ONE, TWO, true)?;
    for (parallel, ids, placements, english, japanese, math_count) in [
        (
            ParallelMode::Rows,
            vec![0, 2, 4, 5, 6, 7, 8],
            vec![0, 1, 2, 3, 4, 7, 8],
            true,
            true,
            4,
        ),
        (
            ParallelMode::Columns,
            vec![0, 2, 4, 5, 6, 7, 8],
            vec![0, 1, 2, 3, 4, 7, 8],
            true,
            true,
            4,
        ),
        (
            ParallelMode::Single {
                language: "en".into(),
                fallbacks: vec![],
            },
            vec![0, 2, 4, 6, 7, 8],
            vec![0, 1, 2, 3, 6, 7],
            true,
            false,
            3,
        ),
        (
            ParallelMode::Single {
                language: "fr".into(),
                fallbacks: vec!["ja".into()],
            },
            vec![0, 2, 5, 6, 7, 8],
            vec![0, 1, 2, 3, 6, 7],
            false,
            true,
            3,
        ),
    ] {
        let options = RenderOptions { parallel };
        let mut b = crate::doc::source::budget();
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
        let mut host = host(&c, &mut codec)?;
        let selections = set
            .pages
            .iter()
            .map(|p| article::collect_structural(&p.document, &mut host, &mut b).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let plans = selections
            .iter()
            .map(|s| article::display::plan(s, &options, &mut b).map_err(err))
            .collect::<Result<Vec<_>, _>>()?;
        let a = [SvgInput { id: "logo", svg: A }];
        let bb = [SvgInput { id: "logo", svg: B }];
        let inputs = [
            planned::RootAssets {
                page: 0,
                inputs: &a,
            },
            planned::RootAssets {
                page: 1,
                inputs: &bb,
            },
        ];
        let mut maths = [Vec::new(), Vec::new()];
        let mut calls = Vec::new();
        let output = planned::render(
            &set,
            &options,
            &plans,
            &inputs,
            &mut host,
            &mut |page, view, embed, codec, b| {
                let page = usize::try_from(page).map_err(err)?;
                calls.push((page, view.kind));
                if view.kind == EmbedKind::Code {
                    codes[page].render_with_codec(view, embed, codec, b)
                } else {
                    super::super::super::math::DisplayHost {
                        compiled: &c,
                        registry: &c.doc.registry,
                        codec,
                        preference: super::super::super::MathRenderer::MathMLOnly,
                    }
                    .render(view, embed, &mut maths[page], b)
                }
            },
            &mut b,
        )
        .map_err(err)?;
        assert_eq!(output.pages.fragments.len(), 2);
        assert_eq!(output.alt_projections.len(), 2);
        assert_eq!(output.alt_projections[0].sentences.len(), 2);
        assert!(output.alt_projections[1].sentences.is_empty());
        assert!(output.alt_projections[1].resolved.is_empty());
        assert_eq!(
            output.alt_projections[0]
                .resolved
                .iter()
                .map(|r| r.text.as_str())
                .collect::<Vec<_>>(),
            vec!["Shared", "漢"]
        );
        assert!(
            output.alt_projections[0]
                .sentences
                .iter()
                .all(|(_, s)| !s.sources.is_empty())
        );
        for (page, resolution) in output.resolutions.iter().enumerate() {
            assert!(std::ptr::eq(resolution.input(), &selections[page]));
            assert_eq!(resolution.adopted(), plans[page].adoption());
        }
        assert_eq!(
            output.sentences[0]
                .iter()
                .map(|r| r.occurrence.0)
                .collect::<Vec<_>>(),
            ids
        );
        assert_eq!(
            output.sentences[0]
                .iter()
                .map(|r| r.placement)
                .collect::<Vec<_>>(),
            placements
        );
        assert_eq!(
            output.sentences[0].first().ok_or("first")?.owner,
            output.sentences[0].last().ok_or("last")?.owner
        );
        // Nested Doc Math is rendered by the retained Sentence host; only
        // root Code and direct Doc Math cross this page-aware callback.
        assert_eq!(
            calls,
            vec![
                (0, EmbedKind::Code),
                (0, EmbedKind::InlineMath),
                (1, EmbedKind::Code),
                (1, EmbedKind::InlineMath)
            ]
        );
        assert_eq!(maths.iter().map(Vec::len).collect::<Vec<_>>(), vec![1, 1]);
        for (page, fragment) in output.pages.fragments.iter().enumerate() {
            let markup = &fragment.fragment.markup;
            let proof = html::validate(&markup.fragment, markup.slot, &markup.policy, &mut b)
                .map_err(err)?;
            let text = html::serialize(&proof, &mut b).map_err(err)?;
            assert!(text.contains("nepl-code-marker"));
            assert_eq!(
                text.matches("<mfrac>").count(),
                if page == 0 { math_count } else { 2 }
            );
            assert_eq!(
                text.matches("<math").count(),
                if page == 0 { math_count } else { 2 }
            );
            if page == 0 {
                assert_eq!(text.contains("English"), english);
                assert_eq!(text.contains("Japanese"), japanese);
                assert!(text.contains("../other/deep.html#n-6775657374"));
                assert!(text.contains("alt=\"Shared\"") && text.contains("alt=\"漢\""));
            } else {
                assert!(text.contains("../first/index.html#n-636170"));
            }
            for record in &output.sentences[page] {
                assert!(
                    record
                        .origins
                        .iter()
                        .all(|o| o.element < markup.fragment.nodes.len() as u64)
                );
            }
            if page == 0 {
                let record = output.sentences[0]
                    .iter()
                    .find(|r| r.occurrence.0 == 7)
                    .ok_or("nested occurrence")?;
                let nested = record
                    .foreign
                    .iter()
                    .find_map(|r| match r {
                        crate::doc::annotations::ForeignRecord::Document(doc)
                            if !doc.foreign.is_empty() =>
                        {
                            Some(doc)
                        }
                        _ => None,
                    })
                    .ok_or("retained nested document")?;
                assert_eq!(nested.foreign.len(), 1);
                let math = &nested.foreign[0].output;
                assert!(!math.syntax.sources.is_empty());
                let placement = &fragment.foreign[record.placement];
                assert!(
                    math.root >= placement.first_element
                        && math.root < placement.first_element + placement.elements
                );
                let nepl3_math_core::model::MathRoot::Expr(expr) = math.syntax.value.root else {
                    return Err("expression root".into());
                };
                let nepl3_math_core::model::MathKind::Frac { left, right } =
                    math.syntax.value.nodes[expr.0 as usize].kind
                else {
                    return Err("fraction source".into());
                };
                let fraction = math.node_roots[expr.0 as usize];
                let HtmlNode::MathElement {
                    tag: nepl3_markup::mathml::Tag::Math,
                    children,
                    ..
                } = &markup.fragment.nodes[math.root as usize]
                else {
                    return Err("actual final math root".into());
                };
                assert_eq!(children, &vec![fraction]);
                let HtmlNode::MathElement {
                    tag: nepl3_markup::mathml::Tag::Fraction,
                    children,
                    ..
                } = &markup.fragment.nodes[fraction as usize]
                else {
                    return Err("actual final fraction".into());
                };
                assert_eq!(
                    children,
                    &vec![
                        math.node_roots[left.0 as usize],
                        math.node_roots[right.0 as usize]
                    ]
                );
                for (node, expected) in [(left, 9), (right, 10)] {
                    let nepl3_math_core::model::MathKind::Number { value, .. } =
                        &math.syntax.value.nodes[node.0 as usize].kind
                    else {
                        return Err("retained number".into());
                    };
                    assert_eq!(
                        value,
                        &nepl3_core::value::Rational::new(expected.into(), 1.into())
                            .map_err(err)?
                    );
                    let HtmlNode::MathElement {
                        tag: nepl3_markup::mathml::Tag::Number,
                        children,
                        ..
                    } = &markup.fragment.nodes[math.node_roots[node.0 as usize] as usize]
                    else {
                        return Err("actual final number".into());
                    };
                    assert_eq!(children.len(), 1);
                    assert!(
                        matches!(&markup.fragment.nodes[children[0] as usize], HtmlNode::Text { text } if text == &expected.to_string())
                    );
                }
                let mut member = 1u64;
                let mut found = false;
                while let Some((origin, selected)) = output.resolutions[0]
                    .members()
                    .guest(nepl3_doc_core::labels::namespace::MemberId(member))
                {
                    if std::ptr::eq(selected.document.as_ref(), nested.document.as_ref()) {
                        assert_eq!(
                            origin,
                            article::namespace::MemberOrigin::SentenceInline {
                                sentence: record.occurrence,
                                owner: record.owner,
                                embed: nested.embed,
                                inline_occurrence: 0
                            }
                        );
                        found = true;
                        break;
                    }
                    member = member.checked_add(1).ok_or("member overflow")?;
                }
                assert!(found, "nested record must retain the adopted owner");
            }
            let images = markup
                .fragment
                .nodes
                .iter()
                .filter_map(|n| match n {
                    HtmlNode::Element { attributes, .. } => Some(attributes),
                    _ => None,
                })
                .flatten()
                .filter_map(|a| match a {
                    HtmlAttribute::EmbeddedSvg { svg } => Some(svg.as_str()),
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert!(!images.is_empty());
            assert!(
                images
                    .iter()
                    .all(|svg| *svg == if page == 0 { A } else { B })
            );
        }
    }
    Ok(())
}

#[path = "svg_host/failures.rs"]
mod failures;
#[path = "svg_host/limits.rs"]
mod limits;

#[test]
fn planned_pages_svg_accepts_asset_free_pages_without_root_bindings() -> Result<(), String> {
    let c = compiled()?;
    let one=ONE.replace(r#"cons image asset "logo" none "Shared" some Sentence sentence cons document anchor cap text "Caption" nil"#,r#"cons paragraph cons Sentence sentence cons document anchor cap text "Caption" nil nil"#)
        .replace(r#"cons sentence cons image asset "logo" none Sentence "{[漢/かん]/note}" nil"#,"");
    let two = TWO.replace(r#"cons image asset "logo" none "Second image" none"#, "");
    let (set, codes) = fixture(&c, &one, &two, false)?;
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let mut b = crate::doc::source::budget();
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut host = host(&c, &mut codec)?;
    let selections = set
        .pages
        .iter()
        .map(|p| article::collect_structural(&p.document, &mut host, &mut b).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let plans = selections
        .iter()
        .map(|s| article::display::plan(s, &options, &mut b).map_err(err))
        .collect::<Result<Vec<_>, _>>()?;
    let mut maths = [Vec::new(), Vec::new()];
    let output = planned::render(
        &set,
        &options,
        &plans,
        &[],
        &mut host,
        &mut |page, view, embed, codec, b| {
            let page = usize::try_from(page).map_err(err)?;
            if view.kind == EmbedKind::Code {
                codes[page].render_with_codec(view, embed, codec, b)
            } else {
                crate::doc::export::math::DisplayHost {
                    compiled: &c,
                    registry: &c.doc.registry,
                    codec,
                    preference: crate::doc::export::MathRenderer::MathMLOnly,
                }
                .render(view, embed, &mut maths[page], b)
            }
        },
        &mut b,
    )
    .map_err(err)?;
    assert_eq!(output.alt_projections.len(), 2);
    assert!(
        output
            .alt_projections
            .iter()
            .all(|p| p.sentences.is_empty() && p.resolved.is_empty())
    );
    assert_eq!(output.pages.fragments.len(), 2);
    assert!(
        output
            .pages
            .fragments
            .iter()
            .all(
                |p| p.fragment.markup.fragment.nodes.iter().all(|n| !matches!(
                    n,
                    HtmlNode::Element {
                        tag: html::HtmlTag::Img,
                        ..
                    }
                ))
            )
    );
    Ok(())
}

#[path = "svg_host/packaging.rs"]
mod packaging;

#[path = "svg_host/packaging_limits.rs"]
mod packaging_limits;

#[path = "svg_host/packaging_failures.rs"]
mod packaging_failures;

#[path = "svg_host/external.rs"]
mod external;

#[path = "svg_host/external/failures.rs"]
mod external_failures;
#[path = "svg_host/external/limits.rs"]
mod external_limits;
