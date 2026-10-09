use super::*;
use nepl3_core::budget::{StopReason, Usage};
use nepl3_engine::selection::HeadShape;
fn err(e: impl core::fmt::Debug) -> String {
    format!("{e:?}")
}
#[test]
fn dynamic_record_kind_admission_meters_only_the_selected_catalog_prefix() -> Result<(), String> {
    let (package, original) = fixture()?;
    let target = original
        .descriptor(&package.schema)
        .ok_or("target")?
        .clone();
    let type_fee = target.types.len() as u64 + 1;
    let shape = HeadShape {
        kind: package.forms[0].kind.clone(),
        fields: vec![],
        binding: BindingId(0),
        styles: vec![],
        selection_rules: vec![],
    };
    let mut compact_work = None;
    for (count, width, suffix) in [
        (0, 0, false),
        (32, 0, false),
        (32, 64, false),
        (32, 0, true),
    ] {
        let mut registry = SchemaRegistry::default();
        let mut lookup = 1;
        for d in [
            nepl3_core::schema::foundation::descriptor(&mut budget()).map_err(err)?,
            nepl3_reader::schema::descriptor(&mut budget()).map_err(err)?,
            nepl3_engine::schema::descriptor(&mut budget()).map_err(err)?,
        ] {
            lookup += (d.package.len() + target.package.len() + 9) as u64;
            registry
                .register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
                .map_err(err)?;
        }
        if suffix {
            registry
                .register(package.schema.clone(), target.clone(), &mut budget())
                .map_err(err)?;
        }
        for i in 0..count {
            let name = format!("padding.{i:02}{}", "x".repeat(width));
            if !suffix {
                lookup += (name.len() + target.package.len() + 9) as u64;
            }
            let d = SchemaDescriptor {
                package: name,
                revision: 1,
                types: vec![],
                operations: vec![],
            };
            registry
                .register(d.reference(&mut budget()).map_err(err)?, d, &mut budget())
                .map_err(err)?;
        }
        if !suffix {
            registry
                .register(package.schema.clone(), target.clone(), &mut budget())
                .map_err(err)?;
        }
        lookup += (2 * target.package.len() + 9 + target.package.len() + 41) as u64;
        registry.finalize(&mut budget()).map_err(err)?;
        let checked = package.check(&registry, &mut budget()).map_err(err)?;
        let mut b = budget();
        assert_eq!(
            checked.validate_head_shape(&shape, &mut b),
            Err(PackageError::KindShape)
        );
        assert_eq!(
            b.usage(),
            Usage {
                work: 1 + lookup + type_fee,
                ..Usage::default()
            }
        );
        if count == 0 {
            compact_work = Some(b.usage().work);
        }
        if count > 0 && !suffix {
            let mut limits = budget().limits();
            limits.work = compact_work.ok_or("compact")?;
            let mut stopped = Budget::new(limits);
            assert_eq!(
                checked.validate_head_shape(&shape, &mut stopped),
                Err(PackageError::Stopped(StopReason::WorkLimit))
            );
            assert_eq!(stopped.poll(), Err(StopReason::WorkLimit));
            assert_eq!(stopped.usage().allocation_units, 0);
        }
        let mut invalid = shape.clone();
        invalid.kind.local_kind = u64::MAX;
        let mut b = budget();
        assert_eq!(
            checked.validate_head_shape(&invalid, &mut b),
            Err(PackageError::Schema(SchemaError::UnknownType))
        );
        assert_eq!(b.usage().work, 1 + lookup);
        let mut foreign = shape.clone();
        foreign.kind.schema.digest.0[0] ^= 1;
        let mut b = budget();
        assert_eq!(
            checked.validate_head_shape(&foreign, &mut b),
            Err(PackageError::KindShape)
        );
        assert_eq!(
            b.usage(),
            Usage {
                work: 1,
                ..Usage::default()
            }
        );
    }
    Ok(())
}
