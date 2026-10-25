use super::*;
use nepl3_doc_core::{
    check::Category,
    labels::namespace as labels,
    pages::{PageDocument, PageRegistration, PageSet},
};
use nepl3_doc_html::{
    ParallelMode, RenderOptions,
    assets::SvgInput,
    namespace::{
        assets::MemberAssets,
        pages::{
            self as routed,
            assets::{self, PageMemberAssets},
        },
    },
};
use nepl3_tools::doc::annotations::{SentenceAnnotationRenderer, alt};
const SVG: &str =
    "<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
#[test]
fn pages_native_svg_uses_actual_retained_sentence_alt_projection() -> Result<(), String> {
    let compiled = compiled()?;
    let source = r#"article en "Title" body cons image asset "logo" none Sentence "{[漢/かん]/note}" none nil"#;
    with_input_route(true, &compiled, source, "Article", |tree, profile, b, _| {
        let store = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let root = nepl3_suite::adapters::sentence::literal::lower(
            tree.syntax(),
            &compiled.doc.package.schema,
            Category::Article,
            profile.registry(),
            b,
            &mut codec,
        )
        .map_err(err)?;
        let projection = alt::prepare(
            &root,
            &mut SentenceAnnotationRenderer {
                registry: profile.registry(),
                surface: &compiled.others.get(3).ok_or("Sentence")?.schema,
                math_surface: Some(&compiled.others.first().ok_or("Math")?.schema),
                doc_surface: Some(&compiled.doc.package.schema),
                codec: &mut codec,
            },
            b,
        )
        .map_err(err)?;
        assert_eq!(projection.resolved.len(), 1);
        assert_eq!(projection.resolved[0].text, "漢");
        assert!(!projection.sentences[0].1.sources.is_empty());
        let set = PageSet {
            pages: vec![PageDocument {
                registration: PageRegistration {
                    id: "one".into(),
                    source: "one.nepld".into(),
                    route: "one.html".into(),
                },
                document: root,
            }],
            files: vec![],
        };
        let member = labels::inspect(
            &set.pages[0].document,
            profile.registry(),
            b,
            &mut admission,
        )
        .map_err(err)?;
        let mut codec =
            FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
        let members = [&member];
        let namespace = labels::resolve(&members, b).map_err(err)?;
        let options = RenderOptions {
            parallel: ParallelMode::Rows,
        };
        let svg = [SvgInput {
            id: "logo",
            svg: SVG,
        }];
        let inputs = [PageMemberAssets {
            page: 0,
            assets: MemberAssets {
                member: labels::MemberId(0),
                inputs: &svg,
                resolved_alt: &projection.resolved,
            },
        }];
        let prepared = assets::prepare_embedded(
            &set,
            &[&namespace],
            &options,
            &inputs,
            profile.registry(),
            &mut codec,
            b,
        )
        .map_err(err)?;
        let mut called = false;
        let output = routed::render(
            &prepared,
            &mut |_, _, _, _, _| {
                called = true;
                Err::<_, String>("alt must not call display".into())
            },
            b,
        )
        .map_err(err)?;
        assert!(!called);
        let mut count = 0;
        for node in &output.fragments[0].fragment.markup.fragment.nodes {
            if let nepl3_markup::html::HtmlNode::Element { attributes, .. } = node {
                for attribute in attributes {
                    if let nepl3_markup::html::HtmlAttribute::Alt { value: text } = attribute {
                        assert_eq!(text, "漢");
                        count += 1;
                    }
                }
            }
        }
        assert_eq!(count, 2);
        let unresolved = [PageMemberAssets {
            page: 0,
            assets: MemberAssets {
                member: labels::MemberId(0),
                inputs: &svg,
                resolved_alt: &[],
            },
        }];
        let result = assets::prepare_embedded(
            &set,
            &[&namespace],
            &options,
            &unresolved,
            profile.registry(),
            &mut codec,
            b,
        );
        assert!(matches!(
            result,
            Err(assets::PreparationError::Member {
                page: 0,
                member: labels::MemberId(0),
                error: nepl3_doc_html::assets::AssetError::Alt
            })
        ));
        Ok(())
    })
}
#[test]
fn pages_native_svg_does_not_grant_code_to_adopted_members() -> Result<(), String> {
    let compiled = compiled()?;
    with_input_route(
        true,
        &compiled,
        r#"article en "Title" body cons code Math frac 1 2 nil"#,
        "Article",
        |tree, profile, b, _| {
            let store = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let root = nepl3_suite::adapters::sentence::literal::lower(
                tree.syntax(),
                &compiled.doc.package.schema,
                Category::Article,
                profile.registry(),
                b,
                &mut codec,
            )
            .map_err(err)?;
            let set = PageSet {
                pages: vec![PageDocument {
                    registration: PageRegistration {
                        id: "one".into(),
                        source: "one.nepld".into(),
                        route: "one.html".into(),
                    },
                    document: root,
                }],
                files: vec![],
            };
            let member = labels::inspect(
                &set.pages[0].document,
                profile.registry(),
                b,
                &mut admission,
            )
            .map_err(err)?;
            let mut codec =
                FoundationCodec::new(profile.registry(), &store, &mut admission).map_err(err)?;
            let members = [&member, &member];
            let namespace = labels::resolve(&members, b).map_err(err)?;
            let options = RenderOptions {
                parallel: ParallelMode::Rows,
            };
            let result = assets::prepare_embedded(
                &set,
                &[&namespace],
                &options,
                &[],
                profile.registry(),
                &mut codec,
                b,
            );
            assert!(matches!(
                result,
                Err(assets::PreparationError::Member {
                    page: 0,
                    member: labels::MemberId(1),
                    error: nepl3_doc_html::assets::AssetError::Unsupported
                })
            ));
            Ok(())
        },
    )
}
