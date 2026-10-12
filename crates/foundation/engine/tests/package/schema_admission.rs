use super::*;
use nepl3_core::{
    budget::StopReason,
    value::OperationRef,
    view::{FallbackRole, PresentationClass},
};

fn failed(
    package: &LanguagePackage,
    registry: &SchemaRegistry,
    work: u64,
) -> Result<(PackageFailure, u64), Box<dyn std::error::Error>> {
    let mut limited = Budget::new(Limits {
        work,
        ..budget().limits()
    });
    match package.check_detailed(registry, &mut limited) {
        Err(failure) => Ok((failure, limited.usage().work)),
        Ok(_) => Err("expected rejected package".into()),
    }
}
fn lookup_cost(
    registry: &SchemaRegistry,
    schema: &nepl3_core::value::SchemaRef,
) -> Result<u64, Box<dyn std::error::Error>> {
    let mut measured = budget();
    registry
        .descriptor_with_budget(schema, &mut measured)
        .map_err(|e| format!("{e:?}"))?;
    Ok(measured.usage().work)
}

#[test]
fn package_root_and_payload_schema_admission_use_shared_meter() -> TestResult {
    for payload in [false, true] {
        let (mut package, registry) = fixture()?;
        let mut unknown = package.schema.clone();
        unknown.digest.0[0] ^= 1;
        // Package entry + each schema's own visit, then the shared exact lookup.
        let expected = if payload {
            package.payload_schemas = vec![unknown.clone()];
            1 + 1 + lookup_cost(&registry, &package.schema)? + 1 + lookup_cost(&registry, &unknown)?
        } else {
            package.schema = unknown.clone();
            1 + 1 + lookup_cost(&registry, &unknown)?
        };
        let (failure, used) = failed(&package, &registry, expected)?;
        assert_eq!(
            failure.error,
            PackageError::Schema(SchemaError::UnknownSchema)
        );
        assert_eq!(failure.subject, None);
        assert_eq!(used, expected);
        let (stopped, _) = failed(&package, &registry, expected - 1)?;
        assert_eq!(stopped.error, PackageError::Stopped(StopReason::WorkLimit));
    }
    Ok(())
}

#[test]
fn extension_and_style_schema_resolution_charge_before_missing_error() -> TestResult {
    for style in [false, true] {
        let (mut package, registry) = fixture()?;
        let mut unknown = package.schema.clone();
        unknown.digest.0[0] ^= 1;
        if style {
            package.forms[0].styles.push(StyleRule {
                selector: StyleSelector::Head,
                class: PresentationClass {
                    schema: unknown.clone(),
                    name: "".into(),
                    fallback: FallbackRole::Content,
                },
            });
        } else {
            package.extensions.push(ExtensionRequirement {
                alias: "extra".into(),
                provider: "".into(),
                signature: "fixture/v1".into(),
                operation: OperationRef {
                    schema: unknown.clone(),
                    name: "facts".into(),
                },
                input: TypeDescriptor::Text,
                output: TypeDescriptor::Unit,
                pure: true,
            });
        }
        // An empty name/provider stops immediately before the target lookup.
        let (_, prefix) = failed(&package, &registry, 10_000_000)?;
        if style {
            package.forms[0].styles[0].class.name = "head".into();
        } else {
            package.extensions[0].provider = "fixture".into();
        }
        let expected = prefix + lookup_cost(&registry, &unknown)?;
        let (failure, used) = failed(&package, &registry, expected)?;
        assert_eq!(
            failure.error,
            if style {
                PackageError::InvalidSelector
            } else {
                PackageError::MissingExtension
            }
        );
        assert_eq!(used, expected);
        let (stopped, _) = failed(&package, &registry, expected - 1)?;
        assert_eq!(stopped.error, PackageError::Stopped(StopReason::WorkLimit));
        assert_eq!(stopped.subject, failure.subject);
    }
    Ok(())
}
