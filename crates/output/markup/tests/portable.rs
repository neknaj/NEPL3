use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::SchemaRegistry,
    source::{SourceAdmission, SourceStore},
    value::NdfValue,
};
use nepl3_markup::{
    html::*,
    portable::{self, PortableError},
};
use nepl3_wire::foundation::FoundationCodec;
fn b() -> Budget {
    Budget::new(Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 1_000_000,
        depth: 1000,
        output_bytes: 10_000_000,
        ..Limits::default()
    })
}
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [
        nepl3_core::schema::foundation::descriptor(&mut b()),
        nepl3_markup::schema::descriptor(&mut b()),
    ] {
        let d = d.map_err(err)?;
        r.register(d.reference(&mut b()).map_err(err)?, d, &mut b())
            .map_err(err)?;
    }
    r.finalize(&mut b()).map_err(err)?;
    Ok(r)
}
fn request() -> HtmlRequest {
    HtmlRequest {
        slot: HtmlSlot::Phrasing,
        policy: HtmlPolicy { classes: vec![] },
        fragment: HtmlFragment {
            root: 0,
            nodes: vec![
                HtmlNode::Element {
                    tag: HtmlTag::Span,
                    attributes: vec![
                        HtmlAttribute::Lang { value: "ja".into() },
                        HtmlAttribute::Id {
                            value: "1-日本%20".into(),
                        },
                    ],
                    children: vec![1],
                },
                HtmlNode::Text {
                    text: "あ🙂<&\r\n".into(),
                },
            ],
        },
    }
}

#[test]
fn first_cbor_receiver_revalidates_owned_markup_and_matches_native_output() -> Result<(), String> {
    let r = registry()?;
    let original = request();
    let bytes = {
        let store = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
        let value = portable::to_value(&original, &r, &mut c, &mut b()).map_err(err)?;
        nepl3_wire::encode(&value, &mut b()).map_err(err)?
    };
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let received = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
    let actual = portable::from_value(&received, &r, &mut c, &mut b()).map_err(err)?;
    assert_eq!(actual, original);
    let proof = validate(&actual.fragment, actual.slot, &actual.policy, &mut b()).map_err(err)?;
    assert_eq!(
        serialize(&proof, &mut b()).map_err(err)?,
        "<span id=\"1-日本%20\" lang=\"ja\">あ🙂&lt;&amp;&#xD;\n</span>"
    );
    assert_eq!(
        nepl3_wire::encode(
            &portable::to_value(&actual, &r, &mut c, &mut b()).map_err(err)?,
            &mut b()
        )
        .map_err(err)?,
        bytes
    );
    Ok(())
}

#[test]
fn schema_valid_invalid_markup_and_unknown_tags_are_rejected_after_cbor() -> Result<(), String> {
    let r = registry()?;
    let store = SourceStore::default();
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &store, &mut a).map_err(err)?;
    let value = portable::to_value(&request(), &r, &mut c, &mut b()).map_err(err)?;
    for bad in 0..4 {
        let mut v = value.clone();
        let NdfValue::Record(req) = &mut v else {
            return Err("request".into());
        };
        let NdfValue::Record(fragment) = &mut req.fields[0] else {
            return Err("fragment".into());
        };
        let NdfValue::List(nodes) = &mut fragment.fields[1] else {
            return Err("nodes".into());
        };
        if bad == 0 {
            fragment.fields[0] = NdfValue::U64(u64::MAX);
        } else if bad == 1 {
            let NdfValue::Variant(text) = &mut nodes[1] else {
                return Err("text".into());
            };
            text.fields[0] = NdfValue::Text("あ\0".into());
        } else {
            let NdfValue::Variant(element) = &mut nodes[0] else {
                return Err("element".into());
            };
            if bad == 2 {
                let NdfValue::Variant(tag) = &mut element.fields[0] else {
                    return Err("tag".into());
                };
                tag.variant = "Script".into();
            } else {
                element.fields[2] = NdfValue::List(vec![NdfValue::U64(0)]);
            }
        }
        let bytes = nepl3_wire::encode(&v, &mut b()).map_err(err)?;
        let decoded = nepl3_wire::decode(&bytes, &mut b()).map_err(err)?;
        assert!(
            portable::from_value(&decoded, &r, &mut c, &mut b()).is_err(),
            "mutation {bad}"
        );
    }
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::DepthLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = b().limits();
        match reason {
            StopReason::WorkLimit => limits.work = 0,
            StopReason::AllocationLimit => limits.allocation_units = 0,
            StopReason::DepthLimit => limits.depth = 0,
            _ => {}
        }
        let mut budget = Budget::new(limits);
        if reason == StopReason::Cancelled {
            budget.cancel();
        }
        assert_eq!(
            portable::from_value(&value, &r, &mut c, &mut budget),
            Err(PortableError::Stopped(reason))
        );
        assert_eq!(
            portable::from_value(&value, &r, &mut c, &mut budget),
            Err(PortableError::Stopped(reason))
        );
    }
    Ok(())
}
