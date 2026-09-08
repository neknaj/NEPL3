#[test]
fn review_indexed_context_schema_valid_raw_id_failures() -> TestResult {
    let (package,registry)=extended()?;let profile=profile(&package,&registry)?;let packages=[&package];
    let resolved=profile.resolve(&RuntimeCatalog{packages:&packages,providers:&[],resources:&[]},&registry,&mut budget()).map_err(err)?;
    let original=tree(&package,&registry,&resolved)?;
    let empty=SourceStore::default();let mut admission=SourceAdmission::default();let mut b=budget();
    let mut codec=FoundationCodec::new(&registry,&empty,&mut admission).map_err(err)?;
    let value=portable::tree::to_value(&original,&resolved,&mut codec,&mut b).map_err(err)?;
    fn fields(v:&mut NdfValue)->Result<&mut Vec<NdfValue>,String>{if let NdfValue::Record(r)=v{Ok(&mut r.fields)}else{Err("record".into())}}
    fn list(v:&mut NdfValue)->Result<&mut Vec<NdfValue>,String>{if let NdfValue::List(v)=v{Ok(v)}else{Err("list".into())}}
    for mode in 0..5 {
        let mut bad=value.clone();
        let contexts=list(&mut fields(&mut bad)?[3])?;
        if mode==4 {
            let replacement=fields(&mut contexts[1])?[0].clone();
            fields(&mut contexts[0])?[0]=replacement;
        } else {
            let selected=list(&mut fields(&mut contexts[0])?[1])?;
            let id=[0,2,1u64<<32,u64::MAX][mode];
            fields(&mut fields(&mut selected[1])?[0])?[0]=NdfValue::U64(id);
        }
        registry.validate(&TypeDescriptor::Named(TypeRef{package:"nepl3.engine".into(),revision:1,name:"ParseTree".into()}),&bad,&mut budget()).map_err(err)?;
        let bytes=nepl3_wire::encode(&bad,&mut budget()).map_err(err)?;
        let store=SourceStore::default();let mut admission=SourceAdmission::default();let mut b=budget();
        let mut receiver=FoundationCodec::new(&registry,&store,&mut admission).map_err(err)?;
        let raw=nepl3_wire::decode(&bytes,&mut b).map_err(err)?;
        let result=portable::tree::from_value(&raw,&resolved,&mut receiver,&mut b).map(|_|()).map_err(err);
        assert!(result.is_err() && b.poll().is_ok(),"{mode} {result:?}");
        println!("REVIEW_RAW {mode} {result:?}");
    }
    Ok(())
}
