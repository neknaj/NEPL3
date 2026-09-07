use nepl3_core::{
    budget::{Budget, Limits, StopReason},
    schema::{SchemaRegistry, foundation},
    source::{SourceAdmission, SourceError, SourceId, SourceSnapshot, SourceStore},
    value::{NdfValue, SchemaRef},
};
use nepl3_wire::{WireError, decode, encode, source::*};

type TestResult = Result<(), Box<dyn std::error::Error>>;
fn budget() -> Budget {
    Budget::new(Limits {
        source_bytes: 1_000_000,
        work: 100_000_000,
        depth: 1024,
        nodes: 1_000_000,
        allocation_units: 100_000_000,
        output_bytes: 10_000_000,
        diagnostics: 100,
        events: 100,
    })
}
fn setup() -> Result<(SchemaRef, SchemaRegistry), String> {
    let mut budget = budget();
    let descriptor = foundation::descriptor(&mut budget).map_err(|e| format!("{e:?}"))?;
    let schema = descriptor
        .reference(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    let mut registry = SchemaRegistry::default();
    registry
        .register(schema.clone(), descriptor, &mut budget)
        .map_err(|e| format!("{e:?}"))?;
    registry
        .finalize(&mut budget)
        .map_err(|e| format!("{e:?}"))?;
    Ok((schema, registry))
}
fn snapshot(id: &str, uri: &str, text: &str) -> Result<SourceSnapshot, String> {
    SourceSnapshot::new(
        SourceId(id.into()),
        1,
        uri.into(),
        text.as_bytes().to_vec(),
        &mut budget(),
    )
    .map_err(|e| format!("{e:?}"))
}
fn wire(bytes: &[u8], change: impl FnOnce(&mut Vec<NdfValue>)) -> Result<Vec<u8>, String> {
    let mut value = decode(bytes, &mut budget()).map_err(|e| format!("{e:?}"))?;
    let NdfValue::Record(root) = &mut value else {
        return Err("bundle record".into());
    };
    let Some(NdfValue::List(entries)) = root.fields.first_mut() else {
        return Err("source list".into());
    };
    change(entries);
    encode(&value, &mut budget()).map_err(|e| format!("{e:?}"))
}


#[test]
fn shared_clone_and_fresh_wire_storage_have_equal_data_not_equal_allocation() -> TestResult {
    let text = "\u{feff}A\r\n\u{6587}\u{1f600}".repeat(8192);
    let original = snapshot("id", "memory:shared", &text)?;
    let mut charged = budget();
    let clone = original.clone_with_budget(&mut charged).map_err(|e|format!("{e:?}"))?;
    assert_eq!(original, clone);
    assert_eq!(charged.usage().source_bytes, 0);
    assert!(charged.usage().work >= text.len() as u64);
    #[cfg(target_has_atomic="ptr")] {
        assert_eq!(original.text().as_ptr(), clone.text().as_ptr());
        assert!(charged.usage().allocation_units < 1024);
    }
    assert!(clone.has_bom());
    assert_eq!(clone.check_range(1,2),Err(SourceError::ScalarBoundary));
    let (schema, registry) = setup()?;
    let wire = encode_sources(core::slice::from_ref(&clone), &schema, &registry, &mut SourceAdmission::default(), &mut budget()).map_err(|e|format!("{e:?}"))?;
    let mut admission = SourceAdmission::default(); let mut operation = budget();
    let restored = decode_sources(&wire,&schema,&registry,&mut admission,&mut operation).map_err(|e|format!("{e:?}"))?;
    assert_eq!(restored[0],original);
    assert_ne!(restored[0].text().as_ptr(),original.text().as_ptr());
    assert_eq!(operation.usage().source_bytes,text.len() as u64);
    let restored2=decode_sources(&wire,&schema,&registry,&mut admission,&mut operation).map_err(|e|format!("{e:?}"))?;
    assert_eq!(restored2[0],original);
    assert_ne!(restored2[0].text().as_ptr(),restored[0].text().as_ptr());
    assert_eq!(operation.usage().source_bytes,text.len() as u64);
    let mut fresh=budget();
    decode_sources(&wire,&schema,&registry,&mut SourceAdmission::default(),&mut fresh).map_err(|e|format!("{e:?}"))?;
    assert_eq!(fresh.usage().source_bytes,text.len() as u64);
    drop(original); assert_eq!(clone.text(),text);
    Ok(())
}
#[test]
fn editing_does_not_mutate_shared_original_and_low_work_cannot_clone() -> TestResult {
    use nepl3_core::source::{TextEdit,Digest};
    let original=snapshot("id","memory:shared","\u{feff}A\r\n\u{6587}")?;
    let held=original.clone_with_budget(&mut budget()).map_err(|e|format!("{e:?}"))?;
    let mut store=SourceStore::default(); store.insert(original.clone()).map_err(|e|format!("{e:?}"))?;
    let edit=TextEdit{span: original.span(3,4).map_err(|e|format!("{e:?}"))?,expected_digest:Digest::of(b"A"),replacement:"\u{1f600}".into()};
    let ids=store.apply(&[edit],&mut budget(),&mut SourceAdmission::default()).map_err(|e|format!("{e:?}"))?;
    let next=store.get_ref(&ids[0]).ok_or("missing output")?;
    assert_eq!(held,original); assert_eq!(held.text(),"\u{feff}A\r\n\u{6587}");
    assert_eq!(next.text(),"\u{feff}\u{1f600}\r\n\u{6587}");
    assert_ne!(next.text().as_ptr(),held.text().as_ptr()); assert_ne!(next.id(),held.id());
    assert_eq!(next.uri(),held.uri());
    let mut limits=budget().limits();limits.work=1;let mut low=Budget::new(limits);
    assert_eq!(held.clone_with_budget(&mut low),Err(StopReason::WorkLimit));
    assert_eq!(low.usage().allocation_units,0);
    Ok(())
}
#[cfg(not(target_family="wasm"))]
#[test]
fn native_shared_snapshot_send_sync_survives_owner_drop() -> TestResult {
    fn send_sync<T: Send+Sync>(){} send_sync::<SourceSnapshot>();
    let source=snapshot("thread","memory:thread","\u{6587}\r\n")?;
    let expected=source.clone();
    let worker=std::thread::spawn(move || (0..1000).all(|_|source.clone()==source));
    assert!(worker.join().map_err(|_|"thread panicked")?);
    assert_eq!(expected.text(),"\u{6587}\r\n");Ok(())
}
