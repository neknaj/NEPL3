use super::*;
use crate::doc::export::pages::svg as package;
const A: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='100' height='50' viewBox='0 0 1 1'><path d='M0 0L1 1'/></svg>";
const B: &str = "<svg xmlns='http://www.w3.org/2000/svg' width='400' height='80' viewBox='0 0 2 2'><path d='M0 0L2 2'/></svg>";

#[test]
fn packaged_svg_pages_keep_owners_csp_css_and_manifest() -> Result<(), String> {
    let c = compiled()?;
    let (mut set, codes) = fixture(&c, ONE, TWO, true)?;
    set.pages[0].registration.route = "deep/a.html".into();
    set.pages[1].registration.route = "deep/b.html".into();
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
    for parallel in [
        ParallelMode::Rows,
        ParallelMode::Columns,
        ParallelMode::Single {
            language: "fr".into(),
            fallbacks: vec!["ja".into()],
        },
    ] {
        let options = RenderOptions { parallel };
        let mut b = crate::doc::source::budget();
        b.charge(Resource::Work, 19).map_err(err)?;
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
        let mut maths = [Vec::new(), Vec::new()];
        let initial = b.usage();
        let output = package::render_embedded(
            &set,
            &options,
            &plans,
            &inputs,
            &mut host,
            &mut |page, view, embed, codec, b| {
                let p = usize::try_from(page).map_err(err)?;
                if view.kind == EmbedKind::Code {
                    codes[p].render_with_codec(view, embed, codec, b)
                } else {
                    crate::doc::export::math::DisplayHost {
                        compiled: &c,
                        registry: &c.doc.registry,
                        codec,
                        preference: crate::doc::export::MathRenderer::MathMLOnly,
                    }
                    .render(view, embed, &mut maths[p], b)
                }
            },
            &mut b,
        )
        .map_err(err)?;
        assert_eq!(output.rendered.alt_projections.len(), 2);
        assert_eq!(output.rendered.sentences.len(), 2);
        assert!(!output.rendered.sentences[0].is_empty());
        assert_eq!(output.files.len(), 4);
        let first = String::from_utf8(output.files["deep/a.html"].clone()).map_err(err)?;
        let second = String::from_utf8(output.files["deep/b.html"].clone()).map_err(err)?;
        assert!(first.contains("img-src data:;"));
        assert!(first.contains("data:image/svg+xml;base64,"));
        assert!(first.contains("b.html#n-6775657374"));
        assert!(second.contains("a.html#n-636170"));
        assert!(first.contains("<math"));
        let styles: Vec<_> = output
            .files
            .iter()
            .filter(|(p, _)| p.ends_with(".css"))
            .collect();
        assert_eq!(styles.len(), 2);
        assert_ne!(styles[0].1, styles[1].1);
        for (path, _) in styles {
            let name = path.strip_prefix("deep/").ok_or("css path")?;
            assert!(first.contains(name) || second.contains(name));
        }
        let receipt: serde_json::Value = serde_json::from_str(&output.manifest).map_err(err)?;
        assert_eq!(receipt["katex_generation"], false);
        assert_eq!(receipt["files"].as_array().ok_or("files")?.len(), 4);
        for record in receipt["files"].as_array().ok_or("files")? {
            let path = record["path"].as_str().ok_or("path")?;
            assert_eq!(
                record["sha256"],
                crate::doc::export::digest_hex(Digest::of(&output.files[path]))
            );
        }
        assert!(
            receipt["pre_receipt_usage"]["work"]
                .as_u64()
                .ok_or("usage")?
                < b.usage().work
        );
        assert_eq!(
            receipt["initial_usage"]["work"].as_u64(),
            Some(initial.work)
        );
        let files = output
            .files
            .into_iter()
            .map(|(p, bytes)| Ok((p, String::from_utf8(bytes).map_err(err)?)))
            .collect::<Result<std::collections::BTreeMap<_, _>, String>>()?;
        println!(
            "NEPL3_PAGES_SVG_CORPUS {}",
            serde_json::json!({"mode": receipt["display"]["mode"], "files": files, "manifest": output.manifest})
        );
    }
    Ok(())
}

#[test]
fn packaged_svg_input_identity_binds_bytes_page_and_requested_display() -> Result<(), String> {
    let c = compiled()?;
    let (set, _) = fixture(&c, ONE, TWO, false)?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&c.doc.registry, &store, &mut admission).map_err(err)?;
    let mut b = crate::doc::source::budget();
    let options = RenderOptions {
        parallel: ParallelMode::Rows,
    };
    let a = [SvgInput { id: "logo", svg: A }];
    let bb = [SvgInput { id: "logo", svg: B }];
    let base = package::identity::input(
        &set,
        &options,
        &[planned::RootAssets {
            page: 0,
            inputs: &a,
        }],
        &c.doc.registry,
        &mut codec,
        &mut b,
    )?;
    for binding in [
        planned::RootAssets {
            page: 0,
            inputs: &bb,
        },
        planned::RootAssets {
            page: 1,
            inputs: &a,
        },
    ] {
        assert_ne!(
            base,
            package::identity::input(
                &set,
                &options,
                &[binding],
                &c.doc.registry,
                &mut codec,
                &mut b
            )?
        );
    }
    let mut previous = None;
    for fallbacks in [
        vec!["en".into(), "ja".into()],
        vec!["ja".into(), "en".into()],
    ] {
        let options = RenderOptions {
            parallel: ParallelMode::Single {
                language: "fr".into(),
                fallbacks,
            },
        };
        let digest = package::identity::input(
            &set,
            &options,
            &[planned::RootAssets {
                page: 0,
                inputs: &a,
            }],
            &c.doc.registry,
            &mut codec,
            &mut b,
        )?;
        assert_ne!(base, digest);
        if let Some(previous) = previous {
            assert_ne!(previous, digest);
        }
        previous = Some(digest);
    }
    Ok(())
}
