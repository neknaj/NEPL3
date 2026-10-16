use super::*;
use nepl3_core::{budget::Limits, schema};
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: u64::MAX,
        work: u64::MAX,
        depth: u64::MAX,
        nodes: u64::MAX,
        allocation_units: u64::MAX,
        output_bytes: u64::MAX,
        diagnostics: u64::MAX,
        events: u64::MAX,
    })
}
fn fixture() -> Result<(SchemaRef, SchemaRef, Vec<SourceSnapshot>), WireError> {
    let f = schema::foundation::descriptor(&mut budget())?.reference(&mut budget())?;
    let a = schema::archive::descriptor(&mut budget())?.reference(&mut budget())?;
    let mut sources = Vec::new();
    for id in ["a", "b"] {
        sources.push(SourceSnapshot::new(
            SourceId(id.into()),
            0,
            alloc::format!("memory:{id}"),
            b"shared text".to_vec(),
            &mut budget(),
        )?);
    }
    Ok((a, f, sources))
}
/// Independent SyntaxArchive collection oracle, not implemented through Capture.
/// Both paths use borrowed sorting and explicitly materialize membership references.
/// This checks callback factoring parity, not pre-borrowing allocation totals.
fn legacy_capture(
    groups: &[&[SourceSnapshot]],
    a: &SchemaRef,
    f: &SchemaRef,
    admission: &mut SourceAdmission,
    b: &mut Budget,
) -> Result<NdfValue, WireError> {
    let mut union = SourceStore::default();
    let mut memberships = Vec::new();
    for sources in groups {
        let ordered = ordered_sources(sources, b)?;
        let mut members = Vec::new();
        for (reference, source) in ordered {
            admission.admit_existing(source, b)?;
            union.insert_ref_with_budget(source, b)?;
            b.charge(
                Resource::AllocationUnits,
                core::mem::size_of::<SourceRef>() as u64 + reference.source.0.len() as u64,
            )?;
            members.push(source.reference());
        }
        b.charge(
            Resource::AllocationUnits,
            core::mem::size_of::<Vec<SourceRef>>() as u64,
        )?;
        memberships.push(members);
    }
    let ordered = ordered_sources(union.snapshots(), b)?;
    let mut sources = Vec::new();
    for (_, source) in ordered {
        sources.push(source.clone_with_budget(b)?);
    }
    to_value(&sources, &memberships, a, f, admission, b)
}
#[test]
fn capture_matches_independent_collection_value_and_all_charges() -> Result<(), WireError> {
    let (a, f, sources) = fixture()?;
    let groups = [&sources[0..0], &sources[0..1], &sources[1..2], &sources[..]];
    let mut old = budget();
    let expected = legacy_capture(&groups, &a, &f, &mut SourceAdmission::default(), &mut old)?;
    let mut new = budget();
    let mut admission = SourceAdmission::default();
    let mut capture = Capture::default();
    for sources in groups {
        assert_eq!(
            capture.take(sources, &mut admission, &mut new)?,
            NdfValue::List(Vec::new())
        );
    }
    let actual = capture.finish(&a, &f, &mut admission, &mut new)?;
    assert_eq!(actual, expected);
    assert_eq!(new.usage(), old.usage());
    Ok(())
}
#[test]
fn restore_preserves_pre_factoring_sets_and_all_charges() -> Result<(), WireError> {
    let (a, f, sources) = fixture()?;
    let groups = [&sources[0..0], &sources[..], &sources[0..1]];
    let value = legacy_capture(
        &groups,
        &a,
        &f,
        &mut SourceAdmission::default(),
        &mut budget(),
    )?;
    let empty = NdfValue::List(Vec::new());
    let mut old = budget();
    let mut old_admission = SourceAdmission::default();
    let mut members = from_value(&value, &a, &f, &mut old_admission, &mut old)?.into_iter();
    let mut expected = Vec::new();
    for _ in groups {
        old.charge(Resource::Work, 1)?;
        assert!(list(&empty)?.is_empty());
        let snapshots = members.next().ok_or(WireError::InvalidLength)?;
        let mut local = SourceStore::default();
        for source in &snapshots {
            local.insert_ref_with_budget(source, &mut old)?;
        }
        expected.push(snapshots);
    }
    assert!(members.next().is_none());
    let mut new = budget();
    let mut restore = Restore::new(&value, &a, &f, &mut SourceAdmission::default(), &mut new)?;
    let mut actual = Vec::new();
    for _ in groups {
        actual.push(restore.take(&empty, &mut new)?.0);
    }
    restore.finish(&mut new)?;
    assert_eq!(actual, expected);
    assert_eq!(new.usage(), old.usage());
    Ok(())
}
