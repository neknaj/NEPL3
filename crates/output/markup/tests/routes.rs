use nepl3_core::{
    budget::{Budget, Limits, Resource, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_markup::{html::*, portable};
use nepl3_wire::foundation::FoundationCodec;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        output_bytes: 100_000,
        nodes: 100_000,
        depth: 100,
        ..Limits::default()
    })
}
fn request(source: &str, target: &str) -> HtmlRequest {
    HtmlRequest {
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
        fragment: HtmlFragment {
            root: 0,
            nodes: vec![
                HtmlNode::Element {
                    tag: HtmlTag::A,
                    attributes: vec![HtmlAttribute::Href {
                        value: HtmlHref::BetweenArtifacts {
                            source: source.into(),
                            target: target.into(),
                            fragment: Some("n-a".into()),
                        },
                    }],
                    children: vec![1],
                },
                HtmlNode::Text {
                    text: "next".into(),
                },
            ],
        },
    }
}
fn render(v: &HtmlRequest, b: &mut Budget) -> Result<String, HtmlError> {
    serialize(&validate(&v.fragment, v.slot, &v.policy, b)?, b)
}
#[test]
fn relative_routes_preserve_complete_directory_segments() -> Result<(), HtmlError> {
    for (source, target, href) in [
        ("index.html", "docs/index.html", "docs/index.html"),
        ("docs/a/index.html", "docs/b/index.html", "../b/index.html"),
        ("docs/a/index.html", "index.html", "../../index.html"),
        ("docs/index.html", "docs/index.html", "index.html"),
        (
            "docs/a/index.html",
            "docs/ab/index.html",
            "../ab/index.html",
        ),
        ("a/b/c.html", "a/b/d/e.html", "d/e.html"),
        ("a/index.html", "a", "../a"),
    ] {
        assert_eq!(
            render(&request(source, target), &mut budget())?,
            format!("<a href=\"{href}#n-a\">next</a>")
        );
    }
    Ok(())
}
#[test]
fn raw_routes_do_not_authorize_parent_segments_or_url_syntax() {
    for invalid in [
        "",
        "/root",
        "a//b",
        ".",
        "..",
        "a/../b",
        "a/./b",
        "https://evil/a",
        "a:b",
        "%2e%2e/a",
        "a\\b",
        "a#b",
        "a?b",
        "a\nb",
        "\u{65e5}/a",
    ] {
        for v in [
            request(invalid, "index.html"),
            request("index.html", invalid),
        ] {
            assert!(matches!(
                render(&v, &mut budget()),
                Err(HtmlError::Attribute { .. })
            ));
        }
    }
}
#[test]
fn route_serialization_keeps_resource_stops_and_input() -> Result<(), HtmlError> {
    let v = request("docs/a/index.html", "reference/grammar/index.html");
    let original = v.clone();
    let mut measured = budget();
    let expected = render(&v, &mut measured)?;
    for (resource, used, reason) in [
        (Resource::Work, measured.usage().work, StopReason::WorkLimit),
        (
            Resource::AllocationUnits,
            measured.usage().allocation_units,
            StopReason::AllocationLimit,
        ),
        (
            Resource::OutputBytes,
            measured.usage().output_bytes,
            StopReason::OutputLimit,
        ),
    ] {
        for cap in [0, used / 2, used - 1, used] {
            let mut limits = budget().limits();
            match resource {
                Resource::Work => limits.work = cap,
                Resource::AllocationUnits => limits.allocation_units = cap,
                _ => limits.output_bytes = cap,
            }
            let mut b = Budget::new(limits);
            let actual = render(&v, &mut b);
            if cap == used {
                assert_eq!(actual?, expected);
            } else {
                assert_eq!(actual, Err(HtmlError::Stopped(reason)));
                assert_eq!(render(&v, &mut b), Err(HtmlError::Stopped(reason)));
            }
            assert_eq!(v, original);
        }
    }
    let mut b = budget();
    b.cancel();
    assert_eq!(
        render(&v, &mut b),
        Err(HtmlError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn route_first_cbor_receiver_checks_both_paths() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut budget()),
        nepl3_markup::schema::descriptor(&mut budget()),
    ] {
        let d = d.map_err(err)?;
        registry
            .register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
            .map_err(err)?;
    }
    registry.finalize(&mut budget()).map_err(err)?;
    let v = request("docs/a/index.html", "docs/b/index.html");
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut c = FoundationCodec::new(&registry, &store, &mut admission).map_err(err)?;
    let native = portable::to_value(&v, &registry, &mut c, &mut budget()).map_err(err)?;
    for mutation in [None, Some(0), Some(1)] {
        let mut value = native.clone();
        if let Some(index) = mutation {
            let NdfValue::Record(req) = &mut value else {
                return Err("request".into());
            };
            let NdfValue::Record(fragment) = &mut req.fields[0] else {
                return Err("fragment".into());
            };
            let NdfValue::List(nodes) = &mut fragment.fields[1] else {
                return Err("nodes".into());
            };
            let NdfValue::Variant(element) = &mut nodes[0] else {
                return Err("element".into());
            };
            let NdfValue::List(attrs) = &mut element.fields[1] else {
                return Err("attrs".into());
            };
            let NdfValue::Variant(href) = &mut attrs[0] else {
                return Err("href".into());
            };
            let NdfValue::Variant(route) = &mut href.fields[0] else {
                return Err("route".into());
            };
            route.fields[index] = NdfValue::Text("../outside.html".into());
        }
        let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
        let received = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
        let empty = SourceStore::default();
        let mut fresh = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(&registry, &empty, &mut fresh).map_err(err)?;
        let restored = portable::from_value(&received, &registry, &mut receiver, &mut budget());
        if mutation.is_some() {
            assert!(restored.is_err());
        } else {
            assert_eq!(restored.map_err(err)?, v);
        }
    }
    Ok(())
}
