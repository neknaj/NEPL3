use super::*;
use nepl3_core::{budget::StopReason, value::TypedValue};
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    portable::dispatch::{self, DispatchContext, ProviderInput},
};
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn dispatch_schema_gate_precedes_source_encoding_and_boxed_decode() -> Result<(), String> {
    let (schema, r, sources, request) = fixture()?;
    let signature = provider::signature(BuiltinReader::Name, &r, &mut budget()).map_err(err)?;
    let ctx = DispatchContext {
        signature: &signature,
        sources: &sources,
        mappings: &[],
        registry: &r,
    };
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &sources, &mut a).map_err(err)?;
    let NdfValue::Record(ref record) =
        request_to_value(&request, &schema, &mut c, &sources, &r, &mut budget()).map_err(err)?
    else {
        return Err("record".into());
    };
    let encoded = TypedValue::Record(record.clone());
    for decode in [false, true] {
        let mut prefix = budget();
        signature.check(&r, &mut prefix).map_err(err)?;
        if decode {
            encoded.clone_with_budget(&mut prefix).map_err(err)?;
        } else {
            r.validate(&signature.state_type, &request.state, &mut prefix)
                .map_err(err)?;
        }
        let mut expected = prefix.usage();
        expected.work += 1;
        let mut limits = budget().limits();
        limits.work = expected.work;
        let mut stopped = Budget::new(limits);
        let mut a = SourceAdmission::default();
        let mut c = FoundationCodec::new(&r, &sources, &mut a).map_err(err)?;
        if decode {
            assert!(matches!(
                dispatch::from_value(&encoded, &ctx, &mut c, &mut stopped),
                Err(PortableError::Stopped(StopReason::WorkLimit))
            ));
        } else {
            let mut absent = request.clone();
            absent.sources.clear();
            assert!(matches!(
                dispatch::to_value(
                    &ProviderInput::Read(Box::new(absent)),
                    &ctx,
                    &mut c,
                    &mut stopped
                ),
                Err(PortableError::Stopped(StopReason::WorkLimit))
            ));
        }
        assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
        assert_eq!(stopped.usage(), expected);
        let input = ProviderInput::Read(Box::new(request.clone()));
        let value = dispatch::to_value(&input, &ctx, &mut c, &mut budget()).map_err(err)?;
        assert_eq!(
            dispatch::from_value(&value, &ctx, &mut c, &mut budget()).map_err(err)?,
            input
        );
    }
    // Source failure is still visible when the gate has sufficient budget.
    let mut absent = request;
    absent.sources.clear();
    assert!(matches!(
        dispatch::to_value(
            &ProviderInput::Read(Box::new(absent)),
            &ctx,
            &mut c,
            &mut budget()
        ),
        Err(PortableError::UndeclaredSource)
    ));
    Ok(())
}
