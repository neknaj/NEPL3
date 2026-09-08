    for host_first in [false,true] {for guest_first in [false,true] {
        let mut altered=tree.clone();
        let host_extra=SourceSnapshot::new(source.identity().source.clone(),1,"memory:host-other-revision".into(),b"wrong host".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?;
        altered.bundle.sources.insert(if host_first{1}else{0},host_extra);
        let FieldValue::Foreign(guest)=&mut altered.bundle.nodes[0].fields[0] else {return Err("foreign".into())};
        let guest_extra=SourceSnapshot::new(source.identity().source.clone(),2,"memory:guest-other-revision".into(),b"wrong guest".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?;
        guest.bundle.sources.insert(if guest_first{1}else{0},guest_extra);
        let mut b=budget();altered.validate(&resolved,&mut b,&mut SourceAdmission::default()).map_err(|e|format!("foreign source order: {e:?}"))?;
        println!("FOREIGN host_first={host_first} guest_first={guest_first} {:?}",b.usage());
    }}
    let mut absent=tree.clone();let FieldValue::Foreign(guest)=&mut absent.bundle.nodes[0].fields[0] else{return Err("foreign".into())};guest.bundle.sources.clear();
    let result=absent.validate(&resolved,&mut budget(),&mut SourceAdmission::default()).map(|_|());assert!(result.is_err());println!("FOREIGN_ERROR missing_guest: {result:?}");
