use super::*;
use nepl3_core::source::Digest;
use nepl3_wire::syntax::group::compact;
fn hash(domain: &[u8], value: &NdfValue) -> Result<Digest, String> {
    let bytes = encode(value, &mut budget()).map_err(|e| format!("{e:?}"))?;
    Ok(Digest::domain(domain, &bytes))
}
fn fields_mut(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match value {
        NdfValue::Record(r) => Ok(&mut r.fields),
        _ => Err("record".into()),
    }
}
fn as_list(value: &NdfValue) -> Result<&Vec<NdfValue>, String> {
    match value {
        NdfValue::List(v) => Ok(v),
        _ => Err("list".into()),
    }
}
fn as_list_mut(value: &mut NdfValue) -> Result<&mut Vec<NdfValue>, String> {
    match value {
        NdfValue::List(v) => Ok(v),
        _ => Err("list".into()),
    }
}
fn bytes(d: Digest) -> NdfValue {
    NdfValue::Bytes(d.0.to_vec())
}
fn take_sources(value: &mut NdfValue) -> Result<NdfValue, String> {
    let mut out = Vec::new();
    for source in as_list(value)? {
        out.push(bytes(hash(b"NEPL3.Foreign.SourceContent.v2\0", source)?));
    }
    *value = NdfValue::List(vec![]);
    Ok(NdfValue::List(out))
}
// Independent oracle starts from checked expanded v1 values. It only traverses
// the fixed typed node-field Foreign path, never arbitrary/opaque record names.
fn syntax_sets(value: &mut NdfValue, sets: &mut Vec<NdfValue>) -> Result<(), String> {
    let bundle = &mut fields_mut(value)?[3];
    let fields = fields_mut(bundle)?;
    for node in as_list_mut(&mut fields[1])? {
        for field in as_list_mut(&mut fields_mut(node)?[2])? {
            if let NdfValue::Variant(v) = field
                && v.variant == "Foreign"
            {
                syntax_sets(&mut v.fields[0], sets)?;
            }
        }
    }
    sets.push(take_sources(&mut fields[0])?);
    Ok(())
}
fn oracle(
    group: &ForeignClosureGroup,
    f: &SchemaRef,
    r: &SchemaRegistry,
) -> Result<Vec<Digest>, String> {
    let encoded = legacy::encode(group, f, r, &mut SourceAdmission::default(), &mut budget())
        .map_err(|e| format!("{e:?}"))?;
    let mut value = decode(&encoded, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let fields = fields_mut(&mut value)?;
    let mut pd = Vec::new();
    for p in as_list_mut(&mut fields[0])? {
        let set = take_sources(&mut fields_mut(p)?[1])?;
        pd.push(hash(
            b"NEPL3.Foreign.Provenance.v2\0",
            &NdfValue::List(vec![p.clone(), NdfValue::List(vec![set])]),
        )?);
    }
    let mut ed = Vec::new();
    for e in as_list_mut(&mut fields[1])? {
        ed.push(hash(b"NEPL3.Foreign.Environment.v2\0", &fields_mut(e)?[1])?);
    }
    let mut out = Vec::new();
    for (entry, x) in group.entries.iter().zip(as_list_mut(&mut fields[2])?) {
        let syntax = &mut fields_mut(x)?[1];
        let mut sets = Vec::new();
        syntax_sets(syntax, &mut sets)?;
        let sd = hash(
            b"NEPL3.Foreign.Syntax.v2\0",
            &NdfValue::List(vec![syntax.clone(), NdfValue::List(sets)]),
        )?;
        let owner = entry.owner.0 as usize;
        out.push(hash(
            b"NEPL3.Foreign.Entry.v2\0",
            &NdfValue::List(vec![
                bytes(pd[group.environments[owner].provenance.0 as usize]),
                bytes(ed[owner]),
                bytes(sd),
            ]),
        )?);
    }
    Ok(out)
}
#[test]
fn compact_matches_independent_expanded_oracle_and_unchanged_transport() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let (transport, ids) = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(ids, oracle(&g, &f, &r)?);
    assert_eq!(
        transport,
        codec::encode(
            &g,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?
    );
    let restored = codec::decode(
        &transport,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let (_, again) = compact::encode(
        &restored,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(ids, again);
    Ok(())
}
#[test]
fn compact_subset_reorder_duplicate_and_unrelated_owner_are_stable() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let (_, ids) = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let proof = g
        .validate(&r, &mut budget(), &mut SourceAdmission::default())
        .map_err(|e| format!("{e:?}"))?;
    for selections in [vec![1], vec![1, 0], vec![0, 0]] {
        let selected = proof
            .select(
                &selections,
                &r,
                &mut budget(),
                &mut SourceAdmission::default(),
            )
            .map_err(|e| format!("{e:?}"))?;
        let (_, actual) = compact::encode(
            &selected,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(
            actual,
            selections
                .iter()
                .map(|i| ids[*i as usize])
                .collect::<Vec<_>>()
        );
    }
    let mut changed = g.clone();
    changed.provenances[1].sources.push(
        SourceSnapshot::new(
            SourceId("unrelated".into()),
            1,
            "memory:unrelated".into(),
            b"outside selected closure".to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?,
    );
    let (_, actual) = compact::encode(
        &changed,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(ids[0], actual[0]);
    assert_ne!(ids[1], actual[1]);
    Ok(())
}
#[test]
fn compact_commits_unused_local_sources_and_opaque_payloads() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let (_, ids) = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    let mut changed = g.clone();
    changed.entries[0].syntax.bundle.sources.push(
        SourceSnapshot::new(
            SourceId("unused".into()),
            1,
            "memory:unused".into(),
            b"unused text".to_vec(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?,
    );
    let (_, actual) = compact::encode(
        &changed,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_ne!(ids[0], actual[0]);
    assert_eq!(ids[1], actual[1]);
    assert_eq!(actual, oracle(&changed, &f, &r)?);
    let payload = nepl3_wire::source::encode_sources(
        &changed.entries[0].syntax.bundle.sources,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    changed.entries[0].syntax.bundle.tokens[0].payload =
        decode(&payload, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let (_, opaque) = compact::encode(
        &changed,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_ne!(actual[0], opaque[0]);
    assert_eq!(opaque, oracle(&changed, &f, &r)?);
    Ok(())
}
#[test]
fn compact_exact_resource_limits_and_sticky_failure() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let mut measured = budget();
    let expected = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut measured,
    )
    .map_err(|e| format!("{e:?}"))?;
    let u = measured.usage();
    let limits = Limits {
        source_bytes: u.source_bytes,
        work: u.work,
        depth: u.depth,
        nodes: u.nodes,
        allocation_units: u.allocation_units,
        output_bytes: u.output_bytes,
        diagnostics: u.diagnostics,
        events: u.events,
    };
    assert_eq!(
        compact::encode(
            &g,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut Budget::new(limits)
        )
        .map_err(|e| format!("{e:?}"))?,
        expected
    );
    for resource in 0..6 {
        let mut short = limits;
        let reason = match resource {
            0 => {
                short.source_bytes -= 1;
                StopReason::SourceLimit
            }
            1 => {
                short.work -= 1;
                StopReason::WorkLimit
            }
            2 => {
                short.depth -= 1;
                StopReason::DepthLimit
            }
            3 => {
                short.nodes -= 1;
                StopReason::NodeLimit
            }
            4 => {
                short.allocation_units -= 1;
                StopReason::AllocationLimit
            }
            _ => {
                short.output_bytes -= 1;
                StopReason::OutputLimit
            }
        };
        let mut b = Budget::new(short);
        let mut admission = SourceAdmission::default();
        assert_eq!(
            compact::encode(&g, &a, &s, &f, &r, &mut admission, &mut b).err(),
            Some(WireError::Stopped(reason))
        );
        let retained = b.usage();
        assert_eq!(
            compact::encode(&g, &a, &s, &f, &r, &mut admission, &mut b).err(),
            Some(WireError::Stopped(reason))
        );
        assert_eq!(b.usage(), retained);
    }
    Ok(())
}

#[test]
fn compact_source_content_changes_and_independent_allocations() -> TestResult {
    let (a, s, f, r, g) = setup()?;
    let mut identities = Vec::new();
    for (id, revision, uri, text) in [
        ("unused", 1, "memory:a", "text"),
        ("unused", 1, "memory:b", "text"),
        ("unused", 1, "memory:a", "changed"),
        ("unused", 2, "memory:a", "text"),
        ("other", 1, "memory:a", "text"),
    ] {
        let mut group = g.clone();
        group.entries[0].syntax.bundle.sources.push(
            SourceSnapshot::new(
                SourceId(id.into()),
                revision,
                uri.into(),
                text.as_bytes().to_vec(),
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?,
        );
        let (_, ids) = compact::encode(
            &group,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(ids, oracle(&group, &f, &r)?);
        assert!(!identities.contains(&ids[0]));
        identities.push(ids[0]);
        // Reallocate all outer declarations independently and reverse input order.
        let mut copied = Vec::new();
        for source in group.entries[0].syntax.bundle.sources.iter().rev() {
            copied.push(
                SourceSnapshot::new(
                    source.identity().source.clone(),
                    source.identity().revision,
                    source.uri().into(),
                    source.text().as_bytes().to_vec(),
                    &mut budget(),
                )
                .map_err(|e| format!("{e:?}"))?,
            );
        }
        group.entries[0].syntax.bundle.sources = copied;
        let (_, again) = compact::encode(
            &group,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget(),
        )
        .map_err(|e| format!("{e:?}"))?;
        assert_eq!(ids, again);
    }
    Ok(())
}
#[test]
fn compact_rejects_invalid_local_scope_and_cancellation() -> TestResult {
    let (a, s, f, r, mut g) = setup()?;
    let mut admission = SourceAdmission::default();
    for source in &g.entries[0].syntax.bundle.sources {
        admission
            .admit_existing(source, &mut budget())
            .map_err(|e| format!("{e:?}"))?;
    }
    g.entries[0].syntax.bundle.sources.clear();
    assert!(compact::encode(&g, &a, &s, &f, &r, &mut admission, &mut budget()).is_err());
    let mut cancelled = budget();
    cancelled.cancel();
    assert_eq!(
        compact::encode(&g, &a, &s, &f, &r, &mut admission, &mut cancelled).err(),
        Some(WireError::Stopped(StopReason::Cancelled))
    );
    Ok(())
}

#[test]
fn compact_growing_source_lists_reduce_expanded_v1_identity_work() -> TestResult {
    let (a, s, f, r, mut g) = setup()?;
    let extra = SourceSnapshot::new(
        SourceId("shared-large".into()),
        1,
        "memory:shared-large".into(),
        vec![b'x'; 65_536],
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    g.entries[0].syntax.bundle.sources.push(extra);
    let mut growing = g.entries[0].clone();
    // This reproduces growing local lists, where v1 prefix-state reuse cannot
    // cross a changed canonical list-length header. Identical lists may favor v1.
    for i in 0..32 {
        growing.syntax.bundle.sources.push(
            SourceSnapshot::new(
                SourceId(format!("progress-{i}")),
                1,
                format!("memory:progress-{i}"),
                vec![b'x'],
                &mut budget(),
            )
            .map_err(|e| format!("{e:?}"))?,
        );
        g.entries.push(growing.clone());
    }
    let sources = SourceStore::default();
    let mut admission = SourceAdmission::default();
    let mut old = budget();
    let mut codec =
        FoundationCodec::new(&r, &sources, &mut admission).map_err(|e| format!("{e:?}"))?;
    let (_, old_ids) = codec
        .encode_foreign_group_with_entry_digests(&g, &mut old)
        .map_err(|e| format!("{e:?}"))?;
    let mut new = budget();
    let (transport, ids) = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut new,
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(ids.len(), old_ids.len());
    assert_ne!(ids, old_ids);
    assert!(
        new.usage().work < old.usage().work,
        "compact {:?}, expanded {:?}",
        new.usage(),
        old.usage()
    );
    let restored = codec::decode(
        &transport,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    // Decoding canonicalizes each local source order; native Vec order is not identity.
    assert_eq!(oracle(&restored, &f, &r)?, ids);
    Ok(())
}

#[test]
fn compact_owner_table_permutation_and_empty_group() -> TestResult {
    let (a, s, f, r, mut g) = setup()?;
    let (_, ids) = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    g.provenances.swap(0, 1);
    for owner in &mut g.environments {
        owner.provenance.0 = 1 - owner.provenance.0;
    }
    g.environments.swap(0, 1);
    for entry in &mut g.entries {
        entry.owner.0 = 1 - entry.owner.0;
    }
    let (_, remapped) = compact::encode(
        &g,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert_eq!(ids, remapped);
    assert_eq!(remapped, oracle(&g, &f, &r)?);
    let empty = ForeignClosureGroup {
        provenances: vec![],
        environments: vec![],
        entries: vec![],
    };
    let (transport, ids) = compact::encode(
        &empty,
        &a,
        &s,
        &f,
        &r,
        &mut SourceAdmission::default(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))?;
    assert!(ids.is_empty());
    assert_eq!(
        transport,
        codec::encode(
            &empty,
            &a,
            &s,
            &f,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .map_err(|e| format!("{e:?}"))?
    );
    let mut wrong = f.clone();
    wrong.digest.0[0] ^= 1;
    assert!(
        compact::encode(
            &empty,
            &a,
            &s,
            &wrong,
            &r,
            &mut SourceAdmission::default(),
            &mut budget()
        )
        .is_err()
    );
    Ok(())
}

#[path = "depths.rs"]
mod depths;
