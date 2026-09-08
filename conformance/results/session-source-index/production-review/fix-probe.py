from pathlib import Path
r=Path(__file__).parent
for name in ['workspace','before']:
 p=r/name/'crates/foundation/engine/tests/parse.rs';s=p.read_text(encoding='utf-8').replace('reply.usage);','reply.report.usage);')
 s=s.replace('("prefix", "let x let y z".to_string(),Scenario::default()),','''("prefix", "let x let y z".to_string(),Scenario::default()),
  ("native12",format!("{}y", "let x ".repeat(12)),Scenario{provider:true,native:Some(host::Action::Serve),..Scenario::default()}),
  ("native24",format!("{}y", "let x ".repeat(24)),Scenario{provider:true,native:Some(host::Action::Serve),..Scenario::default()}),''')
 p.write_text(s,encoding='utf-8',newline='\n')
p=r/'workspace/crates/foundation/engine/src/parse/build/tests.rs'
with p.open('a',encoding='utf-8',newline='\n')as f:f.write(r'''
#[test]
fn reviewer_partial_conflict_retry_preserves_cached_prefix() -> Result<(),String> {
 let old=source("a",0,"mem:a",b'a').map_err(|e|format!("{e:?}"))?;
 let b=source("b",0,"mem:b",b'b').map_err(|e|format!("{e:?}"))?;
 let conflict=source("a",0,"mem:wrong",b'a').map_err(|e|format!("{e:?}"))?;
 let c=source("c",0,"mem:c",b'c').map_err(|e|format!("{e:?}"))?;
 let mut arena=ParseArena{sources:vec![old.clone()],..ParseArena::default()};
 let mut meter=budget();let mut index=SourceIndex::new(&arena.sources,&mut meter).map_err(|e|format!("{e:?}"))?;
 assert_eq!(arena.extend_sources_indexed(&[b.clone(),conflict],&mut index,&mut meter),Err(SyntaxError::Source(SourceError::IdentityConflict)));
 assert!(meter.poll().is_ok());assert_eq!(arena.sources,vec![old.clone(),b.clone()]);
 arena.extend_sources_indexed(&[b.clone(),c.clone()],&mut index,&mut meter).map_err(|e|format!("{e:?}"))?;
 assert_eq!(arena.sources,vec![old,b,c]);
 for(i,s)in arena.sources.iter().enumerate(){let at=source_position(&index.entries,&arena.sources,s,None,&mut meter).map_err(|e|format!("{e:?}"))?.map_err(|_|"missing")?;assert_eq!(index.entries[at],i);}
 Ok(())
}
''')
