use super::*;
use nepl3_engine::package::{ReadSpec, ReadSpecId};
fn alternate_list(compiled: &mut CompiledLanguage) -> Result<(), String> {
    let package = &mut compiled.package;
    let mut alt = package.modes[0].clone();
    alt.name = "Alt".into();
    package.modes.push(alt);
    let form = package
        .forms
        .iter_mut()
        .find(|f| f.spelling == "sequence")
        .ok_or("sequence")?;
    let read = form.fields[0].read;
    let wrapped = ReadSpecId(package.reads.len() as u64);
    package.reads.push(ReadSpec::WithMode {
        mode: "Alt".into(),
        read,
    });
    form.fields[0].read = wrapped;
    Ok(())
}
#[test]
fn list_missing_head_and_tail_keep_distinct_resolved_modes() -> Result<(), String> {
    for (text, path, mode, tail) in [
        (
            "sequence",
            vec![ExpectedReadStep::Child { field: 0 }],
            "Alt",
            None,
        ),
        (
            "sequence cons",
            vec![
                ExpectedReadStep::Child { field: 0 },
                ExpectedReadStep::Child { field: 0 },
            ],
            "Code",
            Some(false),
        ),
        (
            "sequence cons define x x",
            vec![
                ExpectedReadStep::Child { field: 0 },
                ExpectedReadStep::Child { field: 1 },
            ],
            "Alt",
            Some(true),
        ),
    ] {
        with_configured_input(
            text,
            alternate_list,
            |_| Ok(()),
            |input, source, profile| {
                let request = ExpectedReadRequest {
                    key: input.key(),
                    source: source.reference(),
                    offset: text.len() as u64,
                };
                let result = expected_read(
                    input,
                    &request,
                    &mut budget(),
                    &mut SourceAdmission::default(),
                );
                let ExpectedReadOutcome::Complete(Some(value)) = result.outcome else {
                    return Err(err(result));
                };
                assert_eq!(value.path, path);
                assert_eq!(value.expected.mode, mode);
                let ExpectedReadOrigin::Field {
                    declared,
                    resolved_read,
                    ..
                } = value.origin
                else {
                    return Err("field".into());
                };
                let package = profile.language("B", &mut budget()).map_err(err)?;
                match tail {
                    None => assert!(
                        matches!(package.read(declared), Ok(ReadSpec::WithMode { mode, .. }) if mode == "Alt")
                    ),
                    Some(false) => assert!(
                        matches!(package.read(declared), Ok(ReadSpec::Local { category }) if category == "Decl")
                    ),
                    Some(true) => {
                        assert!(matches!(
                            package.read(declared),
                            Ok(ReadSpec::ListOf { .. })
                        ));
                        assert_eq!(resolved_read, Some(declared));
                    }
                }
                Ok(())
            },
        )?;
    }
    Ok(())
}
