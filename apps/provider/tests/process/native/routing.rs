//! Two outstanding requests share one pipe and complete in reverse order.
use super::*;

pub fn child() -> Result<(), String> {
    let (registry, mut prototype) = fixture()?;
    prototype.operation.name = "increment".into();
    let sources = granted_sources()?;
    let authority =
        Grants::new(&prototype.environment, &sources, &[], &mut budget()).map_err(error)?;
    let mut connection = Connection::new(io::stdin().lock(), io::stdout().lock());
    let mut admission = SourceAdmission::default();
    let mut calls = Vec::new();
    for _ in 0..2 {
        let Some(ProviderFrame::Invoke(call)) = connection
            .receive(&registry, &sources, &mut admission, &mut budget())
            .map_err(error)?
        else {
            return Err("expected multiplexed Invoke".into());
        };
        calls.push(call);
    }
    let registration = suspending::Registration {
        operation: &prototype.operation,
        implementation: identity(),
        invoke: &increment,
    };
    for call in calls.iter().rev() {
        let approved = authority.admit(call, &mut budget()).map_err(error)?;
        let result = connection
            .dispatch_invoke(
                &registration,
                identity(),
                &approved,
                context(call, &registry)?,
                &registry,
                &sources,
                &sources,
                &mut admission,
                &mut budget(),
                &mut budget(),
                &mut budget(),
            )
            .map_err(error)?;
        result.delivery.map_err(error)?;
    }
    match connection
        .receive(&registry, &sources, &mut admission, &mut budget())
        .map_err(error)?
    {
        Some(ProviderFrame::Close) => Ok(()),
        _ => Err("expected multiplexed Close".into()),
    }
}

pub fn run() -> Result<(), String> {
    run_process("--routing-child", |mut connection| {
        let (registry, mut first) = fixture()?;
        first.operation.name = "increment".into();
        let mut second = first.clone();
        second.request_id = 27;
        let TypedValue::Record(record) = &mut second.input else {
            return Err("expected Number input".into());
        };
        record.fields[0] = NdfValue::U64(7);
        let calls = [first, second];
        let sources = granted_sources()?;
        let contexts = [
            ReplyContext {
                request: &calls[0],
                context: context(&calls[0], &registry)?,
                authorized_sources: &sources,
            },
            ReplyContext {
                request: &calls[1],
                context: context(&calls[1], &registry)?,
                authorized_sources: &sources,
            },
        ];
        let routes = ReplyRoutes::new(&contexts, &mut budget()).map_err(error)?;
        let mut lifetimes = RequestLifetimes::default();
        let mut admission = SourceAdmission::default();
        for saved in &contexts {
            lifetimes
                .begin_call(saved.request, saved.context, None, &mut budget())
                .map_err(error)?;
            connection
                .send(
                    &ProviderFrame::Invoke(saved.request.clone()),
                    &registry,
                    &sources,
                    &mut admission,
                    &mut budget(),
                )
                .map_err(error)?;
        }
        for (expected_route, expected_value) in [(1, 8), (0, 42)] {
            let (route, reply) = connection
                .receive_active_reply(
                    &routes,
                    &mut lifetimes,
                    &registry,
                    &sources,
                    &mut admission,
                    &mut budget(),
                    &mut budget(),
                )
                .map_err(error)?;
            assert_eq!(route, expected_route);
            let OperationReply::Result(OperationResult::Complete {
                value: TypedValue::Record(record),
                ..
            }) = reply
            else {
                return Err("expected routed Number result".into());
            };
            assert_eq!(record.fields, vec![NdfValue::U64(expected_value)]);
            assert_eq!(
                lifetimes
                    .phase(calls[route].request_id, &mut budget())
                    .map_err(error)?,
                RequestPhase::Finished
            );
            if route == 1 {
                assert_eq!(
                    lifetimes
                        .phase(calls[0].request_id, &mut budget())
                        .map_err(error)?,
                    RequestPhase::Running
                );
            }
        }
        connection
            .send(
                &ProviderFrame::Close,
                &registry,
                &sources,
                &mut admission,
                &mut budget(),
            )
            .map_err(error)
    })
}
