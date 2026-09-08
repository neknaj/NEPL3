#[test]
fn review_indexed_context_permutations_errors_and_first_receiver() -> TestResult {
    let (package, registry)=extended()?;
    let profile=profile(&package,&registry)?;
    let packages=[&package];
    let resolved=profile.resolve(&RuntimeCatalog{packages:&packages,providers:&[],resources:&[]},&registry,&mut budget()).map_err(err)?;
    let original=tree(&package,&registry,&resolved)?;
    let permutations=[[0,1,2],[0,2,1],[1,0,2],[1,2,0],[2,0,1],[2,1,0]];
    for reverse_bundles in [false,true] { for reverse_host in [false,true] { for permutation in permutations {
        let mut input=original.clone();
        input.contexts[0].nodes=permutation.iter().map(|&i|original.contexts[0].nodes[i].clone()).collect();
        if reverse_host {input.contexts[1].nodes.reverse();}
        if reverse_bundles {input.contexts.reverse();}
        let before=input.clone();
        input.validate(&resolved,&mut budget(),&mut SourceAdmission::default()).map_err(err)?;
        assert_eq!(input,before);
        let empty=SourceStore::default();let mut sender=SourceAdmission::default();let mut b=budget();
        let mut codec=FoundationCodec::new(&registry,&empty,&mut sender).map_err(err)?;
        let value=portable::tree::to_value(&input,&resolved,&mut codec,&mut b).map_err(err)?;
        let bytes=nepl3_wire::encode(&value,&mut b).map_err(err)?;
        drop(codec);drop(sender);
        let receiver_store=SourceStore::default();let mut receiver=SourceAdmission::default();let mut b=budget();
        let mut codec=FoundationCodec::new(&registry,&receiver_store,&mut receiver).map_err(err)?;
        let raw=nepl3_wire::decode(&bytes,&mut b).map_err(err)?;
        let decoded=portable::tree::from_value(&raw,&resolved,&mut codec,&mut b).map_err(err)?;
        let again=portable::tree::to_value(&decoded,&resolved,&mut codec,&mut b).map_err(err)?;
        assert_eq!(bytes,nepl3_wire::encode(&again,&mut b).map_err(err)?);
        assert_eq!(decoded.contexts.len(),2);
    }}}
    // Each tuple replaces only table IDs, keeping row metadata/order; first
    // occurrence and invalid-ID behavior are compared to the old public API.
    let ids=[0,1,2,3,1u64<<32,u64::MAX];
    for (a,&x) in ids.iter().enumerate(){for (b,&y) in ids.iter().enumerate(){for (c,&z) in ids.iter().enumerate(){
        let mut input=original.clone();
        for (row,id) in input.contexts[0].nodes.iter_mut().zip([x,y,z]){row.node=NodeRef(id);}
        let before=input.clone();let mut budget=budget();
        let result=input.validate(&resolved,&mut budget,&mut SourceAdmission::default()).map(|_|()).map_err(err);
        assert_eq!(input,before);
        println!("REVIEW_ERROR {a}/{b}/{c} {result:?}");
    }}}
    for swap in [false,true] {
        let mut input=original.clone();
        let replacement=input.contexts[1].nodes[0].entry.clone();
        input.contexts[0].nodes[2].entry=replacement;
        if swap{input.contexts.reverse();}
        assert!(matches!(input.validate(&resolved,&mut budget(),&mut SourceAdmission::default()),Err(TreeError::Selection)));
    }
    Ok(())
}

fn review_chain(package:&LanguagePackage,registry:&SchemaRegistry,resolved:&ResolvedParseProfile<'_>,n:usize)->Result<ParseTree,String>{
    let mut tree=tree(package,registry,resolved)?;
    let templates=tree.contexts[0].nodes.clone();
    let FieldValue::Foreign(f)=&mut tree.bundle.nodes[1].fields[0] else{return Err("foreign".into())};
    let prototypes=f.bundle.nodes.clone();
    let text=format!("{}y","let x ".repeat(n));
    let source=SourceSnapshot::new(SourceId("chain".into()),0,"memory:chain".into(),text.as_bytes().to_vec(),&mut budget()).map_err(err)?;
    let mut nodes=Vec::new();let mut tokens=Vec::new();let mut origins=Vec::new();let mut selections=Vec::new();
    for i in 0..2*n+1 {
        let (prototype,template,start,end,spelling)=if i==2*n {(1,0,6*n,6*n+1,"y")}else if i%2==0 {(2,1,3*i,3*i+3,"let")}else{(0,2,3*i+1,3*i+2,"x")};
        let span=source.span(start as u64,end as u64).map_err(err)?;
        let mut node=prototypes[prototype].clone();
        node.head=Some(span.clone());node.cover=Some(if prototype==2{source.span(start as u64,text.len() as u64).map_err(err)?}else{span.clone()});
        node.token=Some(TokenRef(i as u64));node.origin=OriginId(i as u64);
        node.fields=if prototype==2{vec![FieldValue::Child(NodeRef(i as u64+1)),FieldValue::Child(NodeRef(i as u64+2))]}else{vec![]};
        let mut token=f.bundle.tokens[0].clone();token.head=span.clone();token.payload=NdfValue::Text(spelling.into());
        nodes.push(node);tokens.push(token);origins.push(Origin::Direct(span));
        let mut selected=templates[template].clone();selected.node=NodeRef(i as u64);selections.push(selected);
    }
    f.bundle.nodes=nodes;f.bundle.tokens=tokens;f.bundle.origins=origins;f.bundle.sources=vec![source];f.bundle.root=NodeRef(0);f.root=NodeRef(0);
    tree.contexts[0].nodes=selections;
    Ok(tree)
}

#[test]
fn review_indexed_context_work_and_stops() -> TestResult {
    let (package,registry)=extended()?;let profile=profile(&package,&registry)?;let packages=[&package];
    let resolved=profile.resolve(&RuntimeCatalog{packages:&packages,providers:&[],resources:&[]},&registry,&mut budget()).map_err(err)?;
    for n in [0,1,16,64,256]{
        let input=review_chain(&package,&registry,&resolved,n)?;
        for reverse in [false,true] {
            let mut input=input.clone();if reverse{for c in &mut input.contexts{c.nodes.reverse();}input.contexts.reverse();}
            let before=input.clone();let mut b=budget();
            input.validate(&resolved,&mut b,&mut SourceAdmission::default()).map_err(err)?;
            assert_eq!(before,input);
            println!("REVIEW_COST n={n} reverse={reverse} work={} allocation={}",b.usage().work,b.usage().allocation_units);
        }
    }
    let input=review_chain(&package,&registry,&resolved,16)?;
    let before=input.clone();let mut full=budget();input.validate(&resolved,&mut full,&mut SourceAdmission::default()).map_err(err)?;
    let used=full.usage();let mut stopped=0;
    for resource in 0..5 {for part in 0..=24u64{
        let mut limits=budget().limits();match resource{0=>limits.work=used.work*part/24,1=>limits.allocation_units=used.allocation_units*part/24,2=>limits.nodes=used.nodes*part/24,3=>limits.depth=used.depth*part/24,_=>limits.source_bytes=used.source_bytes*part/24};
        let mut b=nepl3_core::budget::Budget::new(limits);let outcome=input.validate(&resolved,&mut b,&mut SourceAdmission::default()).map(|_|());
        if outcome.is_err(){assert!(b.poll().is_err(),"{outcome:?}");stopped+=1;}else{assert_eq!(part,24);}
        assert_eq!(input,before);
    }}
    let mut b=budget();b.cancel();assert!(input.validate(&resolved,&mut b,&mut SourceAdmission::default()).is_err());assert_eq!(input,before);
    assert!(stopped>0);println!("REVIEW_STOPS {stopped}");
    Ok(())
}
