use super::{budget, load, profile};
use nepl3_core::{budget::*, source::*, value::*};
use nepl3_grammar_core::request::{self, CompileRequest, RequestError};
use nepl3_wire::foundation::FoundationCodec;
fn error(value: impl core::fmt::Debug) -> String {
    format!("{value:?}")
}
fn fixture() -> Result<CompileRequest, String> {
    Ok(CompileRequest {
        document: load("conformance/fixtures/grammar/diagnostic/valid.neplg")?,
        profile: profile::profile(),
    })
}
#[test]
fn compile_input_roundtrip_keeps_document_profile_and_canonical_identity() -> Result<(), String> {
    let registry = profile::registry()?;
    let request = fixture()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let value = request::to_value(&request, &registry, &mut codec, &mut budget()).map_err(error)?;
    let bytes = nepl3_wire::encode(&value, &mut budget()).map_err(error)?;
    let fingerprint =
        request::fingerprint(&request, &registry, &mut codec, &mut budget()).map_err(error)?;
    assert_eq!(
        fingerprint,
        Digest::domain(b"nepl3.grammar.compile-request/1\0", &bytes)
    );
    let receiver_registry = profile::registry()?;
    let receiver_store = SourceStore::default();
    let mut receiver_admission = SourceAdmission::default();
    let mut receiver =
        FoundationCodec::new(&receiver_registry, &receiver_store, &mut receiver_admission)
            .map_err(error)?;
    let raw = nepl3_wire::decode(&bytes, &mut budget()).map_err(error)?;
    let restored = request::from_value(&raw, &receiver_registry, &mut receiver, &mut budget())
        .map_err(error)?;
    assert_eq!(restored, request);
    assert_eq!(
        request::fingerprint(&restored, &receiver_registry, &mut receiver, &mut budget())
            .map_err(error)?,
        fingerprint
    );
    for case in 0..7 {
        let mut changed = request.clone();
        match case {
            0 => changed.profile.package.push('x'),
            1 => changed.profile.state_type = nepl3_core::schema::TypeDescriptor::Text,
            2 => changed.profile.reader_imports[0].provider.push('x'),
            3 => changed.profile.extensions[0].alias.push('x'),
            4 => changed.profile.classes[0].name.push('x'),
            5 => changed.profile.views[0].name.push('x'),
            _ => changed.document = load("examples/grammar/angle-tag.neplg")?,
        }
        assert_ne!(
            request::fingerprint(&changed, &registry, &mut codec, &mut budget()).map_err(error)?,
            fingerprint
        );
    }
    Ok(())
}
#[test]
fn malformed_profile_is_rejected_before_any_document_source_admission() -> Result<(), String> {
    let registry = profile::registry()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let mut value =
        request::to_value(&fixture()?, &registry, &mut codec, &mut budget()).map_err(error)?;
    let NdfValue::Record(record) = &mut value else {
        return Err("request".into());
    };
    record.fields[1] = NdfValue::U64(0);
    let mut receiver_admission = SourceAdmission::default();
    let mut receiver =
        FoundationCodec::new(&registry, &store, &mut receiver_admission).map_err(error)?;
    let mut b = budget();
    assert!(matches!(
        request::from_value(&value, &registry, &mut receiver, &mut b),
        Err(RequestError::Schema(_))
    ));
    assert_eq!(b.usage().source_bytes, 0);
    Ok(())
}
#[test]
fn request_boundary_shares_source_charges_and_preserves_stops() -> Result<(), String> {
    let registry = profile::registry()?;
    let request = fixture()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let mut b = budget();
    let value = request::to_value(&request, &registry, &mut codec, &mut b).map_err(error)?;
    let sources = b.usage().source_bytes;
    assert!(sources > 0);
    request::to_value(&request, &registry, &mut codec, &mut b).map_err(error)?;
    assert_eq!(b.usage().source_bytes, sources);
    for encode in [false, true] {
        for reason in [
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::NodeLimit,
            StopReason::DepthLimit,
            StopReason::Cancelled,
        ] {
            let mut limits = budget().limits();
            match reason {
                StopReason::SourceLimit => limits.source_bytes = 0,
                StopReason::WorkLimit => limits.work = 0,
                StopReason::AllocationLimit => limits.allocation_units = 0,
                StopReason::NodeLimit => limits.nodes = 0,
                StopReason::DepthLimit => limits.depth = 0,
                _ => {}
            }
            let mut a = SourceAdmission::default();
            let mut c = FoundationCodec::new(&registry, &store, &mut a).map_err(error)?;
            let mut b = Budget::new(limits);
            if reason == StopReason::Cancelled {
                b.cancel();
            }
            let result = if encode {
                request::to_value(&request, &registry, &mut c, &mut b).map(|_| ())
            } else {
                request::from_value(&value, &registry, &mut c, &mut b).map(|_| ())
            };
            assert!(
                matches!(result, Err(RequestError::Stopped(actual)) if actual == reason),
                "{reason:?}: {result:?}"
            );
        }
    }
    Ok(())
}

#[test]
fn compile_request_cumulative_exact_and_one_short_limits() -> Result<(), String> {
    let registry = profile::registry()?;
    let request = fixture()?;
    let store = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
    let value = request::to_value(&request, &registry, &mut codec, &mut budget()).map_err(error)?;
    for encode in [false, true] {
        let attempt = |limits: Limits| -> Result<Usage, RequestError<nepl3_wire::WireError>> {
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(&registry, &store, &mut admission)
                .map_err(RequestError::Boundary)?;
            let mut b = Budget::new(limits);
            b.charge(Resource::Work, 17)?;
            if encode {
                request::to_value(&request, &registry, &mut codec, &mut b)?;
            } else {
                request::from_value(&value, &registry, &mut codec, &mut b)?;
            }
            Ok(b.usage())
        };
        let used = attempt(budget().limits()).map_err(error)?;
        let exact = Limits {
            source_bytes: used.source_bytes,
            work: used.work,
            nodes: used.nodes,
            depth: used.depth,
            allocation_units: used.allocation_units,
            ..Limits::default()
        };
        assert_eq!(attempt(exact).map_err(error)?, used);
        for reason in [
            StopReason::SourceLimit,
            StopReason::WorkLimit,
            StopReason::AllocationLimit,
            StopReason::NodeLimit,
            StopReason::DepthLimit,
        ] {
            let mut limits = exact;
            match reason {
                StopReason::SourceLimit => {
                    assert!(limits.source_bytes > 0);
                    limits.source_bytes -= 1;
                }
                StopReason::WorkLimit => {
                    assert!(limits.work > 17);
                    limits.work -= 1;
                }
                StopReason::AllocationLimit => {
                    assert!(limits.allocation_units > 0);
                    limits.allocation_units -= 1;
                }
                StopReason::NodeLimit => {
                    assert!(limits.nodes > 0);
                    limits.nodes -= 1;
                }
                StopReason::DepthLimit => {
                    assert!(limits.depth > 0);
                    limits.depth -= 1;
                }
                _ => {}
            }
            assert!(
                matches!(attempt(limits), Err(RequestError::Stopped(actual)) if actual == reason)
            );
        }
    }
    Ok(())
}

#[test]
fn fingerprint_output_budget_preserves_foundation_stop() -> Result<(), String> {
    let registry = profile::registry()?;
    let request = fixture()?;
    let store = SourceStore::default();
    for output_bytes in [31, 32] {
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &store, &mut admission).map_err(error)?;
        let mut b = Budget::new(Limits {
            output_bytes,
            ..budget().limits()
        });
        let result = request::fingerprint(&request, &registry, &mut codec, &mut b);
        if output_bytes == 31 {
            assert!(matches!(
                result,
                Err(RequestError::Stopped(StopReason::OutputLimit))
            ));
        } else {
            result.map_err(error)?;
            assert_eq!(b.usage().output_bytes, 32);
        }
    }
    Ok(())
}
