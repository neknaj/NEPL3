use super::*;

#[test]
fn host_context_identity_tracks_authorized_content_and_ignores_transport_order()
-> Result<(), String> {
    let (registry, mut call) = setup()?;
    call.sources.push(
        SourceSnapshot::new(
            SourceId("second".into()),
            1,
            "memory:second".into(),
            b"text".to_vec(),
            &mut budget(),
        )
        .map_err(error)?,
    );
    call.resources.push(ResourceContent {
        id: "other".into(),
        digest: Digest::of(b"xyz"),
        bytes: b"xyz".to_vec(),
    });
    let configuration = Digest::of(b"host policy v1");
    let original = context_digest(&call, configuration, &registry, &mut budget()).map_err(error)?;
    let mut reordered = call.clone();
    reordered.sources.reverse();
    reordered.resources.reverse();
    reordered.sources.push(reordered.sources[0].clone());
    reordered.request_id += 1;
    reordered.limits.work += 1;
    reordered.input = reordered.environment.clone();
    assert_eq!(
        context_digest(&reordered, configuration, &registry, &mut budget()).map_err(error)?,
        original
    );
    let mut graph = nepl3_core::operation::lifetime::RequestLifetimes::default();
    graph
        .begin_call(&call, original, None, &mut budget())
        .map_err(error)?;
    reordered.input = call.input.clone();
    let reordered_context =
        context_digest(&reordered, configuration, &registry, &mut budget()).map_err(error)?;
    assert_eq!(
        graph.begin_call(
            &reordered,
            reordered_context,
            Some(call.request_id),
            &mut budget()
        ),
        Err(nepl3_core::operation::lifetime::LifetimeError::CyclicOperation)
    );
    for kind in ["environment", "source", "resource", "configuration"] {
        let mut changed = call.clone();
        let mut config = configuration;
        match kind {
            "environment" => changed.environment = changed.input.clone(),
            "source" => changed.sources.pop().map(|_| ()).ok_or("source")?,
            "resource" => {
                changed.resources[0].bytes = b"changed".to_vec();
                changed.resources[0].digest = Digest::of(b"changed");
            }
            _ => config = Digest::of(b"host policy v2"),
        }
        assert_ne!(
            context_digest(&changed, config, &registry, &mut budget()).map_err(error)?,
            original,
            "{kind}"
        );
    }
    let mut forged = call.clone();
    forged.resources[0].bytes.push(0);
    assert!(context_digest(&forged, configuration, &registry, &mut budget()).is_err());
    let mut duplicate = call.clone();
    duplicate.resources.push(duplicate.resources[0].clone());
    assert!(context_digest(&duplicate, configuration, &registry, &mut budget()).is_err());
    for reason in [
        StopReason::WorkLimit,
        StopReason::AllocationLimit,
        StopReason::Cancelled,
    ] {
        let mut limits = budget().limits();
        if reason == StopReason::WorkLimit {
            limits.work = 0;
        }
        if reason == StopReason::AllocationLimit {
            limits.allocation_units = 0;
        }
        let mut stopped = Budget::new(limits);
        if reason == StopReason::Cancelled {
            stopped.cancel();
        }
        assert!(context_digest(&call, configuration, &registry, &mut stopped).is_err());
        assert_eq!(stopped.poll(), Err(reason));
    }
    Ok(())
}
