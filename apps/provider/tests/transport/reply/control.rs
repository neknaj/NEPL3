use super::*;
use nepl3_core::operation::lifetime::{LifetimeError, RequestLifetimes, RequestPhase};
use nepl3_provider::control::ControlError;

fn active_tree(parent: &Invoke) -> Result<RequestLifetimes, String> {
    let mut lifetimes = RequestLifetimes::default();
    let context = Digest::of(b"context");
    lifetimes
        .begin_call(parent, context, None, &mut budget())
        .map_err(error)?;
    let mut child = parent.clone();
    child.request_id = 18;
    if let TypedValue::Record(record) = &mut child.input {
        record.fields[0] = NdfValue::U64(42);
    }
    lifetimes
        .begin_call(&child, context, Some(17), &mut budget())
        .map_err(error)?;
    for id in [19, 20] {
        child.request_id = id;
        lifetimes
            .begin_call(&child, context, None, &mut budget())
            .map_err(error)?;
    }
    lifetimes.finish(20, &mut budget()).map_err(error)?;
    Ok(lifetimes)
}

#[test]
fn managed_cancel_targets_subtree_and_eof_cancels_remaining_requests() -> Result<(), String> {
    let (registry, request) = fixture()?;
    let mut lifetimes = active_tree(&request)?;
    let mut c = connection(&ProviderFrame::Cancel { request_id: 17 }, &registry)?;
    let mut cancelled = vec![];
    let frame = c
        .receive_managed(
            &mut lifetimes,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut budget(),
            |id| cancelled.push(id),
        )
        .map_err(error)?;
    assert_eq!(frame, Some(ProviderFrame::Cancel { request_id: 17 }));
    assert_eq!(cancelled, vec![17, 18]);
    assert!(!c.is_closed());
    assert_eq!(
        lifetimes.phase(19, &mut budget()),
        Ok(RequestPhase::Running)
    );
    assert_eq!(
        lifetimes.phase(20, &mut budget()),
        Ok(RequestPhase::Finished)
    );
    assert!(
        c.receive_managed(
            &mut lifetimes,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut budget(),
            |id| cancelled.push(id)
        )
        .map_err(error)?
        .is_none()
    );
    assert_eq!(cancelled, vec![17, 18, 19]);
    assert!(c.is_closed());
    // Repeated closed receive does not repeat cancellation notifications.
    assert!(
        c.receive_managed(
            &mut lifetimes,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut budget(),
            |id| cancelled.push(id)
        )
        .is_err()
    );
    assert_eq!(cancelled, vec![17, 18, 19]);
    Ok(())
}

#[test]
fn control_failure_and_transport_termination_close_all_active_lifetimes() -> Result<(), String> {
    let (registry, request) = fixture()?;
    for case in 0..4 {
        let mut lifetimes = active_tree(&request)?;
        let mut c = match case {
            0 => connection(&ProviderFrame::Cancel { request_id: 99 }, &registry)?,
            1 => connection(&ProviderFrame::Cancel { request_id: 17 }, &registry)?,
            2 => Connection::new(Cursor::new(vec![0, 1]), vec![]),
            _ => connection(&ProviderFrame::Close, &registry)?,
        };
        let mut control = if case == 1 {
            Budget::new(Limits {
                work: 0,
                ..budget().limits()
            })
        } else {
            budget()
        };
        let mut cancelled = vec![];
        let result = c.receive_managed(
            &mut lifetimes,
            &registry,
            &SourceStore::default(),
            &mut SourceAdmission::default(),
            &mut budget(),
            &mut control,
            |id| cancelled.push(id),
        );
        match case {
            0 => assert!(matches!(
                result,
                Err(ControlError::Lifetime(LifetimeError::UnknownRequest))
            )),
            1 => assert!(matches!(
                result,
                Err(ControlError::Lifetime(LifetimeError::Stopped(
                    StopReason::WorkLimit
                )))
            )),
            2 => assert!(matches!(
                result,
                Err(ControlError::Transport(TransportError::Truncated))
            )),
            _ => assert_eq!(result.map_err(error)?, Some(ProviderFrame::Close)),
        }
        assert!(c.is_closed());
        assert_eq!(cancelled, vec![17, 18, 19]);
        assert!(matches!(
            lifetimes.begin_call(&request, Digest::of(b"context"), None, &mut budget()),
            Err(LifetimeError::Closed)
        ));
    }
    Ok(())
}
