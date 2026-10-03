use super::*;
use nepl3_engine::{
    analysis::{BindingAccessError, BindingOptions, trace::ReferenceTraceAccessError},
    binding::trace::TraceAccessError,
    portable::analysis as keyed,
};

#[test]
fn keyed_reference_trace_checks_all_key_digests_and_preserves_native_report() -> Result<(), String>
{
    let compiled = execution()?;
    with_input(&compiled, "lambda x x", |tree, profile, b, a| {
        let empty = SourceStore::default();
        let prepared = {
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            keyed::prepare(
                "keyed-trace",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?
        };
        let bound = prepared.trace_references(b, a).map_err(err)?;
        assert_eq!(bound.key(), prepared.key());
        let original_report = bound.trace().reply().report.clone();
        let mut query = budget();
        assert!(core::ptr::eq(
            bound.for_key(&prepared.key(), &mut query).map_err(err)?,
            bound.trace()
        ));
        assert_eq!(query.usage().work, 128);
        let value = bound
            .final_reference(&prepared.key(), 0, &mut query)
            .map_err(err)?;
        assert_eq!(value.occurrence.name, "x");
        assert!(matches!(
            value.occurrence.resolution,
            ReferenceResolution::Resolved(_)
        ));
        for index in 0..4 {
            let mut wrong = prepared.key();
            match index {
                0 => wrong.tree_digest.0[0] ^= 1,
                1 => wrong.profile_digest.0[0] ^= 1,
                2 => wrong.execution_digest.0[0] ^= 1,
                _ => wrong.request_digest.0[0] ^= 1,
            }
            assert_eq!(
                bound.for_key(&wrong, &mut budget()).err(),
                Some(BindingAccessError::StaleAnalysis)
            );
            assert!(matches!(
                bound.final_reference(&wrong, 0, &mut budget()),
                Err(ReferenceTraceAccessError::Access(
                    BindingAccessError::StaleAnalysis
                ))
            ));
            let mut cancelled = budget();
            cancelled.cancel();
            assert_eq!(
                bound.for_key(&wrong, &mut cancelled).err(),
                Some(BindingAccessError::Stopped(StopReason::Cancelled))
            );
        }
        let mut wrong_limits = b.limits();
        wrong_limits.depth -= 1;
        let mut changed = Budget::new(wrong_limits);
        changed.cancel();
        assert_eq!(
            bound.for_key(&prepared.key(), &mut changed).err(),
            Some(BindingAccessError::LimitsMismatch)
        );
        assert_eq!(changed.usage().work, 0);
        assert!(matches!(
            prepared.trace_references(&mut changed, &mut SourceAdmission::default()),
            Err(BindingAccessError::LimitsMismatch)
        ));
        let mut short = budget();
        short
            .charge(Resource::Work, short.limits().work - 127)
            .map_err(err)?;
        assert_eq!(
            bound.for_key(&prepared.key(), &mut short).err(),
            Some(BindingAccessError::Stopped(StopReason::WorkLimit))
        );
        assert!(matches!(
            bound.final_reference(&prepared.key(), usize::MAX, &mut budget()),
            Err(ReferenceTraceAccessError::Trace(
                TraceAccessError::MissingRecord
            ))
        ));
        let mut native_measure = budget();
        bound
            .trace()
            .final_reference(0, &mut native_measure)
            .map_err(err)?;
        let mut short_native = budget();
        short_native
            .charge(
                Resource::Work,
                short_native.limits().work - 128 - native_measure.usage().work + 1,
            )
            .map_err(err)?;
        assert!(matches!(
            bound.final_reference(&prepared.key(), 0, &mut short_native),
            Err(ReferenceTraceAccessError::Trace(TraceAccessError::Stopped(
                StopReason::WorkLimit
            )))
        ));
        let mut short_nodes = budget();
        short_nodes
            .charge(
                Resource::Nodes,
                short_nodes.limits().nodes - native_measure.usage().nodes + 1,
            )
            .map_err(err)?;
        assert!(matches!(
            bound.final_reference(&prepared.key(), 0, &mut short_nodes),
            Err(ReferenceTraceAccessError::Trace(TraceAccessError::Stopped(
                StopReason::NodeLimit
            )))
        ));
        assert_eq!(bound.trace().reply().report, original_report);
        let raw = bound.into_trace();
        assert!(raw.complete().is_some());
        Ok(())
    })
}

#[test]
fn keyed_reference_trace_preserves_invalid_stopped_and_explicit_host_execution()
-> Result<(), String> {
    let compiled = custom::compiled()?;
    with_input(&compiled, "early x z custom x x", |tree, profile, b, a| {
        let empty = SourceStore::default();
        let prepared = {
            let mut codec = FoundationCodec::new(profile.registry(), &empty, a).map_err(err)?;
            keyed::prepare(
                "keyed-trace",
                tree.tree(),
                BindingOptions,
                b.limits(),
                profile,
                &mut codec,
                b,
            )
            .map_err(err)?
        };
        let invalid = prepared.trace_references(b, a).map_err(err)?;
        assert!(matches!(
            invalid
                .for_key(&prepared.key(), &mut budget())
                .map_err(err)?
                .reply()
                .outcome,
            BindingOutcome::Invalid {
                error: BindingError::MissingProvider,
                ..
            }
        ));
        assert!(matches!(
            invalid.final_reference(&prepared.key(), 0, &mut budget()),
            Err(ReferenceTraceAccessError::Trace(
                TraceAccessError::Incomplete
            ))
        ));
        let mut cancelled = budget();
        cancelled.cancel();
        let stopped = prepared
            .trace_references(&mut cancelled, &mut SourceAdmission::default())
            .map_err(err)?;
        assert!(matches!(
            stopped
                .for_key(&prepared.key(), &mut budget())
                .map_err(err)?
                .reply()
                .outcome,
            BindingOutcome::Stopped {
                reason: StopReason::Cancelled,
                ..
            }
        ));
        let mut host = custom::query_host(true, false);
        let complete = prepared
            .trace_references_with_host(&mut host, &mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        assert_eq!(complete.key(), invalid.key());
        // Key equality does not turn independently executed envelopes into one proof.
        assert!(!core::ptr::eq(complete.trace(), invalid.trace()));
        assert_eq!(
            complete
                .final_reference(&prepared.key(), 0, &mut budget())
                .map_err(err)?
                .occurrence
                .resolution,
            ReferenceResolution::Resolved(EntityId(100))
        );
        Ok(())
    })
}
