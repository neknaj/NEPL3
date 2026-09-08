#[test]
fn reviewer_index_stop_subject_observation() -> TestResult {
 use nepl3_core::{source::{SourceSnapshot,SourceId},origin::{Origin,OriginId}};
 let (mut package,registry)=fixture()?;
 let source=SourceSnapshot::new(SourceId("grammar-source".into()),0,"memory:grammar".into(),b"grammar".to_vec(),&mut budget()).map_err(|e|format!("{e:?}"))?;
 package.provenance.origins.push(Origin::Direct(source.span(0,7).map_err(|e|format!("{e:?}"))?));
 package.provenance.sources.push(source);
 package.provenance.declarations.push(DeclarationOrigin{category:None,kind:DeclarationKind::Category,name:"Expr".into(),origin:OriginId(0)});
 let mut full=budget();package.check(&registry,&mut full).map_err(|e|format!("{e:?}"))?;
 let mut limited=Budget::new(Limits{allocation_units:full.usage().allocation_units-1,..budget().limits()});
 let failure=match package.check_detailed(&registry,&mut limited){Err(e)=>e,Ok(_)=>return Err("expected stop".into())};
 eprintln!("REVIEW_SUBJECT {failure:?} usage={:?}",limited.usage());
 assert_eq!(failure.subject,None);
 Ok(())
}
