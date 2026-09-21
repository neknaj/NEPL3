use super::*;
use nepl3_suite::suspension::execution::ExecutionScope;

#[test]
fn suspended_parent_limits_cover_children_and_resume_for_every_resource() -> Result<(), StopReason>
{
    for (resource, reason) in [
        (Resource::SourceBytes, StopReason::SourceLimit),
        (Resource::Work, StopReason::WorkLimit),
        (Resource::Nodes, StopReason::NodeLimit),
        (Resource::AllocationUnits, StopReason::AllocationLimit),
        (Resource::OutputBytes, StopReason::OutputLimit),
        (Resource::Diagnostics, StopReason::DiagnosticLimit),
        (Resource::Events, StopReason::EventLimit),
    ] {
        let mut execution = budget();
        let outer = execution.limits();
        let requested = Limits {
            source_bytes: 10,
            work: 10,
            nodes: 10,
            allocation_units: 10,
            output_bytes: 10,
            diagnostics: 10,
            events: 10,
            depth: 4,
        };
        let root = ExecutionScope::root(&mut execution, requested)?;
        root.run(&mut execution, |b| {
            assert_eq!(b.current_depth(), 1);
            b.charge(resource, 4)
        })?;
        // Invoke has returned; the child still inherits its saved parent's cap.
        let child = root.child(outer, &mut execution)?;
        child.run(&mut execution, |b| {
            assert_eq!(b.limits(), requested);
            assert_eq!(b.current_depth(), 2);
            b.charge(resource, 3)
        })?;
        root.run(&mut execution, |b| b.charge(resource, 3))?;
        assert_eq!(
            root.run(&mut execution, |b| b.charge(resource, 1)),
            Err(reason)
        );
        assert_eq!(execution.limits(), outer);
        assert_eq!(execution.current_depth(), 0);
        assert_eq!(execution.poll(), Err(reason));
        let mut invoked = false;
        let retried: Result<(), StopReason> = child.run(&mut execution, |_| {
            invoked = true;
            Ok(())
        });
        assert_eq!(retried, Err(reason));
        assert!(!invoked);
    }
    Ok(())
}

#[test]
fn saved_depth_and_stricter_child_ceiling_are_preserved() -> Result<(), StopReason> {
    let mut execution = budget();
    let outer = execution.limits();
    let root = ExecutionScope::root(&mut execution, Limits { depth: 2, ..outer })?;
    let child = root.child(Limits { work: 5, ..outer }, &mut execution)?;
    root.run(&mut execution, |b| b.charge(Resource::Work, 2))?;
    child.run(&mut execution, |b| b.charge(Resource::Work, 3))?;
    assert_eq!(execution.usage().work, 5);
    assert_eq!(execution.usage().depth, 2);
    assert!(matches!(
        child.child(outer, &mut execution),
        Err(StopReason::DepthLimit)
    ));
    assert_eq!(execution.poll(), Err(StopReason::DepthLimit));
    assert_eq!(execution.current_depth(), 0);
    assert_eq!(execution.limits(), outer);
    Ok(())
}

#[test]
fn root_scope_accounts_existing_host_depth_and_rejects_zero_depth() -> Result<(), StopReason> {
    let mut execution = budget();
    let limits = Limits {
        depth: 4,
        ..execution.limits()
    };
    let root = execution.with_depth_at_least(3, |b| ExecutionScope::root(b, limits))?;
    assert_eq!(execution.current_depth(), 0);
    root.run(&mut execution, |b| {
        assert_eq!(b.current_depth(), 4);
        Ok::<_, StopReason>(())
    })?;
    assert_eq!(execution.usage().depth, 4);
    assert!(matches!(
        root.child(limits, &mut execution),
        Err(StopReason::DepthLimit)
    ));
    let mut zero = budget();
    let limits = Limits {
        depth: 0,
        ..zero.limits()
    };
    assert!(matches!(
        ExecutionScope::root(&mut zero, limits),
        Err(StopReason::DepthLimit)
    ));
    assert_eq!(zero.poll(), Err(StopReason::DepthLimit));
    assert_eq!(zero.current_depth(), 0);
    Ok(())
}
