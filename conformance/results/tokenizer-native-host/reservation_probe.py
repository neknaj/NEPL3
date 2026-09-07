from pathlib import Path
import subprocess,os,json
d=Path(__file__).resolve().parent;p=d/'reservation-probe';p.mkdir(exist_ok=True)
(p/'Cargo.toml').write_text((d/'final-engine/Cargo.toml').read_text(encoding='utf-8').replace('independent-tokenizer-engine-final','independent-tokenizer-reservation'),encoding='utf-8',newline='\n')
s=(d/'final-engine/root.rs').read_text(encoding='utf-8').replace((d/'final-engine/host.rs').as_posix(),(p/'host.rs').as_posix()).replace('#[path="../engine_independent.rs"]mod independent;','''
#[test]fn independent_nested_reservation_stop_preserves_reason()->TestResult{
 for action in [host::Action::NestedReaderStop,host::Action::NestedSourceStop]{let reply=run_scenario(r#"let "x" y"#,true,Scenario{text:true,provider:true,caller_depth:7,native:Some(action),..Scenario::default()})?;assert!(matches!(reply.outcome,ParseOutcome::Stopped{reason:nepl3_core::budget::StopReason::Cancelled,..}));println!("reservation nested {action:?}: Stopped Cancelled, diagnostics {}",reply.report.diagnostics.len());}Ok(())}
''');(p/'root.rs').write_text(s,encoding='utf-8',newline='\n')
h=(d/'final-engine/host.rs').read_text(encoding='utf-8').replace('Action::NestedReaderStop => return Err(ParseError::Reader(nepl3_reader::runtime::ReaderError::Stopped(nepl3_core::budget::StopReason::Cancelled))),', 'Action::NestedReaderStop => return Err(ParseError::Context),').replace('Action::NestedSourceStop => return Err(ParseError::Source(nepl3_core::source::SourceError::Stopped(nepl3_core::budget::StopReason::Cancelled))),', 'Action::NestedSourceStop => return Err(ParseError::Context),').replace('''        Ok(Some(SourceReservation {''','''        println!("independent reservation callback reached: {:?}", self.action);
        match self.action {
            Action::NestedReaderStop => return Err(ParseError::Reader(nepl3_reader::runtime::ReaderError::Stopped(nepl3_core::budget::StopReason::Cancelled))),
            Action::NestedSourceStop => return Err(ParseError::Source(nepl3_core::source::SourceError::Stopped(nepl3_core::budget::StopReason::Cancelled))),
            _ => {}
        }
        Ok(Some(SourceReservation {''');(p/'host.rs').write_text(h,encoding='utf-8',newline='\n')
rows=[]
for target in ['native','wasm32-wasip2']:
 cmd=['cargo','test','--offline','--manifest-path',str(p/'Cargo.toml'),'--target-dir',str(d.parent/'review-fixed-target')]
 if target!='native':cmd+=['--target',target]
 cmd+=['independent_nested_reservation_stop_preserves_reason','--','--exact','--nocapture']
 env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';name=f'final-{target}-reservation.log'
 with (d/name).open('w',encoding='utf-8',newline='\n') as log:result=subprocess.run(cmd,stdout=log,stderr=subprocess.STDOUT,env=env,cwd=d.parent.parent)
 rows.append(dict(target=target,command=cmd,exit=result.returncode,log=name));print(target,result.returncode,flush=True)
 (d/'reservation-runs.json').write_text(json.dumps(rows,indent=2),encoding='utf-8',newline='\n')
