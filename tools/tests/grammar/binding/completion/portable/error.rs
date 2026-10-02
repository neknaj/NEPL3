use super::*;
use nepl3_engine::portable::completion::error as failure;

#[test]
fn candidate_failure_transport_preserves_typed_causes_and_remote_stops() -> Result<(), String> {
    let compiled = execution()?;
    with_input(&compiled, "lambda a probe", |tree, profile, _, _| {
        let empty = SourceStore::default();
        let mut a = SourceAdmission::default();
        let mut codec = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
        let prepared = keyed::prepare(
            "candidate-errors",
            tree.tree(),
            BindingOptions,
            budget().limits(),
            profile,
            &mut codec,
            &mut budget(),
        )
        .map_err(err)?;
        let bound = prepared
            .execute(&mut budget(), &mut SourceAdmission::default())
            .map_err(err)?;
        let request = ScopeCandidateRequest {
            key: prepared.key(),
            occurrence: OccurrenceId(u64::MAX),
            prefix: "",
        };
        let actual = match names(&bound, &request, &mut budget()) {
            Ok(_) => return Err("missing occurrence accepted".into()),
            Err(error) => error,
        };
        assert_eq!(actual, CandidateError::NoOccurrence);
        let mut errors = vec![
            actual,
            CandidateError::NotReference,
            CandidateError::NoStage,
            CandidateError::Access(BindingAccessError::LimitsMismatch),
            CandidateError::Access(BindingAccessError::StaleAnalysis),
            CandidateError::Access(BindingAccessError::MissingSource),
            CandidateError::Access(BindingAccessError::Incomplete),
            CandidateError::Binding(BindingError::ProviderInvalid),
            CandidateError::Binding(BindingError::MissingNamespace),
        ];
        for reason in [
            StopReason::Cancelled,
            StopReason::WorkLimit,
            StopReason::SourceLimit,
            StopReason::AllocationLimit,
            StopReason::DepthLimit,
            StopReason::NodeLimit,
            StopReason::OutputLimit,
            StopReason::DiagnosticLimit,
            StopReason::EventLimit,
        ] {
            errors.push(CandidateError::Stopped(reason));
            errors.push(CandidateError::Access(BindingAccessError::Stopped(reason)));
            errors.push(CandidateError::Binding(BindingError::Stopped(reason)));
            errors.push(CandidateError::Binding(BindingError::Source(
                nepl3_core::source::SourceError::Stopped(reason),
            )));
        }
        for error in errors {
            let mut sender_budget = budget();
            let value =
                failure::to_value(&error, profile.registry(), &mut sender_budget).map_err(err)?;
            let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(err)?;
            let value = nepl3_wire::decode(&bytes, &mut budget()).map_err(err)?;
            let mut receiver_budget = budget();
            let decoded = failure::from_value(&value, profile.registry(), &mut receiver_budget)
                .map_err(err)?;
            assert_eq!(decoded, error);
            // A remote stopped cause is descriptive metadata, including when
            // nested under Binding/Access. Local work must remain possible.
            receiver_budget.charge(Resource::Work, 1).map_err(err)?;
            sender_budget.charge(Resource::Work, 1).map_err(err)?;
            assert_eq!(receiver_budget.usage().source_bytes, 0);
            for malformed_mode in 0..4 {
                let mut malformed = value.clone();
                let NdfValue::Variant(v) = &mut malformed else {
                    return Err("variant".into());
                };
                match malformed_mode {
                    0 => v.schema.digest = Digest::of(b"incorrect failure schema"),
                    1 => v.variant = "UnknownCandidateError".into(),
                    2 => v.fields.push(NdfValue::U64(0)),
                    _ => v.type_name = "BindingAccessError".into(),
                }
                assert!(
                    failure::from_value(&malformed, profile.registry(), &mut budget()).is_err()
                );
            }
            for mode in 0..5 {
                let mut limits = budget().limits();
                match mode {
                    0 => limits.work = 0,
                    1 => limits.nodes = 0,
                    2 => limits.allocation_units = 0,
                    3 => limits.depth = 0,
                    _ => {}
                }
                let mut b = Budget::new(limits);
                if mode == 4 {
                    b.cancel();
                }
                assert!(matches!(
                    failure::to_value(&error, profile.registry(), &mut b),
                    Err(PortableError::Stopped(reason)) if reason == [StopReason::WorkLimit, StopReason::NodeLimit, StopReason::AllocationLimit, StopReason::DepthLimit, StopReason::Cancelled][mode]
                ));
                let mut b = Budget::new(limits);
                if mode == 4 {
                    b.cancel();
                }
                let expected = [
                    StopReason::WorkLimit,
                    StopReason::NodeLimit,
                    StopReason::AllocationLimit,
                    StopReason::DepthLimit,
                    StopReason::Cancelled,
                ][mode];
                assert!(
                    matches!(failure::from_value(&value, profile.registry(), &mut b), Err(PortableError::Stopped(reason)) if reason == expected)
                );
            }
        }
        Ok(())
    })
}
