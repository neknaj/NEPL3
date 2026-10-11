use super::*;
use nepl3_core::{
    budget::{Limits, StopReason},
    source::SourceId,
};
fn budget() -> Budget {
    crate::doc::source::budget()
}
fn host_before_catalog<'a>(
    compiled: &'a CompiledLanguage,
    source: &SourceSnapshot,
    implementation: Digest,
    b: &mut Budget,
) -> Result<NativeHost<'a>, String> {
    let r = &compiled.registry;
    let prefix = driver::reservation_prefix(source, b)?;
    b.charge(
        Resource::AllocationUnits,
        prefix.len() as u64 + 2 * core::mem::size_of::<NativeReader>() as u64,
    )
    .map_err(err)?;
    let readers = vec![
        NativeReader {
            operation: provider::operation(BuiltinReader::Name, r, b).map_err(err)?,
            read: provider::read,
        },
        NativeReader {
            operation: crate::sentence::reader::signature(r, b)
                .map_err(err)?
                .operation,
            read: crate::sentence::reader::read,
        },
    ];
    NativeHost::new(r, implementation, prefix, readers, b).map_err(err)
}
#[test]
fn catalog_copy_is_charged_before_sentence_profile_resolution() -> Result<(), String> {
    let implementation = Digest::of(b"sentence source catalog fixture");
    let compiled = crate::sentence::catalog::standard(
        implementation,
        &mut budget(),
        &mut SourceAdmission::default(),
    )?;
    let source = SourceSnapshot::new(
        SourceId("catalog-source".into()),
        0,
        "memory:catalog-source".into(),
        b"\"hello\"".to_vec(),
        &mut budget(),
    )
    .map_err(err)?;
    let mut preparation = budget();
    let _ = host_before_catalog(&compiled, &source, implementation, &mut preparation)?;
    let limits = Limits {
        allocation_units: preparation.usage().allocation_units,
        ..budget().limits()
    };
    let mut oracle = Budget::new(limits);
    let host = host_before_catalog(&compiled, &source, implementation, &mut oracle)?;
    assert!(matches!(
        host.provider_catalog(&mut oracle),
        Err(nepl3_engine::parse::ParseError::Stopped(
            StopReason::AllocationLimit
        ))
    ));
    for native in [false, true] {
        let mut b = Budget::new(limits);
        let mut finished = false;
        let result = with_tree(
            &compiled,
            &source,
            implementation,
            &mut b,
            &mut SourceAdmission::default(),
            native,
            |_, _, _, _| {
                finished = true;
                Ok(())
            },
        );
        assert!(matches!(result,Err(ref e) if e.contains("AllocationLimit")));
        assert!(!finished);
        assert_eq!(b.usage(), oracle.usage());
        assert_eq!(b.poll(), Err(StopReason::AllocationLimit));
        let mut b = budget();
        let mut completed = false;
        with_tree(
            &compiled,
            &source,
            implementation,
            &mut b,
            &mut SourceAdmission::default(),
            native,
            |_, _, _, _| {
                completed = true;
                Ok(())
            },
        )?;
        assert!(completed);
        assert!(b.usage().work > preparation.usage().work);
        assert_eq!(b.poll(), Ok(()));
    }
    Ok(())
}
