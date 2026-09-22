use super::*;
use alloc::vec;
use nepl3_core::budget::Limits;
use nepl3_markup::html::HtmlTag;

fn limits() -> Limits {
    Limits {
        work: 1_000_000,
        allocation_units: 1_000_000,
        ..Limits::default()
    }
}

fn anchor(id: &str) -> HtmlNode {
    HtmlNode::Element {
        tag: HtmlTag::Span,
        attributes: vec![HtmlAttribute::Id { value: id.into() }],
        children: vec![],
    }
}

#[test]
fn text_growth_and_repeated_links_do_not_multiply_scans() -> Result<(), StopReason> {
    let mut work = Vec::new();
    for count in [128, 256, 512] {
        // An anchor after a growing text body is queried count times. Looking
        // through all output nodes on each query requires quadratic work.
        let mut nodes = vec![
            HtmlNode::Text {
                text: "body".into()
            };
            count
        ];
        nodes.push(anchor("destination"));
        let mut b = Budget::new(limits());
        let ids = OutputAnchors::collect(&nodes, &mut b)?;
        for _ in 0..count {
            assert!(ids.contains("destination", &mut b)?);
            assert!(!ids.contains("missing", &mut b)?);
        }
        work.push(b.usage().work);
    }
    assert!(work[1] <= 2 * work[0]);
    assert!(work[2] <= 2 * work[1]);
    Ok(())
}

#[test]
fn exact_ids_and_budget_boundaries() -> Result<(), StopReason> {
    let nodes = vec![
        HtmlNode::Text {
            text: "hidden".into(),
        },
        anchor("A"),
        anchor("B"),
    ];
    let mut measured = Budget::new(limits());
    let ids = OutputAnchors::collect(&nodes, &mut measured)?;
    let allocation = measured.usage().allocation_units;
    assert!(ids.contains("B", &mut measured)?);
    assert!(!ids.contains("b", &mut measured)?);
    assert!(!ids.contains("hidden", &mut measured)?);
    let work = measured.usage().work;
    let mut exact = Budget::new(Limits {
        work,
        allocation_units: allocation,
        ..limits()
    });
    let ids = OutputAnchors::collect(&nodes, &mut exact)?;
    assert!(ids.contains("B", &mut exact)?);
    assert!(!ids.contains("b", &mut exact)?);
    assert!(!ids.contains("hidden", &mut exact)?);
    let mut short = Budget::new(Limits {
        allocation_units: allocation - 1,
        ..limits()
    });
    assert!(matches!(
        OutputAnchors::collect(&nodes, &mut short),
        Err(StopReason::AllocationLimit)
    ));
    let mut stopped = Budget::new(Limits {
        work: 0,
        ..limits()
    });
    assert_eq!(ids.contains("B", &mut stopped), Err(StopReason::WorkLimit));
    let mut cancelled = Budget::new(limits());
    cancelled.cancel();
    assert!(matches!(
        OutputAnchors::collect(&[], &mut cancelled),
        Err(StopReason::Cancelled)
    ));
    Ok(())
}
