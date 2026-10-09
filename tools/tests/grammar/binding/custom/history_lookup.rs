use super::*;
use nepl3_core::budget::StopReason;
use nepl3_engine::portable::{
    PortableError,
    binding::{
        DecodedBindingOutcome, DecodedBindingReply, decoded_to_value, reply_from_value,
        reply_to_value,
    },
};
#[test]
fn retained_provider_schema_lookup_obeys_work_admission() -> Result<(), String> {
    let compiled = compiled()?;
    with_input(&compiled, "early x z custom x x", |tree, profile, _, _| {
        let reply = analyze_with_host(
            "history-lookup",
            tree,
            profile,
            &mut query_host(true, false),
            &mut budget(),
            &mut SourceAdmission::default(),
        );
        if !matches!(reply.outcome, BindingOutcome::Complete(_)) {
            return Err(format!("{reply:?}"));
        }
        let empty = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec =
            FoundationCodec::new(profile.registry(), &empty, &mut admission).map_err(err)?;
        let value =
            reply_to_value(&reply, profile.registry(), &mut codec, &mut budget()).map_err(err)?;
        let run = |data: &DecodedBindingReply, b: &mut Budget| {
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(profile.registry(), &empty, &mut a).map_err(err)?;
            Ok::<_, String>(decoded_to_value(data, profile.registry(), &mut c, b))
        };
        for defect in 0..3 {
            let mut decoded =
                reply_from_value(&value, profile.registry(), &mut codec, &mut budget())
                    .map_err(err)?;
            let DecodedBindingOutcome::Complete(result) = &mut decoded.outcome else {
                return Err("complete history fixture".into());
            };
            assert_eq!(result.resolution_history.len(), 1);
            let provider = &mut result.resolution_history[0].provider;
            match defect {
                0 => provider.operation.schema.package.push('x'),
                1 => provider.operation.schema.revision += 1,
                _ => provider.operation.schema.digest.0[0] ^= 1,
            }
            provider.provider.clear();
            let mut control = budget();
            assert!(matches!(
                run(&decoded, &mut control)?,
                Err(PortableError::Shape)
            ));
            let before = control.usage();
            let DecodedBindingOutcome::Complete(result) = &mut decoded.outcome else {
                return Err("complete history fixture".into());
            };
            result.resolution_history[0].provider.provider.push('x');
            let original = format!("{decoded:?}");
            let mut limited = Budget::new(Limits {
                work: before.work + 1,
                ..budget().limits()
            });
            // Empty provider stops just before lookup; adding one byte spends
            // the last Work unit. Registry admission must stop before scanning.
            assert!(matches!(
                run(&decoded, &mut limited)?,
                Err(PortableError::Stopped(StopReason::WorkLimit))
            ));
            assert_eq!(limited.poll(), Err(StopReason::WorkLimit));
            assert_eq!(
                limited.usage(),
                Usage {
                    work: before.work + 1,
                    ..before
                }
            );
            assert_eq!(format!("{decoded:?}"), original);
            assert!(matches!(
                run(&decoded, &mut budget())?,
                Err(PortableError::Shape)
            ));
        }
        Ok(())
    })
}
