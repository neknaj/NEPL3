from pathlib import Path
import shutil,subprocess,os,time,json,sys
r=Path(__file__).resolve().parent;dest=r/'fault';shutil.copytree(r/'probe',dest,dirs_exist_ok=True)
p=dest/'crates/foundation/reader/src/tokenizer/session.rs';original=p.read_bytes()
old=b'Err(failure) => match prefix.restore(failure.accepted) {'
new=b'''Err(mut failure) => match prefix.restore({
                // REVIEW-ONLY fault: shorten one accepted prefix vector immediately
                // before the real public dispatch recovery. Not a legitimate input.
                if self.session_id == "fault-0" { failure.accepted.sources.clear(); }
                if self.session_id == "fault-1" { failure.accepted.source_maps.clear(); budget.cancel(); }
                failure.accepted
            }) {'''
assert original.count(old)==1;p.write_bytes(original.replace(old,new))
(r/'fault-hook.txt').write_bytes(new)
p=dest/'crates/foundation/reader/tests/independent_recover.rs';s=p.read_text()
s=s.replace('for mode in 0..7 {','for mode in 0..2 {').replace('TokenizationSession::new("recover-probe".into()', 'TokenizationSession::new(format!("fault-{mode}")')
s=s.replace('host.mode=mode;','host.mode=0;')
start=s.index('        if mode==0 {',s.index('let second='));end=s.index('\n    }\n    Ok(())',start)
s=s[:start]+'''        let Err(AcceptedTokenizationFailure::BrokenPrefix{original_error,observed_stop})=second else {panic!("fault must not issue accepted proof")};
        assert_eq!(original_error,ReaderError::Source(SourceError::IdentityConflict));
        assert_eq!(observed_stop,if mode==1{Some(StopReason::Cancelled)}else{None});
        assert_eq!(session.read(request(5).input,&store,&mut b,&mut a),Err(ReaderError::Closed));
        println!("injected prefix corruption mode {mode}: original typed error retained, stop {observed_stop:?}, public session closed");
'''+s[end:];p.write_text(s,encoding='utf-8',newline='\n')
(r/'fault-probe.rs').write_text(s,encoding='utf-8',newline='\n')
mode=sys.argv[1];env=os.environ.copy();env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run'
cmd=['cargo','test','--offline','-p','nepl3-reader','--test','independent_recover','--target-dir',str(r/(mode+'-target'))]
if 'wasi' in mode:cmd+=['--target','wasm32-wasip2']
cmd+=['--','--nocapture'];start=time.time()
with (r/(mode+'.log')).open('wb') as log:code=subprocess.run(cmd,cwd=dest,env=env,stdout=log,stderr=subprocess.STDOUT).returncode
(r/(mode+'.json')).write_text(json.dumps(dict(command=cmd,exit_code=code,elapsed_seconds=time.time()-start),indent=2)+'\n',encoding='utf-8');print(mode,code);print((r/(mode+'.log')).read_text(encoding='utf-8')[-1500:]);sys.exit(code)
