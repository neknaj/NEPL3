from pathlib import Path
import tempfile,subprocess,json,hashlib,tomllib,shutil,os,time
R=Path(__file__).resolve().parent;repo=R.parents[1];base='782309899fdc55aa9db05588f8cc0d9316ee8738'
def sha(b):return hashlib.sha256(b).hexdigest()
assert subprocess.check_output(['git','-C',str(repo),'diff',base,'--','crates/foundation'])==b''
workspace=Path(tempfile.mkdtemp(prefix='nepl3-independent-consumer-')).resolve();assert not workspace.is_relative_to(repo)
fixture=R/'initial/conformance/extensions/hello';shutil.copytree(fixture/'src',workspace/'src');shutil.copyfile(fixture/'Cargo.lock',workspace/'Cargo.lock')
manifest=(fixture/'Cargo.toml').read_text(encoding='utf-8')
for n in ['core','reader','engine','wire']:manifest=manifest.replace('"../../../crates/foundation/'+n+'"',json.dumps((repo/'crates/foundation'/n).as_posix()))
(workspace/'Cargo.toml').write_text(manifest,encoding='utf-8',newline='\n')
# Independent variation of the supplied test adapter: fresh receiving codec has no ambient source.
tests=(workspace/'src/tests.rs').read_text(encoding='utf-8')
old='''        let restored = portable::tree::from_value(&decoded, &resolved, &mut codec, &mut transfer)
            .map_err(error)?;'''
new='''        let received_sources = SourceStore::default();
        let mut received_admission = SourceAdmission::default();
        let mut receiver = FoundationCodec::new(&registry, &received_sources, &mut received_admission).map_err(error)?;
        let restored = portable::tree::from_value(&decoded, &resolved, &mut receiver, &mut transfer)
            .map_err(error)?;'''
assert tests.count(old)==1;tests=tests.replace(old,new)
tests+='''
#[test]
fn independent_missing_final_operand_and_unfinished_unicode() -> Result<(), String> {
    let missing = run("hello", true, false)?;
    assert!(matches!(missing.outcome, ParseOutcome::Recovered { .. }));
    assert!(!missing.report.diagnostics.is_empty());
    let unfinished = run("hello 世界", false, false)?;
    assert!(matches!(unfinished.outcome, ParseOutcome::NeedMore { .. }));
    Ok(())
}

#[test]
fn independent_invalid_package_read_reference_is_rejected() -> Result<(), String> {
    let (mut package, registry) = language()?;
    package.forms[0].fields[0].read = ReadSpecId(u64::MAX);
    assert!(package.check(&registry, &mut budget()).is_err());
    Ok(())
}
'''
(workspace/'src/tests.rs').write_text(tests,encoding='utf-8',newline='\n')
for p in workspace.rglob('*'):
 if p.is_file():q=R/'actual-source'/p.relative_to(workspace);q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(p.read_bytes())
toolchain=tomllib.loads((repo/'rust-toolchain.toml').read_text(encoding='utf-8'))['toolchain']['channel']
commands=[['cargo','+'+toolchain,'metadata','--locked','--format-version','1'],['cargo','+'+toolchain,'test','--locked'],['cargo','+'+toolchain,'test','--locked','--target','wasm32-wasip2']]
record={'scope':'independent copied consumer and public APIs; fresh source-less receiving codec; not independent release/provider','base':base,'workspace':str(workspace),'outside_repository':True,'runs':[]}
env=dict(os.environ);env['CARGO_TARGET_WASM32_WASIP2_RUNNER']='wasmtime run';env['CARGO_TARGET_DIR']=str(workspace/'target')
for i,command in enumerate(commands):
 log=R/f'actual-{i}.log';start=time.monotonic()
 with log.open('wb') as f:
  try:r=subprocess.run(command,cwd=workspace,env=env,stdout=f,stderr=subprocess.STDOUT,timeout=600);code=r.returncode
  except subprocess.TimeoutExpired:code=None
 record['runs'].append({'command':command,'cwd':str(workspace),'exit':code,'elapsed_seconds':time.monotonic()-start,'log':log.name,'sha256':sha(log.read_bytes())})
 (R/'actual-results.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n');print(i,code,flush=True)
 if code!=0:break
 if i==0:
  metadata=json.loads(log.read_bytes());assert Path(metadata['workspace_root']).resolve()==workspace
  ps=[p for p in metadata['packages'] if p['name'].startswith('nepl3-')];assert len(ps)==4 and len({p['name'] for p in ps})==4
  for p in ps:assert Path(p['manifest_path']).resolve()==repo/'crates/foundation'/p['name'][6:]/'Cargo.toml'
  record['metadata_nepl3_packages']=[{'id':p['id'],'name':p['name'],'manifest_path':p['manifest_path']} for p in ps]
record['foundation_git_diff_empty']=subprocess.check_output(['git','-C',str(repo),'diff',base,'--','crates/foundation'])==b''
record['binaries']=[{'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in (workspace/'target').rglob('*') if p.is_file() and p.suffix in ['.exe','.wasm']]
(R/'actual-results.json').write_text(json.dumps(record,indent=2)+'\n',encoding='utf-8',newline='\n')
