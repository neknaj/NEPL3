from pathlib import Path
import subprocess,hashlib,tarfile,io,json
R=Path(__file__).resolve().parent;W=R/'workspace'
repo='C:/projects/NEPL3-completed-tree-owned-validation';rev='7251b0199ce5ba4216fc78060ed32b012c70e09f'
assert not W.exists()
raw=subprocess.check_output(['git','-C',repo,'archive','--format=tar',rev],timeout=60)
W.mkdir()
with tarfile.open(fileobj=io.BytesIO(raw)) as tar:tar.extractall(W,filter='data')
(R/'archive.json').write_text(json.dumps(dict(commit=rev,sha256=hashlib.sha256(raw).hexdigest(),bytes=len(raw)))+'\n',encoding='utf-8')
changes=[]
def edit(path,fun):
 p=W/path;b=p.read_bytes();a=fun(b.decode()).encode();assert a!=b
 name=path.replace('/','--')
 (R/(name+'.before')).write_bytes(b);(R/(name+'.after')).write_bytes(a);p.write_bytes(a)
 changes.append(dict(path=path,before=hashlib.sha256(b).hexdigest(),after=hashlib.sha256(a).hexdigest()))
api='''
// Scratch-only observation/fault hooks. No production Budget charges changed.
pub static REVIEW_MODE: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(1);
pub static REVIEW_OK: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
pub static REVIEW_STOP: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
pub static REVIEW_ERROR: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
pub static REVIEW_PHASE: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);
pub static REVIEW_WORK: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub static REVIEW_ALLOC: core::sync::atomic::AtomicU64 = core::sync::atomic::AtomicU64::new(0);
pub fn review_phase(n:usize,b:&nepl3_core::budget::Budget) { REVIEW_PHASE.store(n,core::sync::atomic::Ordering::Relaxed); REVIEW_WORK.store(b.usage().work,core::sync::atomic::Ordering::Relaxed); REVIEW_ALLOC.store(b.usage().allocation_units,core::sync::atomic::Ordering::Relaxed); }
'''
edit('crates/foundation/engine/src/parse/mod.rs',lambda s:s+api)
def session(s):
 a='        let progress = &mut machine.progress;'
 assert s.count(a)==1
 s=s.replace(a,'        let review_mode = super::REVIEW_MODE.load(core::sync::atomic::Ordering::Relaxed);\n        let review_before = (review_mode != 0).then(|| machine.progress.clone());\n'+a)
 a='        let result = tree.validate(self.profile, budget, admission).map(|_| ());'
 assert s.count(a)==1
 s=s.replace(a,'        if review_mode == 2 { tree.profile_digest = nepl3_core::source::Digest::of(b"scratch mismatched profile"); }\n        super::review_phase(30, budget);\n'+a+'\n        if result.is_ok() { super::review_phase(31, budget); }')
 a='        result.map_err(ParseError::from)'
 assert s.count(a)==1
 s=s.replace(a,'''        if let Some(before) = review_before { assert_eq!(machine.progress, before, "entire progress must restore"); }
        match &result {
            Ok(()) => { super::REVIEW_OK.fetch_add(1, core::sync::atomic::Ordering::Relaxed); }
            Err(_) if budget.poll().is_err() => { super::REVIEW_STOP.fetch_add(1, core::sync::atomic::Ordering::Relaxed); }
            Err(_) => { super::REVIEW_ERROR.fetch_add(1, core::sync::atomic::Ordering::Relaxed); }
        }
'''+a)
 return s
edit('crates/foundation/engine/src/parse/session.rs',session)
def tree(s):
 pairs=[('        let syntax = self\n','        crate::parse::review_phase(60, budget);\n        let syntax = self\n'),('        let mut contexts: Vec<(&SyntaxBundle, &BundleContext)> = Vec::new();','        crate::parse::review_phase(61, budget);\n        let mut contexts: Vec<(&SyntaxBundle, &BundleContext)> = Vec::new();'),('        let mut recoveries = Vec::new();','        crate::parse::review_phase(62, budget);\n        let mut recoveries = Vec::new();'),('        let mut pending = Vec::new();\n        push(&mut pending, (&self.bundle, 1u64), budget)?;','        crate::parse::review_phase(63, budget);\n        let mut pending = Vec::new();\n        push(&mut pending, (&self.bundle, 1u64), budget)?;')]
 for a,b in pairs:assert s.count(a)==1,(a,s.count(a));s=s.replace(a,b)
 return s
edit('crates/foundation/engine/src/tree.rs',tree)
def main(s):
 a='    match run() {'
 assert s.count(a)==1
 return s.replace(a,'''    nepl3_engine::parse::REVIEW_MODE.store(0, std::sync::atomic::Ordering::Relaxed);
    let result = run();
    eprintln!("REVIEW_PHASE={} start_work={} start_alloc={}", nepl3_engine::parse::REVIEW_PHASE.load(std::sync::atomic::Ordering::Relaxed),nepl3_engine::parse::REVIEW_WORK.load(std::sync::atomic::Ordering::Relaxed),nepl3_engine::parse::REVIEW_ALLOC.load(std::sync::atomic::Ordering::Relaxed));
    match result {''')
edit('tools/src/main.rs',main)
probe=(R/'probe.rs').read_text(encoding='utf-8')
edit('crates/foundation/engine/tests/parse.rs',lambda s:s+'\n'+probe)
(R/'instrumentation.json').write_text(json.dumps(changes,indent=2)+'\n',encoding='utf-8')
print('ready',rev,len(changes))
