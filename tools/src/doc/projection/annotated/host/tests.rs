use super::*;

#[test]
fn composed_single_document_keeps_caller_limits_and_cancellation() -> Result<(), String> {
    let compiled = crate::doc::source::compiled()?;
    let source = r#"article en sentence sentence cons doc anchor target text "Title" nil body cons paragraph cons sentence sentence cons doc ref target text "Label" nil nil nil"#;
    let mut measured = budget();
    measured.charge(Resource::Work, 7).map_err(err)?;
    let expected = from_source_with_budget(&compiled, source, &[], &mut measured)?;
    assert!(
        expected.markdown.contains("[Label](<#n-746172676574>)"),
        "{}",
        expected.markdown
    );
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::NodeLimit,
        StopReason::DepthLimit,
        StopReason::SourceLimit,
        StopReason::OutputLimit,
    ] {
        for below in [false, true] {
            let mut limits = budget().limits();
            let used = measured.usage();
            let (limit, used) = match reason {
                StopReason::WorkLimit => (&mut limits.work, used.work),
                StopReason::AllocationLimit => {
                    (&mut limits.allocation_units, used.allocation_units)
                }
                StopReason::NodeLimit => (&mut limits.nodes, used.nodes),
                StopReason::DepthLimit => (&mut limits.depth, used.depth),
                StopReason::SourceLimit => (&mut limits.source_bytes, used.source_bytes),
                StopReason::OutputLimit => (&mut limits.output_bytes, used.output_bytes),
                _ => unreachable!(),
            };
            *limit = used
                .checked_sub(u64::from(below))
                .ok_or("nonzero boundary")?;
            let mut receiver = Budget::new(limits);
            receiver.charge(Resource::Work, 7).map_err(err)?;
            let result = from_source_with_budget(&compiled, source, &[], &mut receiver);
            if below {
                assert!(result.is_err());
                assert_eq!(receiver.poll(), Err(reason));
                let usage = receiver.usage();
                assert!(from_source_with_budget(&compiled, source, &[], &mut receiver).is_err());
                assert_eq!(receiver.usage(), usage);
            } else {
                let actual = result?;
                assert_eq!(actual.markdown, expected.markdown);
                assert_eq!(actual.document_digest, expected.document_digest);
                assert_eq!(receiver.usage(), measured.usage());
            }
        }
    }
    let mut cancelled = budget();
    cancelled.cancel();
    let usage = cancelled.usage();
    assert!(from_source_with_budget(&compiled, source, &[], &mut cancelled).is_err());
    assert_eq!(cancelled.usage(), usage);
    assert_eq!(cancelled.poll(), Err(StopReason::Cancelled));
    Ok(())
}
