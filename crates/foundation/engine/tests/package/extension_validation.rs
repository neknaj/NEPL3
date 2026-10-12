use super::*;
use nepl3_core::{budget::StopReason, value::OperationRef};

#[test]
fn extension_operation_and_type_comparisons_share_cumulative_budget() -> TestResult {
    let (mut package, registry) = fixture()?;
    let operation = OperationRef {
        schema: package.schema.clone(),
        name: "facts".into(),
    };
    package.extensions.push(ExtensionRequirement {
        alias: "extra".into(),
        provider: "".into(),
        signature: "fixture/v1".into(),
        operation: operation.clone(),
        input: TypeDescriptor::Text,
        output: TypeDescriptor::Bool,
        pure: true,
    });
    let mut prefix = budget();
    let failure = package
        .check_detailed(&registry, &mut prefix)
        .err()
        .ok_or("expected empty provider")?;
    assert_eq!(failure.error, PackageError::EmptyName);
    assert_eq!(failure.subject, Some(PackageSubject::Extension(0)));
    package.extensions[0].provider = "fixture".into();
    let mut lookup = budget();
    let descriptor = registry
        .operation_with_budget(&operation, &mut lookup)
        .map_err(|e| format!("{e:?}"))?
        .ok_or("operation")?;
    assert_eq!(descriptor.input, TypeDescriptor::Text);
    assert_eq!(descriptor.output, TypeDescriptor::Unit);
    // The two scalar descriptor comparisons each cost one visited layer. The
    // output mismatch ends this stage before any later package processing.
    let expected = prefix.usage().work + lookup.usage().work + 2;
    for (work, error) in [
        (expected, PackageError::SignatureMismatch),
        (expected - 1, PackageError::Stopped(StopReason::WorkLimit)),
    ] {
        let mut exact = Budget::new(Limits {
            work,
            ..budget().limits()
        });
        let failure = package
            .check_detailed(&registry, &mut exact)
            .err()
            .ok_or("expected mismatch or stop")?;
        assert_eq!(failure.error, error);
        assert_eq!(failure.subject, Some(PackageSubject::Extension(0)));
        assert_eq!(exact.usage().work, work);
    }
    Ok(())
}
