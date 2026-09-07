use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    diagnostic::validation::ReportValidationError as R,
    facts::FactError as F,
    origin::OriginError as O,
    schema::{SchemaError as S, SchemaRegistry, TypeDescriptor, foundation},
    source::SourceError as Source,
    syntax::SyntaxError as Syntax,
    value::NdfValue,
    value_codec::FoundationCodecError,
    view::ViewError as V,
};
use nepl3_wire::WireError as W;
#[test]
fn codec_stop_extraction_preserves_every_typed_nested_cause() {
    for reason in [
        StopReason::Cancelled,
        StopReason::WorkLimit,
        StopReason::SourceLimit,
        StopReason::DepthLimit,
        StopReason::NodeLimit,
        StopReason::AllocationLimit,
        StopReason::OutputLimit,
        StopReason::DiagnosticLimit,
        StopReason::EventLimit,
    ] {
        let mut errors = vec![
            W::Stopped(reason),
            W::Schema(S::Stopped(reason)),
            W::Source(Source::Stopped(reason)),
            W::Origin(O::Stopped(reason)),
            W::Origin(O::Source(Source::Stopped(reason))),
            W::Facts(F::Stopped(reason)),
            W::Facts(F::Source(Source::Stopped(reason))),
            W::Facts(F::Schema(S::Stopped(reason))),
            W::Facts(F::Origin(O::Stopped(reason))),
            W::Facts(F::Origin(O::Source(Source::Stopped(reason)))),
            W::Report(R::Stopped(reason)),
            W::Report(R::Source(Source::Stopped(reason))),
            W::Report(R::Schema(S::Stopped(reason))),
        ];
        for view in [
            V::Stopped(reason),
            V::Source(Source::Stopped(reason)),
            V::Schema(S::Stopped(reason)),
            V::Origin(O::Stopped(reason)),
            V::Origin(O::Source(Source::Stopped(reason))),
        ] {
            errors.push(W::View(view.clone()));
            errors.push(W::Syntax(Syntax::View(view)));
        }
        for syntax in [
            Syntax::Stopped(reason),
            Syntax::Source(Source::Stopped(reason)),
            Syntax::Schema(S::Stopped(reason)),
            Syntax::Origin(O::Stopped(reason)),
            Syntax::Origin(O::Source(Source::Stopped(reason))),
        ] {
            errors.push(W::Syntax(syntax));
        }
        for error in errors {
            assert_eq!(error.stop_reason(), Some(reason), "{error:?}");
        }
    }
    for error in [
        W::Schema(S::WrongType),
        W::Facts(F::Authority),
        W::Report(R::Usage),
        W::Syntax(Syntax::Reference),
        W::View(V::Cover),
    ] {
        assert_eq!(error.stop_reason(), None);
    }
}
#[test]
fn actual_checked_codec_schema_work_stop_is_extractable() -> Result<(), String> {
    let mut setup = Budget::new(Limits {
        source_bytes: 1000,
        work: 1_000_000,
        depth: 100,
        nodes: 10000,
        allocation_units: 1_000_000,
        output_bytes: 10000,
        diagnostics: 100,
        events: 100,
    });
    let descriptor = foundation::descriptor(&mut setup).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema, descriptor, &mut setup)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut setup)
        .map_err(|e| format!("{e:?}"))?;
    let mut limits = setup.limits();
    limits.work = 0;
    let error = match nepl3_wire::encode_checked(
        &NdfValue::Unit,
        &TypeDescriptor::Unit,
        &registry,
        &mut Budget::new(limits),
    ) {
        Err(error) => error,
        Ok(_) => return Err("expected WorkLimit".into()),
    };
    assert_eq!(error.stop_reason(), Some(StopReason::WorkLimit));
    // Permit exactly intrinsic CBOR decoding, then stop at the actual schema
    // boundary. The bytes are the NDF/1 Unit [tag 0] vector (also fixed in codec.rs).
    let mut decoding = Budget::new(setup.limits());
    assert_eq!(
        nepl3_wire::decode(&[0x81, 0x00], &mut decoding).map_err(|e| format!("{e:?}"))?,
        NdfValue::Unit
    );
    limits.work = decoding.usage().work;
    let error = match nepl3_wire::decode_checked(
        &[0x81, 0x00],
        &TypeDescriptor::Unit,
        &registry,
        &mut Budget::new(limits),
    ) {
        Err(error) => error,
        Ok(_) => return Err("expected schema WorkLimit".into()),
    };
    assert!(matches!(
        error,
        W::Schema(S::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(error.stop_reason(), Some(StopReason::WorkLimit));
    Ok(())
}
