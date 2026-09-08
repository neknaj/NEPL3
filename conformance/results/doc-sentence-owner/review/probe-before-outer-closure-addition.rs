use nepl3_core::{budget::*, origin::Origin, schema::SchemaRegistry, source::*, value::NdfValue, value_codec::FoundationValueCodec};
use nepl3_doc_core::{check::Category, lower::{self, DocumentLowerError}, model::*, portable::{self, PortableError}, sentence::{self, SentenceOutcome}};
use nepl3_tools::doc::source::{budget, compiled, err, with_input_route};
use nepl3_wire::foundation::FoundationCodec;

fn registry() -> Result<SchemaRegistry, String> {
    let mut r = SchemaRegistry::default();
    for d in [nepl3_core::schema::foundation::descriptor(&mut budget()), nepl3_doc_core::schema::descriptor(&mut budget())] {
        let d=d.map_err(err)?;let s=d.reference(&mut budget()).map_err(err)?;
        r.register(s,d,&mut budget()).map_err(err)?;
    }
    r.finalize(&mut budget()).map_err(err)?;Ok(r)
}
fn scan(r: &SchemaRegistry, padding: usize) -> Result<DocumentSyntax,String> {
    let text=format!("\r\n{}{}",r#""[漢/かん]{a/n}\[x\]\r\n""#," ".repeat(padding));
    let s=SourceSnapshot::new(SourceId("probe-owner".into()),9,"memory:probe-owner".into(),text.into_bytes(),&mut budget()).map_err(err)?;
    let result=sentence::read(&s,2,s.text().len() as u64,true,r,&mut budget(),&mut SourceAdmission::default()).map_err(err)?;
    let SentenceOutcome::Matched(lit)=result.outcome else{return Err("literal".into())};
    assert_eq!(lit.head.start(),2);
    assert_eq!(lit.head.end(),s.text().len() as u64-padding as u64);
    assert!(lit.value.nodes.iter().any(|n| matches!(&n.kind,DocKind::Text{text} if text=="[x]\r\n")));
    Ok(DocumentSyntax{value:lit.value,sources:vec![s],origins:lit.origins,views:vec![lit.view],source_maps:vec![]})
}
fn cbor(v:&NdfValue)->Result<NdfValue,String>{
    nepl3_wire::decode(&nepl3_wire::encode(v,&mut budget()).map_err(err)?,&mut budget()).map_err(err)
}
fn payload(doc:&DocumentSyntax,r:&SchemaRegistry)->Result<NdfValue,String>{
    let empty=SourceStore::default();let mut a=SourceAdmission::default();
    let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
    portable::sentence::to_value(doc,r,&mut c,&mut budget()).map_err(err)
}
#[test]
fn explicit_owner_fresh_receiver_identity_and_stop_matrix()->Result<(),String>{
    let r=registry()?;let doc=scan(&r,2048)?;let larger=scan(&r,8192)?;
    let value=payload(&doc,&r)?;let other=payload(&larger,&r)?;
    let size=nepl3_wire::encode(&value,&mut budget()).map_err(err)?.len();
    assert_eq!(size,nepl3_wire::encode(&other,&mut budget()).map_err(err)?.len(),"unrelated tail length must not enter payload");
    let received=cbor(&value)?;
    let empty=SourceStore::default();let mut a=SourceAdmission::default();
    let mut c=FoundationCodec::new(&r,&empty,&mut a).map_err(err)?;
    let sources=c.encode_sources(&doc.sources,&mut budget()).map_err(err)?;
    let sources=cbor(&sources)?;
    let mut first=SourceAdmission::default();let mut b=budget();
    let mut receiver=FoundationCodec::new(&r,&empty,&mut first).map_err(err)?;
    let owners=receiver.decode_sources(&sources,&mut b).map_err(err)?;
    let actual=portable::sentence::from_value(&received,&owners[0],&r,&mut receiver,&mut b).map_err(err)?;
    assert_eq!(actual,doc);
    assert_eq!(b.usage().source_bytes,doc.sources[0].text().len() as u64);
    portable::sentence::from_value(&received,&owners[0],&r,&mut receiver,&mut b).map_err(err)?;
    assert_eq!(b.usage().source_bytes,doc.sources[0].text().len() as u64);
    let mut ambient=SourceStore::default();ambient.insert(doc.sources[0].clone()).map_err(err)?;
    for (id,rev,text) in [("probe-owner",10,doc.sources[0].text()),("other-owner",9,doc.sources[0].text()),("probe-owner",9,larger.sources[0].text())] {
        let wrong=SourceSnapshot::new(SourceId(id.into()),rev,"memory:probe-owner".into(),text.as_bytes().to_vec(),&mut budget()).map_err(err)?;
        let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&ambient,&mut a).map_err(err)?;
        let mut b=budget();let result=portable::sentence::from_value(&received,&wrong,&r,&mut c,&mut b);
        assert!(result.is_err(),"wrong owner accepted");assert_eq!(b.poll(),Ok(()));
    }
    for (resource,stop) in [(0,StopReason::SourceLimit),(1,StopReason::WorkLimit),(2,StopReason::AllocationLimit),(3,StopReason::DepthLimit),(4,StopReason::NodeLimit),(5,StopReason::Cancelled)] {
        for cap in [0,1] {
            let mut limits=budget().limits();match resource {0=>limits.source_bytes=cap,1=>limits.work=cap,2=>limits.allocation_units=cap,3=>limits.depth=cap,4=>limits.nodes=cap,_=>{}}
            let mut b=Budget::new(limits);if resource==5{b.cancel()}
            let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(&r,&ambient,&mut a).map_err(err)?;
            let result=portable::sentence::from_value(&received,&doc.sources[0],&r,&mut c,&mut b);
            assert!(matches!(result,Err(PortableError::Stopped(s)) if s==stop),"resource {resource} cap {cap}: {result:?}");
            assert_eq!(b.poll(),Err(stop));assert_eq!(received,value);
        }
    }
    println!("explicit owner: same payload bytes {size} for owner lengths {} / {}; 3 identity negatives; 12 stops",doc.sources[0].text().len(),larger.sources[0].text().len());
    Ok(())
}

// Use the wider legacy document encoder to construct a schema-valid but
// semantically invalid narrow payload. Never ask its checked encoder to mint it.
fn unchecked_narrow(doc:&DocumentSyntax,template:&NdfValue,r:&SchemaRegistry)->Result<NdfValue,String>{
    let empty=SourceStore::default();let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
    let NdfValue::Record(ref old)=portable::to_value(doc,r,&mut c,&mut budget()).map_err(err)? else{return Err("legacy record".into())};
    let NdfValue::List(views)=&old.fields[3] else{return Err("views".into())};
    let mut result=template.clone();let NdfValue::Record(new)=&mut result else{return Err("narrow record".into())};
    new.fields=vec![old.fields[0].clone(),old.fields[2].clone(),views[0].clone()];Ok(result)
}

#[test]
fn actual_parse_outer_cbor_lower_and_token_spoofs()->Result<(),String>{
    let compiled=compiled()?;
    let source="paragraph cons \"[甲/こう]{a/n}\" cons \"乙\" nil\r\n";
    let mut saved=None;
    for native in [false,true] {
        with_input_route(native,&compiled,source,"Block",|tree,profile,_,_|{
            if let Some(before)=&saved{assert_eq!(tree.tree(),before)}else{saved=Some(tree.tree().clone())}
            let original=&tree.tree().bundle;let r=profile.registry();let empty=SourceStore::default();
            let tokens:Vec<_>=original.nodes.iter().filter(|n|n.kind=="Leaf:SentenceLiteral").map(|n|n.token.ok_or("token").map(|x|x.0 as usize)).collect::<Result<_,_>>()?;
            assert_eq!(tokens.len(),2);let first=tokens[0];let second=tokens[1];
            let owner=original.sources.iter().find(|s|s.identity()==original.tokens[first].head.snapshot_ref()).ok_or("owner")?;
            let lower_raw=|raw:&nepl3_core::syntax::SyntaxBundle| ->Result<DocumentSyntax,String>{
                let mut b=budget();let mut a=SourceAdmission::default();let checked=raw.validate_with_sources(r,&mut b,&mut a).map_err(err)?;
                let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
                lower::document(&checked,&compiled.doc.package.schema,Category::Block,r,&mut b,&mut c).map_err(err)
            };
            let expected=lower_raw(original)?;
            let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
            let wire=cbor(&c.encode_syntax(original,&mut budget()).map_err(err)?)?;
            let mut fresh=SourceAdmission::default();let mut b=budget();let mut receiver=FoundationCodec::new(r,&empty,&mut fresh).map_err(err)?;
            let actual=receiver.decode_syntax(&wire,&mut b).map_err(err)?;
            assert_eq!(lower_raw(&actual)?,expected);
            assert_eq!(b.usage().source_bytes,source.len() as u64);
            let doc=portable::sentence::from_value(&original.tokens[first].payload,owner,r,&mut c,&mut budget()).map_err(err)?;
            for case in 0..9 {
                println!("case {case} native={native}");
                let mut raw=original.clone();let mut changed=doc.clone();
                match case {
                    0=>raw.tokens[first].payload=raw.tokens[second].payload.clone(),
                    1=>{changed.views[0].head=owner.span(0,owner.text().len() as u64).map_err(err)?;raw.tokens[first].payload=payload(&changed,r)?;}
                    2=>{changed.views[0].view.elements.clear();changed.views[0].view.roots.clear();raw.tokens[first].payload=payload(&changed,r)?;}
                    3=>{changed.value.nodes[0].span=Some(raw.tokens[second].head.clone());raw.tokens[first].payload=unchecked_narrow(&changed,&raw.tokens[first].payload,r)?;}
                    4=>{changed.origins[0]=Origin::Direct(raw.tokens[second].head.clone());raw.tokens[first].payload=unchecked_narrow(&changed,&raw.tokens[first].payload,r)?;}
                    5=>{for node in &mut changed.value.nodes{node.span=None;node.origin=None}raw.tokens[first].payload=unchecked_narrow(&changed,&raw.tokens[first].payload,r)?;}
                    6=>{let NdfValue::Record(record)=&mut raw.tokens[first].payload else{return Err("payload".into())};record.schema.digest=Digest::of(b"wrong schema digest");}
                    7=>{let NdfValue::Record(record)=&mut raw.tokens[first].payload else{return Err("payload".into())};record.kind="DocumentSyntax".into();}
                    _=>raw.sources.clear(),
                }
                // Deliberately include the correct original sources in ambient:
                // neither lower nor first receipt may fill an omitted closure.
                let mut ambient=SourceStore::default();for s in &original.sources{ambient.insert(s.clone()).map_err(err)?;}
                let mut a=SourceAdmission::default();let mut b=budget();let mut codec=FoundationCodec::new(r,&ambient,&mut a).map_err(err)?;
                let result=match raw.validate_with_sources(r,&mut b,codec.source_admission()) {
                    Ok(checked)=>lower::document(&checked,&compiled.doc.package.schema,Category::Block,r,&mut b,&mut codec).map_err(err),
                    Err(e)=>Err(err(e)),
                };
                assert!(result.is_err(),"native {native} case {case} accepted");assert_eq!(b.poll(),Ok(()));
                // Opaque outer encoding is schema-only where possible; direct
                // payload CBOR always preserves the malformed claim for review.
                let received=cbor(&raw.tokens[first].payload)?;
                assert_eq!(received,raw.tokens[first].payload);
                if case>=3 && case<=7 {
                    let mut a=SourceAdmission::default();let mut codec=FoundationCodec::new(r,&ambient,&mut a).map_err(err)?;
                    assert!(portable::sentence::from_value(&received,owner,r,&mut codec,&mut budget()).is_err(),"payload case {case}");
                }
            }
            let checked=original.validate_with_sources(r,&mut budget(),&mut SourceAdmission::default()).map_err(err)?;
            let mut limits=budget().limits();limits.source_bytes=0;let mut b=Budget::new(limits);let mut a=SourceAdmission::default();let mut c=FoundationCodec::new(r,&empty,&mut a).map_err(err)?;
            assert!(matches!(lower::document(&checked,&compiled.doc.package.schema,Category::Block,r,&mut b,&mut c),Err(DocumentLowerError::Stopped(StopReason::SourceLimit))));
            assert_eq!(b.poll(),Err(StopReason::SourceLimit));
            println!("actual native={native}: full CBOR lower Eq; 9 malformed branches; fresh lower Source0");
            Ok(())
        })?;
    }
    Ok(())
}
