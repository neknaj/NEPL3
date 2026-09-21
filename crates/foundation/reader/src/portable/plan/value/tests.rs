use super::*;
use alloc::vec;
use nepl3_core::{
    budget::Limits,
    source::{SourceAdmission, SourceStore},
};
use nepl3_wire::foundation::FoundationCodec;

fn budget() -> Budget {
    Budget::new(Limits {
        work: 10_000_000,
        allocation_units: 10_000_000,
        depth: 1000,
        nodes: 100_000,
        ..Limits::default()
    })
}
fn error(e: impl core::fmt::Debug) -> String {
    alloc::format!("{e:?}")
}

#[test]
fn every_expression_and_character_class_preserves_schema_fields() -> Result<(), String> {
    let mut registry = SchemaRegistry::default();
    for descriptor in [
        nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(error)?,
        crate::schema::descriptor(&mut budget()).map_err(error)?,
    ] {
        let schema = descriptor.reference(&mut budget()).map_err(error)?;
        registry
            .register(schema, descriptor, &mut budget())
            .map_err(error)?;
    }
    registry.finalize(&mut budget()).map_err(error)?;
    let context = Context::new::<nepl3_wire::WireError>(&registry).map_err(error)?;
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut codec = FoundationCodec::new(&registry, &sources, &mut admission).map_err(error)?;
    let provider = OperationRef {
        schema: context.reader.clone(),
        name: "provider".into(),
    };
    let expressions = vec![
        (ReaderExpr::Literal("世界\\\"\n".into()), "Literal", 1),
        (ReaderExpr::Scalar(CharClass::Digit), "Scalar", 1),
        (ReaderExpr::Seq(vec![ReaderId(4), ReaderId(2)]), "Seq", 1),
        (
            ReaderExpr::Choice(vec![ReaderId(3), ReaderId(1)]),
            "Choice",
            1,
        ),
        (ReaderExpr::Many(ReaderId(7)), "Many", 1),
        (ReaderExpr::Some(ReaderId(7)), "Some", 1),
        (ReaderExpr::Optional(ReaderId(7)), "Optional", 1),
        (
            ReaderExpr::Repeat {
                min: 2,
                max: 5,
                body: ReaderId(7),
            },
            "Repeat",
            3,
        ),
        (ReaderExpr::Look(ReaderId(7)), "Look", 1),
        (ReaderExpr::Not(ReaderId(7)), "Not", 1),
        (ReaderExpr::Commit(ReaderId(7)), "Commit", 1),
        (
            ReaderExpr::Capture {
                name: "capture".into(),
                body: ReaderId(7),
            },
            "Capture",
            2,
        ),
        (
            ReaderExpr::Region {
                class: PresentationClass {
                    schema: context.reader.clone(),
                    name: "class".into(),
                    fallback: FallbackRole::Content,
                },
                body: ReaderId(7),
            },
            "Region",
            2,
        ),
        (
            ReaderExpr::Node {
                kind: KindRef {
                    schema: context.reader.clone(),
                    local_kind: 3,
                },
                body: ReaderId(7),
            },
            "Node",
            2,
        ),
        (ReaderExpr::Discard(ReaderId(7)), "Discard", 1),
        (ReaderExpr::Ref("rule".into()), "Ref", 1),
        (
            ReaderExpr::Decode {
                provider: provider.clone(),
                body: ReaderId(7),
            },
            "Decode",
            2,
        ),
        (
            ReaderExpr::Map {
                provider: provider.clone(),
                body: ReaderId(7),
            },
            "Map",
            2,
        ),
        (
            ReaderExpr::Then {
                first: ReaderId(7),
                provider: provider.clone(),
            },
            "Then",
            2,
        ),
        (ReaderExpr::Call(provider.clone()), "Call", 1),
        (ReaderExpr::Eof, "Eof", 0),
        (ReaderExpr::TakeCount(19), "TakeCount", 1),
        (ReaderExpr::Until("終".into()), "Until", 1),
    ];
    for (expression, name, count) in expressions {
        let value = expression
            .encode(&context, &mut codec, &mut budget())
            .map_err(error)?;
        registry
            .validate(
                &TypeDescriptor::Named(TypeRef {
                    package: "nepl3.reader".into(),
                    revision: 1,
                    name: "ReaderExpr".into(),
                }),
                &value,
                &mut budget(),
            )
            .map_err(error)?;
        let NdfValue::Variant(v) = &value else {
            return Err("variant".into());
        };
        assert_eq!(v.variant, name);
        assert_eq!(v.fields.len(), count);
        if name == "Repeat" {
            assert_eq!(v.fields[0], NdfValue::U64(2));
            assert_eq!(v.fields[1], NdfValue::U64(5));
        }
        if name == "Then" {
            let NdfValue::Record(first) = &v.fields[0] else {
                return Err("ReaderId".into());
            };
            assert_eq!(first.fields, vec![NdfValue::U64(7)]);
        }
        assert_eq!(
            ReaderExpr::decode(&value, &context, &mut codec, &mut budget()).map_err(error)?,
            expression
        );
    }
    for class in [
        CharClass::Any,
        CharClass::Whitespace,
        CharClass::IdentifierStart,
        CharClass::IdentifierContinue,
        CharClass::Digit,
        CharClass::AsciiLetter,
        CharClass::Chars("あ𠮷".into()),
        CharClass::Except("\n".into()),
        CharClass::Range {
            lo: 'あ', hi: '𠮷'
        },
    ] {
        let value = class
            .encode(&context, &mut codec, &mut budget())
            .map_err(error)?;
        assert_eq!(
            CharClass::decode(&value, &context, &mut codec, &mut budget()).map_err(error)?,
            class
        );
    }
    for kind in [
        ProviderKind::Read,
        ProviderKind::Transform,
        ProviderKind::Dependent,
    ] {
        let signature = ProviderSignature {
            operation: provider.clone(),
            kind,
            value_input: TypeDescriptor::Text,
            value_output: TypeDescriptor::U64,
            state_type: TypeDescriptor::Bool,
            continuation_type: TypeDescriptor::Bytes,
            pure: true,
        };
        let value = signature
            .encode(&context, &mut codec, &mut budget())
            .map_err(error)?;
        let NdfValue::Record(record) = &value else {
            return Err("signature".into());
        };
        assert_eq!(record.fields.len(), 7);
        for (index, name) in [(2, "Text"), (3, "U64"), (4, "Bool"), (5, "Bytes")] {
            let NdfValue::Variant(field) = &record.fields[index] else {
                return Err("type".into());
            };
            assert_eq!(field.variant, name);
        }
        assert_eq!(record.fields[6], NdfValue::Bool(true));
        assert_eq!(
            ProviderSignature::decode(&value, &context, &mut codec, &mut budget())
                .map_err(error)?,
            signature
        );
    }
    for (lo, hi) in [("", "a"), ("ab", "z"), ("z", "a")] {
        let value = variant::<nepl3_wire::WireError, 2>(
            context.reader,
            "CharClass",
            "Range",
            [NdfValue::Text(lo.into()), NdfValue::Text(hi.into())],
            &mut budget(),
        )
        .map_err(error)?;
        assert!(CharClass::decode(&value, &context, &mut codec, &mut budget()).is_err());
    }
    Ok(())
}
