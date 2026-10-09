use super::*;
use nepl3_core::{budget::Usage, schema::SchemaError, value::SchemaRef};

fn schema(call: &mut HeadCall, site: usize) -> Result<&mut SchemaRef, String> {
    if site == 0 {
        return Ok(&mut call.head.token.kind.schema);
    }
    let HeadRequest::ChildContext { completed, .. } = &mut call.request else {
        return Err("child fixture".into());
    };
    if site == 1 {
        Ok(&mut completed.nodes[0]
            .token
            .as_mut()
            .ok_or("token")?
            .kind
            .schema)
    } else {
        Ok(&mut completed.nodes[0].schema)
    }
}

#[test]
fn projected_schema_lookup_costs_and_stop_prefix_cover_each_site() -> TestResult {
    with_profile(|profile| {
        for site in 0..3 {
            let mut prefix = None;
            let mut costs = Vec::new();
            let mut cases = Vec::new();
            for defect in 0..3 {
                let mut input = call(profile)?;
                let reference = schema(&mut input, site)?;
                match defect {
                    0 => reference.revision += 1,
                    1 => reference.digest.0[0] ^= 1,
                    _ => {
                        let last = reference.package.pop().ok_or("package")?;
                        reference.package.push(if last == 'x' { 'y' } else { 'x' });
                    }
                }
                let requested = reference.clone();
                let mut measured = budget();
                assert_eq!(
                    input.validate_projection(profile, &mut measured),
                    Err(HeadError::Schema(SchemaError::UnknownSchema))
                );
                let full = measured.usage();
                let mut lookup = budget();
                assert!(
                    profile
                        .registry()
                        .descriptor_with_budget(&requested, &mut lookup)
                        .map_err(error)?
                        .is_none()
                );
                let cost = lookup.usage().work;
                costs.push(cost);
                let before = full.work.checked_sub(cost).ok_or("lookup cost missing")?;
                // Equal-length bad references leave the admission prefix
                // unchanged. Only the selected registry scan cost can differ.
                if let Some(expected) = prefix {
                    assert_eq!(before, expected, "site {site}, defect {defect}");
                } else {
                    prefix = Some(before);
                }
                cases.push((input, full, before));
            }
            assert!(
                costs.windows(2).any(|pair| pair[0] != pair[1]),
                "must discriminate raw scan from budgeted lookup"
            );
            for (input, full, before) in cases {
                let original = input.clone();
                for entry in [0, 1] {
                    let mut limits = budget().limits();
                    limits.work = before + entry;
                    let mut stopped = Budget::new(limits);
                    assert_eq!(
                        input.validate_projection(profile, &mut stopped),
                        Err(HeadError::Stopped(StopReason::WorkLimit))
                    );
                    assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
                    assert_eq!(
                        stopped.usage(),
                        Usage {
                            work: before + entry,
                            ..full
                        }
                    );
                    assert_eq!(input, original);
                }
            }
        }
        call(profile)?
            .validate_projection(profile, &mut budget())
            .map_err(error)?;
        Ok(())
    })
}
