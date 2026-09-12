use nepl3_core::budget::{Budget, Limits, StopReason};
use nepl3_math_core::{
    environment::{self, EnvironmentError},
    exact::ExactValueError,
    model::*,
};
fn budget() -> Budget {
    Budget::new(Limits {
        work: 100000,
        nodes: 10000,
        allocation_units: 0,
        ..Limits::default()
    })
}
fn input(names: &[&str]) -> BindingEnvironment {
    BindingEnvironment {
        assignments: names
            .iter()
            .enumerate()
            .map(|(i, name)| MathAssignment {
                name: (*name).into(),
                value: MathExactValue::Truth { value: i % 2 == 0 },
            })
            .collect(),
    }
}
#[test]
fn exact_names_binary_lookup_and_missing_values() -> Result<(), String> {
    // No NFC or case folding: these are distinct UTF-8 keys, including empty Text.
    let value = input(&["", "A", "a", "e\u{301}", "z", "é"]);
    let checked = environment::check(&value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    assert!(core::ptr::eq(checked.environment(), &value));
    for item in &value.assignments {
        let found = checked
            .get(&item.name, &mut budget())
            .map_err(|e| format!("{e:?}"))?
            .ok_or("missing")?;
        assert!(core::ptr::eq(found, &item.value));
    }
    for missing in ["B", "f", "zz", "ê"] {
        assert_eq!(checked.get(missing, &mut budget()), Ok(None));
    }
    let large = BindingEnvironment {
        assignments: (0..1024)
            .map(|i| MathAssignment {
                name: format!("v{i:04}"),
                value: MathExactValue::Truth { value: true },
            })
            .collect(),
    };
    let checked = environment::check(&large, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut b = budget();
    assert!(
        checked
            .get("v0999", &mut b)
            .map_err(|e| format!("{e:?}"))?
            .is_some()
    );
    // Binary search needs at most 11 comparisons of <=5 bytes, each charged +1.
    assert!(b.usage().work <= 66);
    Ok(())
}
#[test]
fn reject_noncanonical_invalid_unused_and_sticky_stops() -> Result<(), String> {
    for (names, expected) in [
        (["a", "a"], EnvironmentError::DuplicateName { index: 1 }),
        (["z", "a"], EnvironmentError::UnsortedName { index: 1 }),
    ] {
        assert!(
            matches!(environment::check(&input(&names), &mut budget()), Err(e) if e == expected)
        );
    }
    let mut invalid = input(&["a", "unused"]);
    invalid.assignments[1].value = MathExactValue::Vector { values: vec![] };
    assert!(matches!(
        environment::check(&invalid, &mut budget()),
        Err(EnvironmentError::Value {
            index: 1,
            error: ExactValueError::EmptyVector
        })
    ));
    let value = input(&["a", "b", "long"]);
    let mut measured = budget();
    let checked = environment::check(&value, &mut measured).map_err(|e| format!("{e:?}"))?;
    for cap in 0..measured.usage().work {
        let mut b = Budget::new(Limits {
            work: cap,
            ..budget().limits()
        });
        assert!(matches!(
            environment::check(&value, &mut b),
            Err(EnvironmentError::Stopped(StopReason::WorkLimit))
        ));
        assert_eq!(b.poll(), Err(StopReason::WorkLimit));
    }
    let mut b = Budget::new(Limits {
        nodes: measured.usage().nodes - 1,
        ..budget().limits()
    });
    assert!(matches!(
        environment::check(&value, &mut b),
        Err(EnvironmentError::Stopped(StopReason::NodeLimit))
    ));
    let mut b = Budget::new(Limits {
        work: 0,
        ..budget().limits()
    });
    assert_eq!(checked.get("missing", &mut b), Err(StopReason::WorkLimit));
    let empty = input(&[]);
    let checked = environment::check(&empty, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let mut b = budget();
    b.cancel();
    assert_eq!(checked.get("missing", &mut b), Err(StopReason::Cancelled));
    assert!(matches!(
        environment::check(&empty, &mut b),
        Err(EnvironmentError::Stopped(StopReason::Cancelled))
    ));
    Ok(())
}
