use nepl3_core::budget::{Budget, Limits};
use nepl3_markup::html::*;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        output_bytes: 100_000,
        nodes: 1000,
        depth: 100,
        ..Limits::default()
    })
}
fn input(id: &str, href: HtmlHref) -> HtmlFragment {
    HtmlFragment {
        root: 0,
        nodes: vec![
            HtmlNode::Element {
                tag: HtmlTag::Div,
                attributes: vec![HtmlAttribute::Id { value: id.into() }],
                children: vec![1],
            },
            HtmlNode::Element {
                tag: HtmlTag::A,
                attributes: vec![HtmlAttribute::Href { value: href }],
                children: vec![2],
            },
            HtmlNode::Text { text: "go".into() },
        ],
    }
}
#[test]
fn decoded_ids_and_all_internal_href_variants_preserve_unicode() -> Result<(), HtmlError> {
    for (id, encoded) in [
        ("1-方針", "1-%E6%96%B9%E9%87%9D"),
        ("x%20#?", "x%2520%23%3F"),
        ("A_b.~", "A_b.~"),
        ("🙂", "%F0%9F%99%82"),
        ("e\u{301}", "e%CC%81"),
    ] {
        for (href, prefix) in [
            (HtmlHref::Fragment { id: id.into() }, ""),
            (
                HtmlHref::Artifact {
                    path: "page.html".into(),
                    fragment: Some(id.into()),
                },
                "page.html",
            ),
            (
                HtmlHref::BetweenArtifacts {
                    source: "a/index.html".into(),
                    target: "b/index.html".into(),
                    fragment: Some(id.into()),
                },
                "../b/index.html",
            ),
        ] {
            let f = input(id, href);
            let mut b = budget();
            let checked = validate(&f, HtmlSlot::Block, &HtmlPolicy { classes: vec![] }, &mut b)?;
            assert_eq!(
                serialize(&checked, &mut b)?,
                format!("<div id=\"{id}\"><a href=\"{prefix}#{encoded}\">go</a></div>")
            );
        }
    }
    Ok(())
}
#[test]
fn anchor_extension_does_not_relax_data_or_css_tokens() {
    let policy = HtmlPolicy { classes: vec![] };
    for id in ["", "a b", "a\tb", "a\nb", "a\rb", "a\u{7f}b", "a\u{85}b"] {
        let f = input(id, HtmlHref::Fragment { id: id.into() });
        assert!(validate(&f, HtmlSlot::Block, &policy, &mut budget()).is_err());
    }
    for attr in [
        HtmlAttribute::DataId {
            value: "日本".into(),
        },
        HtmlAttribute::DataGroup {
            value: "1-x".into(),
        },
    ] {
        let mut f = input("valid", HtmlHref::Fragment { id: "valid".into() });
        if let HtmlNode::Element { attributes, .. } = &mut f.nodes[0] {
            attributes.push(attr);
        }
        assert!(matches!(
            validate(&f, HtmlSlot::Block, &policy, &mut budget()),
            Err(HtmlError::Attribute { .. })
        ));
    }
    let f = input("valid", HtmlHref::Fragment { id: "valid".into() });
    assert!(matches!(
        validate(
            &f,
            HtmlSlot::Block,
            &HtmlPolicy {
                classes: vec!["日本".into()]
            },
            &mut budget()
        ),
        Err(HtmlError::Policy)
    ));
    let f = input(
        "é",
        HtmlHref::Fragment {
            id: "e\u{301}".into(),
        },
    );
    assert!(matches!(
        validate(&f, HtmlSlot::Block, &policy, &mut budget()),
        Err(HtmlError::MissingFragment(_))
    ));
}
