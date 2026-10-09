use super::*;
use nepl3_core::{
    budget::{Resource, StopReason},
    value::SchemaRef,
};
use nepl3_engine::profile::{ParseProfile, ProfileError, RuntimeCatalog};

fn fixture(padding: usize) -> Result<(SchemaRegistry, SchemaRef, Vec<String>), String> {
    let mut registry = SchemaRegistry::default();
    let mut names = Vec::new();
    for i in 0..padding {
        names.push(format!("padding.{i:02}.{}", "x".repeat(64)));
    }
    names.push("target".into());
    let mut target = None;
    for name in &names {
        let descriptor = SchemaDescriptor {
            package: name.clone(),
            revision: 1,
            types: vec![],
            operations: vec![],
        };
        let reference = descriptor
            .reference(&mut budget())
            .map_err(|e| format!("{e:?}"))?;
        if name == "target" {
            target = Some(reference.clone());
        }
        registry
            .register(reference, descriptor, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
    }
    registry
        .finalize(&mut budget())
        .map_err(|e| format!("{e:?}"))?;
    Ok((registry, target.ok_or("missing target")?, names))
}
fn profile(schema: SchemaRef) -> ParseProfile {
    ParseProfile {
        id: "profile.lookup".into(),
        languages: vec![],
        schemas: vec![schema],
        category_modes: vec![],
        head_providers: vec![],
        providers: vec![],
        allowlist: vec![],
        resources: vec![],
        limits: budget().limits(),
    }
}

#[test]
fn profile_schema_admission_accounts_for_registry_identity_before_closure() -> TestResult {
    let catalog = RuntimeCatalog {
        packages: &[],
        providers: &[],
        resources: &[],
    };
    let mut successful = Vec::new();
    for padding in [0, 16] {
        let (registry, target, names) = fixture(padding)?;
        let valid = profile(target.clone());
        let mut ample = budget();
        let resolved = valid
            .resolve(&catalog, &registry, &mut ample)
            .map_err(|e| format!("{e:?}"))?;
        successful.push((resolved.digest(), ample.usage()));
        for case in 0..3 {
            let mut query = target.clone();
            match case {
                0 => query.package = "absent".into(),
                1 => query.revision += 1,
                _ => query.digest.0[0] ^= 1,
            }
            let input = profile(query.clone());
            let before = input.clone();
            // One resolve entry charge, one lookup entry charge, then each
            // visited key comparison. Exact identity adds its own charge only
            // when package/revision selected an entry, including forged digests.
            let mut charges = vec![1, 1];
            charges.extend(
                names
                    .iter()
                    .map(|name| (name.len() + query.package.len() + 9) as u64),
            );
            if case == 2 {
                charges.push((query.package.len() + 41) as u64);
            }
            let required: u64 = charges.iter().sum();
            for prior in [0, 7] {
                let mut exact = Budget::new(Limits {
                    work: prior + required,
                    allocation_units: 0,
                    ..budget().limits()
                });
                exact
                    .charge(Resource::Work, prior)
                    .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    input.resolve(&catalog, &registry, &mut exact),
                    Err(ProfileError::MissingSchema)
                ));
                assert_eq!(exact.usage().work, prior + required);
                assert_eq!(exact.usage().allocation_units, 0);
                let mut accepted = prior;
                for charge in &charges {
                    let mut stopped = Budget::new(Limits {
                        work: accepted + charge - 1,
                        allocation_units: 0,
                        ..budget().limits()
                    });
                    stopped
                        .charge(Resource::Work, prior)
                        .map_err(|e| format!("{e:?}"))?;
                    assert!(matches!(
                        input.resolve(&catalog, &registry, &mut stopped),
                        Err(ProfileError::Stopped(StopReason::WorkLimit))
                    ));
                    assert_eq!(stopped.usage().work, accepted);
                    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
                    assert_eq!(stopped.usage().allocation_units, 0);
                    accepted += charge;
                }
            }
            let mut cancelled = budget();
            cancelled.cancel();
            assert!(matches!(
                input.resolve(&catalog, &registry, &mut cancelled),
                Err(ProfileError::Stopped(StopReason::Cancelled))
            ));
            assert_eq!(cancelled.usage().work, 0);
            assert_eq!(input, before);
        }
    }
    assert_eq!(successful[0].0, successful[1].0);
    let mut expected = successful[0].1;
    // Only registry padding differs; the selected profile and its digest do not.
    expected.work += 16 * (("padding.00.".len() + 64) + "target".len() + 9) as u64;
    assert_eq!(successful[1].1, expected);
    Ok(())
}
