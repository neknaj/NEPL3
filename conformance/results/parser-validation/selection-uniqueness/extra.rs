    use nepl3_core::budget::{Resource,StopReason};
    let make=|n:usize| -> Result<ParseTree,String> {
        let text="let x ".repeat(n)+"y";
        let source=SourceSnapshot::new(SourceId("tree-\u{65e5}\u{672c}".into()),u64::MAX,"memory:tree".into(),text.as_bytes().to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?;
        let mut nodes=vec![];let mut tokens=vec![];let mut origins=vec![];let mut selected=vec![];
        for id in 0..=n*2 {
            let is_form=id<n*2 && id%2==0;let is_name=id%2==1;
            let start=if is_form {id/2*6}else if is_name {id/2*6+4}else{n*6};let end=start+if is_form{3}else{1};
            let span=source.span(start as u64,end as u64).map_err(|e|format!("{e:?}"))?;
            let fields=if is_form{vec![FieldValue::Child(NodeRef(id as u64+1)),FieldValue::Child(NodeRef(id as u64+2))]}else{vec![]};
            nodes.push(SyntaxNode{schema:package.schema.clone(),kind:if is_form{"Form:Let"}else if is_name{"Builtin:Name"}else{"Leaf:Name"}.into(),fields,head:Some(span.clone()),cover:Some(if is_form{source.span(start as u64,text.len() as u64).map_err(|e|format!("{e:?}"))?}else{span.clone()}),origin:OriginId(id as u64),token:Some(TokenRef(id as u64))});
            tokens.push(Token{kind:package.leaves[0].token_kind.clone(),head:span.clone(),payload:NdfValue::Text(text[start..end].into()),views:ViewBundle{elements:vec![],roots:vec![]},leading_trivia:vec![]});
            origins.push(Origin::Direct(span));selected.push(NodeSelection{node:NodeRef(id as u64),entry:entry.clone(),execution_digest:execution,shape:if is_form{ShapeSelection::Form{index:0}}else if is_name{ShapeSelection::Builtin{read:ReadSpecId(0)}}else{ShapeSelection::Leaf{index:0}}});
        }
        Ok(ParseTree{profile_digest:resolved.digest(),bundle:SyntaxBundle{sources:vec![source],nodes,origins,root:NodeRef(0),environments:vec![],tokens,source_maps:vec![]},recovery:vec![],contexts:vec![BundleContext{path:vec![],nodes:selected}]})
    };
    let validate=|tree:&ParseTree,b:&mut Budget|tree.validate(&resolved,b,&mut SourceAdmission::default()).map(|_|());
    for n in [0,1,4,16,64,128] {
        let original=make(n)?;
        for order in 0..3 {
            let mut tree=original.clone();let nodes=&mut tree.contexts[0].nodes;
            if order==1{nodes.reverse();}if order==2 {let len=nodes.len();for i in 0..len{nodes.swap(i,(i*19+7)%len);}}
            let saved=tree.clone();let mut b=budget();validate(&tree,&mut b).map_err(|e|format!("n{n} order{order}: {e:?}"))?;assert_eq!(tree,saved);
            println!("COST n={} order={} {:?}",tree.bundle.nodes.len(),order,b.usage());
        }
    }
    let tree=make(1)?;
    let mut bad=tree.clone();bad.bundle.root=NodeRef(2);bad.contexts[0].nodes.pop();assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Unreachable));
    let mut bad=tree.clone();bad.bundle.nodes.clear();assert!(matches!(validate(&bad,&mut budget()),Err(TreeError::Syntax(_))));
    let mut bad=tree.clone();bad.contexts.clear();assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Selection));
    for a in [0,1,2,3,u64::MAX] {for b in [0,1,2,3,u64::MAX] {for c in [0,1,2,3,u64::MAX] {
        let mut altered=tree.clone();for (i,id) in [a,b,c].iter().enumerate(){altered.contexts[0].nodes[i]=tree.contexts[0].nodes[usize::try_from(*id).ok().filter(|i|*i<3).unwrap_or(0)].clone();altered.contexts[0].nodes[i].node=NodeRef(*id);}
        let result=validate(&altered,&mut budget());println!("MATRIX {a},{b},{c}: {result:?}");
        if a<3&&b<3&&c<3&&a!=b&&a!=c&&b!=c{assert!(result.is_ok());}else{assert!(result.is_err());}
    }}}
    let mut bad=tree.clone();bad.contexts[0].nodes.pop();bad.contexts[0].nodes[0].node=NodeRef(u64::MAX);assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Selection));
    let mut bad=tree.clone();bad.contexts[0].nodes=vec![tree.contexts[0].nodes[1].clone(),tree.contexts[0].nodes[1].clone(),tree.contexts[0].nodes[0].clone()];bad.contexts[0].nodes[1].entry.alias="missing".into();assert_eq!(validate(&bad,&mut budget()),Err(TreeError::Duplicate));
    bad.contexts[0].nodes[0].entry.alias="missing".into();let result=validate(&bad,&mut budget());assert!(matches!(result,Err(TreeError::Profile(_))));println!("PRECEDENCE {result:?}");
    let tree=make(16)?;let mut full=budget();validate(&tree,&mut full).map_err(|e|format!("{e:?}"))?;
    for cap in [0,1,10,100,1000,full.usage().work-1,full.usage().work,full.usage().work+1] {
        let mut limits=budget().limits();limits.work=cap;let mut b=Budget::new(limits);let result=validate(&tree,&mut b);
        if cap<full.usage().work {assert_eq!(result,Err(TreeError::Stopped(StopReason::WorkLimit)));assert_eq!(b.poll(),Err(StopReason::WorkLimit));}else{assert!(result.is_ok());}
    }
    for allocation in [0,1,10,100,1000] {let mut limits=budget().limits();limits.allocation_units=allocation;let mut b=Budget::new(limits);assert_eq!(validate(&tree,&mut b),Err(TreeError::Stopped(StopReason::AllocationLimit)));}
    let mut b=budget();b.cancel();assert_eq!(validate(&tree,&mut b),Err(TreeError::Stopped(StopReason::Cancelled)));assert_eq!(b.usage().allocation_units,0);
    let mut b=budget();b.charge(Resource::Work,9).map_err(|e|format!("{e:?}"))?;let before=b.usage();b.cancel();assert_eq!(validate(&tree,&mut b),Err(TreeError::Stopped(StopReason::Cancelled)));assert_eq!(b.usage(),before);
    Ok(())
}
