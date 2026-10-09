//! Actual standalone builtinName execution; not a host ReaderSession resume.
use super::*;
use nepl3_core::syntax::{Environment, EnvironmentEntry};
use nepl3_reader::{
    builtin::{BuiltinReader, provider},
    model::{OwnedReadRequest, ReaderContext},
    portable::{self, read::sender},
};
use nepl3_wire::{environment::environment_digest, foundation::FoundationCodec};

#[derive(Clone, Copy)]
pub enum Case {
    Matched,
    NoMatch,
    NeedMore,
    Stopped,
}
impl Case {
    fn argument(self) -> &'static str {
        match self {
            Self::Matched => "--reader-name-match",
            Self::NoMatch => "--reader-name-no-match",
            Self::NeedMore => "--reader-name-need-more",
            Self::Stopped => "--reader-name-stop",
        }
    }
}
pub fn child_mode() -> Option<Case> {
    [Case::Matched, Case::NoMatch, Case::NeedMore, Case::Stopped]
        .into_iter()
        .find(|c| std::env::args().any(|a| a == c.argument()))
}
fn setup(case: Case) -> Result<(SchemaRegistry, SourceStore, Invoke), String> {
    let (mut r, mut call) = fixture()?;
    let d = nepl3_reader::schema::descriptor(&mut budget()).map_err(error)?;
    let reference = d.reference(&mut budget()).map_err(error)?;
    r.register(reference.clone(), d, &mut budget())
        .map_err(error)?;
    r.finalize(&mut budget()).map_err(error)?;
    let (text, final_input) = match case {
        Case::Matched | Case::Stopped => ("変数 ", true),
        Case::NoMatch => ("9", true),
        Case::NeedMore => ("変数", false),
    };
    let source = SourceSnapshot::new(
        SourceId("reader-input".into()),
        1,
        "memory:reader-input".into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(error)?;
    let env = Environment {
        bindings: vec![],
        resources: vec![],
    };
    let foundation = r.selected("nepl3.foundation", 1).ok_or("foundation")?;
    let digest = environment_digest(&env, foundation, &r, &mut budget()).map_err(error)?;
    let request = OwnedReadRequest {
        snapshot: source.reference(),
        sources: vec![source.clone()],
        start: 0,
        limit: text.len() as u64,
        final_input,
        context: ReaderContext {
            schema: reference.clone(),
            category: "fixture".into(),
            mode: "default".into(),
            environment: EnvironmentEntry {
                id: 0,
                digest,
                value: env,
            },
            origins: vec![],
        },
        state: NdfValue::Unit,
    };
    let mut sources = SourceStore::default();
    sources.insert(source.clone()).map_err(error)?;
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(&r, &sources, &mut a).map_err(error)?;
    let input =
        portable::request_to_value(&request, &reference, &mut c, &sources, &r, &mut budget())
            .map_err(error)?;
    let NdfValue::Record(record) = &input else {
        return Err("Reader input".into());
    };
    call.operation = provider::operation(BuiltinReader::Name, &r, &mut budget()).map_err(error)?;
    call.input = TypedValue::Record(record.clone());
    call.sources = vec![source];
    Ok((r, sources, call))
}
fn execute(call: &Invoke, r: &SchemaRegistry, s: &SourceStore) -> Result<OperationReply, String> {
    let mut a = SourceAdmission::default();
    let mut c = FoundationCodec::new(r, s, &mut a).map_err(error)?;
    let mut execution = Budget::new(call.limits);
    let issued = sender::execute_name(&call.operation, &call.input, r, s, &mut c, &mut execution)
        .map_err(error)?;
    let usage = execution.usage();
    let out = issued
        .to_operation_reply(&mut c, &mut budget())
        .map_err(error)?;
    assert_eq!(execution.usage(), usage);
    Ok(out)
}
pub fn child(case: Case) -> Result<(), String> {
    let (r, s, expected) = setup(case)?;
    let mut c = Connection::new(io::stdin().lock(), io::stdout().lock());
    let mut a = SourceAdmission::default();
    let Some(ProviderFrame::Invoke(call)) =
        c.receive(&r, &s, &mut a, &mut budget()).map_err(error)?
    else {
        return Err("Reader Invoke".into());
    };
    let limits_ok = if matches!(case, Case::Stopped) {
        let mut allowed = expected.limits;
        allowed.work = call.limits.work;
        call.limits == allowed && call.limits.work > 0 && call.limits.work <= expected.limits.work
    } else {
        call.limits == expected.limits
    };
    if call.operation != expected.operation
        || call.input != expected.input
        || call.sources != expected.sources
        || call.request_id != expected.request_id
        || call.environment != expected.environment
        || call.resources != expected.resources
        || !limits_ok
    {
        return Err("unexpected Reader fixture request".into());
    }
    let reply = execute(&call, &r, &s)?;
    c.send(
        &ProviderFrame::Reply {
            request_id: call.request_id,
            reply,
        },
        &r,
        &s,
        &mut a,
        &mut budget(),
    )
    .map_err(error)?;
    if !matches!(
        c.receive(&r, &s, &mut a, &mut budget()).map_err(error)?,
        Some(ProviderFrame::Close)
    ) {
        return Err("Reader Close".into());
    }
    Ok(())
}
pub fn run() -> Result<(), String> {
    for case in [Case::Matched, Case::NoMatch, Case::NeedMore, Case::Stopped] {
        run_process(case.argument(), move |mut c| {
            let (r, s, mut call) = setup(case)?;
            if matches!(case, Case::Stopped) {
                let complete = execute(&call, &r, &s)?;
                let OperationReply::Result(OperationResult::Complete { report, .. }) = complete
                else {
                    return Err("stop calibration".into());
                };
                call.limits.work = report.usage.work - 1;
            }
            let expected = execute(&call, &r, &s)?;
            let mut a = SourceAdmission::default();
            c.send(
                &ProviderFrame::Invoke(call.clone()),
                &r,
                &s,
                &mut a,
                &mut budget(),
            )
            .map_err(error)?;
            let mut receive = budget();
            let pending = c
                .receive_pending_reply(&r, &mut a, &mut receive)
                .map_err(error)?
                .ok_or("Reader receipt")?;
            let frame = pending.finish(&call, &s).map_err(error)?;
            assert_eq!(
                frame,
                ProviderFrame::Reply {
                    request_id: call.request_id,
                    reply: expected.clone()
                }
            );
            match (&expected, case) {
                (
                    OperationReply::Result(OperationResult::Complete {
                        value: TypedValue::Variant(v),
                        ..
                    }),
                    Case::Matched,
                ) => {
                    assert_eq!(v.variant, "Matched");
                    assert_eq!(v.fields[0], NdfValue::Text("変数".into()));
                    assert_eq!(v.fields[1], NdfValue::U64(6));
                }
                (
                    OperationReply::Result(OperationResult::Complete {
                        value: TypedValue::Variant(v),
                        ..
                    }),
                    Case::NoMatch,
                ) => {
                    assert_eq!(v.variant, "NoMatch");
                    assert_eq!(v.fields[1], NdfValue::U64(0));
                }
                (
                    OperationReply::Result(OperationResult::Complete {
                        value: TypedValue::Variant(v),
                        ..
                    }),
                    Case::NeedMore,
                ) => assert_eq!(v.variant, "NeedMore"),
                (
                    OperationReply::Result(OperationResult::Stopped {
                        reason: StopReason::WorkLimit,
                        partial: Some(TypedValue::Variant(v)),
                        ..
                    }),
                    Case::Stopped,
                ) => {
                    assert_eq!(v.variant, "Stopped");
                }
                _ => return Err("Reader outcome mismatch".into()),
            }
            c.send(&ProviderFrame::Close, &r, &s, &mut a, &mut budget())
                .map_err(error)?;
            Ok(())
        })?;
    }
    Ok(())
}
