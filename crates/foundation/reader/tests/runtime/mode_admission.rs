use super::*;
use nepl3_reader::{
    portable::{self, PortableError},
    tokenizer::*,
};
use nepl3_wire::foundation::FoundationCodec;

fn rules(schema: &SchemaRef, count: usize) -> ReaderPlan {
    let mut p = plan(
        schema,
        vec![ReaderExpr::Literal("x".into())],
        0,
        TypeDescriptor::Unit,
    );
    p.rules = (0..count)
        .map(|i| ReaderRule {
            name: format!("common.rule.{i:02}"),
            root: ReaderId(0),
            output: TypeDescriptor::Unit,
        })
        .collect();
    p
}
#[test]
fn mode_admission_precharges_missing_rule_on_native_and_portable_paths() -> Result<(), String> {
    let (registry, schema) = registry().map_err(|e| format!("{e:?}"))?;
    for count in [1, 32] {
        let p = rules(&schema, count);
        let checked = p
            .check(&registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let mode = ReaderMode {
            name: "m".into(),
            skip: vec![SkipRule {
                reader: TokenReader::Rule("common.rule.zz".into()),
            }],
            take: vec![],
        };
        let modes = [mode.clone()];
        for portable in [false, true] {
            let mut limits = budget().limits();
            limits.work = 1;
            let mut b = Budget::new(limits);
            if portable {
                let sources = SourceStore::default();
                let mut admission = SourceAdmission::default();
                let mut codec = FoundationCodec::new(&registry, &sources, &mut admission)
                    .map_err(|e| format!("{e:?}"))?;
                assert!(matches!(
                    portable::plan::mode_to_value(&mode, &checked, &mut codec, &mut b),
                    Err(PortableError::Stopped(StopReason::WorkLimit))
                ));
            } else {
                assert!(matches!(
                    TokenizationSession::new("modes".into(), &modes, &checked, &registry, &mut b),
                    Err(ReaderError::Stopped(StopReason::WorkLimit))
                ));
            }
            assert_eq!(b.usage().work, 1);
            assert_eq!(b.usage().allocation_units, 0);
        }
        let mut b = budget();
        assert!(matches!(
            TokenizationSession::new("modes".into(), &modes, &checked, &registry, &mut b),
            Err(ReaderError::Plan(PlanError::Reference))
        ));
        // Mode name, one reader entry, and each 14-byte common-prefix rule.
        assert_eq!(b.usage().work, 1 + 1 + count as u64 * (14 + 14 + 1));
        assert_eq!(b.usage().allocation_units, 0);
    }
    Ok(())
}

#[test]
fn mode_admission_validates_exact_token_kinds_and_sticky_stops() -> Result<(), String> {
    use nepl3_reader::builtin::BuiltinReader;
    for padding in [0, 32] {
        let (registry, schema) = registry_with_padding(padding).map_err(|e| format!("{e:?}"))?;
        let p = rules(&schema, 1);
        let checked = p
            .check(&registry, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        let mode = ReaderMode {
            name: "m".into(),
            skip: vec![],
            take: vec![TakeRule {
                reader: TokenReader::Builtin(BuiltinReader::Name),
                kind: KindRef {
                    schema: schema.clone(),
                    local_kind: 0,
                },
            }],
        };
        for mutation in 0..4 {
            let mut mode = mode.clone();
            match mutation {
                0 => mode.take[0].kind.schema.package = "missing".into(),
                1 => mode.take[0].kind.schema.revision += 1,
                2 => mode.take[0].kind.schema.digest.0[0] ^= 1,
                _ => mode.take[0].kind.local_kind = u64::MAX,
            }
            let expected = if mutation == 3 {
                SchemaError::UnknownType
            } else {
                SchemaError::UnknownSchema
            };
            let mut b = budget();
            let modes = [mode.clone()];
            assert!(
                matches!(TokenizationSession::new("modes".into(),&modes,&checked,&registry,&mut b),Err(ReaderError::Schema(e)) if e==expected)
            );
            let sources = SourceStore::default();
            let mut admission = SourceAdmission::default();
            let mut codec = FoundationCodec::new(&registry, &sources, &mut admission)
                .map_err(|e| format!("{e:?}"))?;
            let mut b = budget();
            assert!(
                matches!(portable::plan::mode_to_value(&mode,&checked,&mut codec,&mut b),Err(PortableError::Reader(ReaderError::Schema(e))) if e==expected)
            );
            assert_eq!(b.usage().allocation_units, 0);
            if mutation == 0 {
                // Mode, reader, kind and schema-lookup entries precede a miss
                // across foundation, reader, unrelated padding, and test.
                let catalog: u64 = ["nepl3.foundation", "nepl3.reader", "test"]
                    .iter()
                    .map(|name| (name.len() + 7 + 9) as u64)
                    .sum();
                let padding_work: u64 = (0..padding)
                    .map(|i| (format!("unrelated.{i}").len() + 7 + 9) as u64)
                    .sum();
                assert_eq!(b.usage().work, 4 + catalog + padding_work);
            }
        }
        let modes = [mode.clone()];
        let mut b = budget();
        let _session =
            TokenizationSession::new("modes".into(), &modes, &checked, &registry, &mut b)
                .map_err(|e| format!("{e:?}"))?;
        let sources = SourceStore::default();
        let mut admission = SourceAdmission::default();
        let mut codec = FoundationCodec::new(&registry, &sources, &mut admission)
            .map_err(|e| format!("{e:?}"))?;
        let value = portable::plan::mode_to_value(&mode, &checked, &mut codec, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            portable::plan::mode_from_value(&value, &checked, &mut codec, &mut budget())
                .map_err(|e| format!("{e:?}"))?,
            mode
        );
        for cancel in [false, true] {
            let mut limits = budget().limits();
            if !cancel {
                limits.work = 3;
            }
            let mut b = Budget::new(limits);
            if cancel {
                b.charge(Resource::Work, 7).map_err(|e| format!("{e:?}"))?;
                b.cancel();
            }
            let reason = if cancel {
                StopReason::Cancelled
            } else {
                StopReason::WorkLimit
            };
            assert!(
                matches!(portable::plan::mode_to_value(&mode,&checked,&mut codec,&mut b),Err(PortableError::Stopped(r)) if r==reason)
            );
            let before = b.usage();
            assert!(
                matches!(portable::plan::mode_to_value(&mode,&checked,&mut codec,&mut b),Err(PortableError::Stopped(r)) if r==reason)
            );
            assert_eq!(b.usage(), before);
            assert!(
                matches!(TokenizationSession::new("modes".into(),&modes,&checked,&registry,&mut b),Err(ReaderError::Stopped(r)) if r==reason)
            );
            assert_eq!(b.usage(), before);
        }
    }
    Ok(())
}

#[test]
fn mode_admission_charges_long_rule_and_duplicate_mode_comparisons() -> Result<(), String> {
    let (registry, schema) = registry().map_err(|e| format!("{e:?}"))?;
    let mut p = rules(&schema, 32);
    let prefix = "a".repeat(1024);
    for rule in &mut p.rules {
        rule.name = format!("{prefix}{}", rule.name);
    }
    let checked = p
        .check(&registry, &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mode = ReaderMode {
        name: "m".into(),
        skip: vec![SkipRule {
            reader: TokenReader::Rule(format!("{prefix}common.rule.zz")),
        }],
        take: vec![],
    };
    let mut b = budget();
    let modes = [mode];
    assert!(matches!(
        TokenizationSession::new("modes".into(), &modes, &checked, &registry, &mut b),
        Err(ReaderError::Plan(PlanError::Reference))
    ));
    assert_eq!(b.usage().work, 2 + 32 * (1038 + 1038 + 1));
    let mode = ReaderMode {
        name: prefix,
        skip: vec![],
        take: vec![],
    };
    let modes = [mode.clone(), mode];
    let mut b = budget();
    assert!(matches!(
        TokenizationSession::new("modes".into(), &modes, &checked, &registry, &mut b),
        Err(ReaderError::Context)
    ));
    assert_eq!(b.usage().work, 1024 + 1025 + 2048);
    let mut limits = budget().limits();
    limits.work = 1024 + 1025;
    let mut b = Budget::new(limits);
    assert!(matches!(
        TokenizationSession::new("modes".into(), &modes, &checked, &registry, &mut b),
        Err(ReaderError::Stopped(StopReason::WorkLimit))
    ));
    assert_eq!(b.usage().work, 1024 + 1025);
    Ok(())
}
