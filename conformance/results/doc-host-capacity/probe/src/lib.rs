#[cfg(test)]
mod tests {
    use nepl3_core::{budget::{Budget, Limits, Resource, StopReason}, source::{SourceAdmission, SourceId, SourceSnapshot}};
    use nepl3_tools::doc::{export, source};

    fn input(unique: bool) -> String {
        let mut text = String::from("article ja \"T\" body ");
        for paragraph in 0..128 {
            text.push_str("cons paragraph ");
            for sentence in 0..4 {
                text.push_str("cons \"[\u{6587}/\u{3076}\u{3093}]");
                if unique { text.push_str(&format!("item{:03}", paragraph * 4 + sentence)); }
                text.push_str("\u{3002}\" ");
            }
            text.push_str("nil ");
        }
        text.push_str("nil");
        text
    }

    #[test]
    fn old_and_new_caps_real_parser_and_sticky_other_limits() -> Result<(), String> {
        let compiled = source::compiled()?;
        let original = input(false);
        let expected = Limits { source_bytes: 10_000_000, work: 100_000_000, depth: 1000,
            nodes: 10_000_000, allocation_units: 500_000_000, output_bytes: 10_000_000,
            diagnostics: 1000, events: 1000 };
        assert_eq!(source::budget().limits(), expected);
        source::with_input_route(true, &compiled, "article ja \"T\" body nil", "Article", |_, profile, _, _| {
            let mut stops = Vec::new();
            let mut cases = vec![("old nodes", Limits { nodes: 1_000_000, ..expected }, Some(StopReason::NodeLimit)),
                ("new nodes", expected, None),
                ("work", Limits { work: 10_000, ..expected }, Some(StopReason::WorkLimit)),
                ("allocation", Limits { allocation_units: 1_000, ..expected }, Some(StopReason::AllocationLimit)),
                ("source", Limits { source_bytes: 1, ..expected }, Some(StopReason::SourceLimit)),
                ("depth", Limits { depth: 0, ..expected }, Some(StopReason::DepthLimit)),
                ("output", Limits { output_bytes: 0, ..expected }, Some(StopReason::OutputLimit))];
            // Cancellation is prior to entry, not inferred from a later failure.
            cases.push(("cancel", expected, Some(StopReason::Cancelled)));
            for (label, limits, wanted) in cases {
                let mut b = Budget::new(limits);
                let mut admission = SourceAdmission::default();
                if label == "cancel" { b.stop(StopReason::Cancelled); }
                let result = (|| {
                    let snapshot = SourceSnapshot::new(SourceId("capacity-original".into()), 7,
                        "memory:capacity-original".into(), original.as_bytes().to_vec(), &mut b).map_err(source::err)?;
                    let tree = source::parse_source_route(&snapshot, profile, "Doc", "Article", &mut b, &mut admission, true)?;
                    let _checked = tree.validate(profile, &mut b, &mut admission).map_err(source::err)?;
                    assert!(tree.bundle.sources.iter().any(|s| s.identity() == snapshot.identity() && s.text() == original));
                    assert!(tree.bundle.nodes.len() < 10_000);
                    Ok::<_,String>(tree.bundle.nodes.len())
                })();
                println!("case={label}; result={result:?}; usage={:?}", b.usage());
                match wanted {
                    None => { assert!(result.is_ok()); assert!(b.usage().nodes > 1_000_000); }
                    Some(reason) => {
                        assert!(result.is_err(), "{label}");
                        assert_eq!(b.poll(), Err(reason), "{label}");
                        let before = b.usage();
                        assert_eq!(b.charge(Resource::Nodes, 0), Err(reason));
                        assert_eq!(b.charge(Resource::Work, 1), Err(reason));
                        assert_eq!(b.usage(), before);
                        assert_eq!(b.limits(), limits);
                        stops.push(reason);
                    }
                }
            }
            assert_eq!(stops.len(), 7);
            // Unused event/diagnostic quotas are unchanged too; they remain real ceilings.
            for resource in [Resource::Diagnostics, Resource::Events] {
                let mut b=source::budget();
                assert!(b.charge(resource, 1000).is_ok());
                assert!(b.charge(resource, 1).is_err());
                let stopped=b.poll();
                assert_eq!(b.charge(Resource::Nodes, 0), stopped);
            }
            Ok(())
        })
    }

    #[test]
    fn distinct_authored_sentences_are_all_exported_in_order() -> Result<(), String> {
        let original = input(true);
        let output = export::generate(&source::compiled()?, &original)?;
        assert_eq!(output.html.matches("class=\"nepl-ruby\"").count(), 512);
        assert_eq!(output.html.matches("\u{3076}\u{3093}").count(), 512);
        let mut next = 0;
        for index in 0..512 {
            let marker = format!("item{index:03}\u{3002}");
            assert_eq!(output.html.matches(&marker).count(), 1);
            let offset = output.html[next..].find(&marker).ok_or("ordered sentence")?;
            next += offset + marker.len();
        }
        assert!(!output.html.contains("<script"));
        println!("source_bytes={}; html_bytes={}; 512 distinct Ruby sentences retained in order",original.len(),output.html.len());
        Ok(())
    }
}
