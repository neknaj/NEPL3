    use nepl3_engine::tree::TreeError;
    for flip_context in [false,true] {for host_order in [false,true] {for guest_order in [false,true] {
        let mut altered=tree.clone();
        for context in &mut altered.contexts {if if context.path.is_empty(){host_order}else{guest_order}{context.nodes.reverse();}}
        if flip_context {altered.contexts.reverse();}
        let mut b=budget();altered.validate(&resolved,&mut b,&mut SourceAdmission::default()).map_err(|e|format!("foreign permutation {e:?}"))?;
        println!("FOREIGN {flip_context} {host_order} {guest_order} {:?}",b.usage());
    }}}
    for guest in [false,true] {
        let mut bad=tree.clone();let context=bad.contexts.iter_mut().find(|c|(!c.path.is_empty())==guest).ok_or("context")?;
        context.nodes[1]=context.nodes[0].clone();
        let result=bad.validate(&resolved,&mut budget(),&mut SourceAdmission::default()).map(|_|());
        println!("FOREIGN_ERROR guest={guest}: {result:?}");
        // Parent fields are checked in selection order. Missing child selection
        // is encountered before the second duplicate parent in both revisions.
        assert_eq!(result,Err(TreeError::Selection));
    }
    let mut bad=tree.clone();let guest=bad.contexts.iter_mut().find(|c|!c.path.is_empty()).ok_or("guest")?;
    guest.nodes[1].entry.alias="Host".into();
    assert!(bad.validate(&resolved,&mut budget(),&mut SourceAdmission::default()).is_err());
    let mut bad=tree.clone();bad.contexts[1].path=bad.contexts[0].path.clone();
    assert!(matches!(bad.validate(&resolved,&mut budget(),&mut SourceAdmission::default()),Err(TreeError::Duplicate)));
