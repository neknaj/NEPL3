use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_markup::html::*;

fn limits() -> Limits {
    Limits {
        work: 100_000_000,
        allocation_units: 100_000_000,
        nodes: 100_000,
        depth: 100,
        ..Limits::default()
    }
}

fn input(ids: usize, links: usize) -> HtmlFragment {
    let mut nodes = vec![HtmlNode::Element {
        tag: HtmlTag::Div,
        attributes: vec![],
        children: (1..=ids + links).map(|i| i as u64).collect(),
    }];
    // Forward references and reverse ID order exercise resolution independently
    // of traversal order. Fixed-width Unicode names keep key length constant.
    for i in 0..links {
        nodes.push(HtmlNode::Element {
            tag: HtmlTag::A,
            attributes: vec![HtmlAttribute::Href {
                value: HtmlHref::Fragment {
                    id: format!("節-{:06}", i % ids),
                },
            }],
            children: vec![],
        });
    }
    for i in (0..ids).rev() {
        nodes.push(HtmlNode::Element {
            tag: HtmlTag::Span,
            attributes: vec![HtmlAttribute::Id {
                value: format!("節-{i:06}"),
            }],
            children: vec![],
        });
    }
    HtmlFragment { root: 0, nodes }
}

fn work(f: &HtmlFragment) -> Result<u64, HtmlError> {
    let mut b = Budget::new(limits());
    validate(f, HtmlSlot::Block, &HtmlPolicy { classes: vec![] }, &mut b)?;
    Ok(b.usage().work)
}

#[test]
fn id_count_and_reference_count_scale_independently() -> Result<(), HtmlError> {
    for links in [0, 512] {
        let values = [128, 512, 2048].map(|ids| work(&input(ids, links)));
        let [a, b, c] = values;
        let (a, b, c) = (a?, b?, c?);
        // A fourfold increase permits n log n growth (<6x), while rejecting
        // the former all-ID work charge at every insertion (approximately16x).
        assert!(b < 6 * a && c < 6 * b, "{a}, {b}, {c}");
    }
    let base = work(&input(512, 0))?;
    let a = work(&input(512, 128))? - base;
    let b = work(&input(512, 256))? - base;
    let c = work(&input(512, 512))? - base;
    assert!(b < 3 * a && c < 3 * b, "{a}, {b}, {c}");
    Ok(())
}

#[test]
fn full_validation_work_boundary_and_input_preservation() -> Result<(), HtmlError> {
    let f = input(128, 128);
    let original = f.clone();
    let required = work(&f)?;
    let mut b = Budget::new(Limits {
        work: required - 1,
        ..limits()
    });
    assert!(matches!(
        validate(&f, HtmlSlot::Block, &HtmlPolicy { classes: vec![] }, &mut b),
        Err(HtmlError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    let mut b = Budget::new(Limits {
        work: required,
        ..limits()
    });
    validate(&f, HtmlSlot::Block, &HtmlPolicy { classes: vec![] }, &mut b)?;
    assert_eq!(f, original);
    Ok(())
}

#[test]
fn duplicate_reports_first_repeated_occurrence_and_missing_links_keep_order() {
    let mut f = input(4, 0);
    for (index, name) in [(1, "z"), (2, "a"), (3, "z"), (4, "a")] {
        if let HtmlNode::Element { attributes, .. } = &mut f.nodes[index] {
            *attributes = vec![HtmlAttribute::Id { value: name.into() }];
        }
    }
    assert_eq!(work(&f), Err(HtmlError::DuplicateId(3)));
    let mut f = input(1, 2);
    for (index, name) in [(1, "z-missing"), (2, "a-missing")] {
        if let HtmlNode::Element { attributes, .. } = &mut f.nodes[index] {
            *attributes = vec![HtmlAttribute::Href {
                value: HtmlHref::Fragment { id: name.into() },
            }];
        }
    }
    assert_eq!(work(&f), Err(HtmlError::MissingFragment(1)));
}
