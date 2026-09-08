from pathlib import Path
import json,hashlib
R=Path(__file__).resolve().parent;W=R/'phase';rows=[];labels={}
def edit(path,fn):
 p=W/path;raw=p.read_bytes();after=fn(raw.decode('utf-8')).encode('utf-8');assert raw!=after
 name=path.replace('/','--');(R/('phase-'+name+'.before')).write_bytes(raw);(R/('phase-'+name+'.after')).write_bytes(after);p.write_bytes(after)
 rows.append(dict(path=path,before=hashlib.sha256(raw).hexdigest(),after=hashlib.sha256(after).hexdigest()))
api='\n// Isolated reviewer observation only: fixed storage, no Budget mutations.\npub static REVIEW_ENABLED: core::sync::atomic::AtomicBool = core::sync::atomic::AtomicBool::new(false);\npub static REVIEW_COUNT: core::sync::atomic::AtomicUsize = core::sync::atomic::AtomicUsize::new(0);\npub static REVIEW_DATA: [core::sync::atomic::AtomicU64; 2048] = [const { core::sync::atomic::AtomicU64::new(0) }; 2048];\npub fn review_mark(id:u64,b:&nepl3_core::budget::Budget) {\n use core::sync::atomic::Ordering::Relaxed;\n if !REVIEW_ENABLED.load(Relaxed) {return;}\n let n=REVIEW_COUNT.fetch_add(1,Relaxed);\n if n<512 { for (j,v) in [id,b.usage().work,b.usage().allocation_units,b.usage().nodes].into_iter().enumerate() { REVIEW_DATA[n*4+j].store(v,Relaxed); } }\n}\n'
edit('crates/languages/doc/core/src/lib.rs',lambda s:s+api)
def marks(path,pairs,prefix='crate'):
 def change(s):
  for needle,id,label in pairs:
   assert s.count(needle)==1,(path,needle,s.count(needle));labels[id]=label
   s=s.replace(needle,prefix+'::review_mark('+str(id)+', b);\n'+needle)
  return s
 edit(path,change)
marks('crates/languages/doc/core/src/pages.rs',[
('    registrations(set, b)?;',1,'registrations'),
('    let value = portable::pages::set_to_value',2,'set_to_value'),
('    let identity = c\n',3,'SET digest'),
('    let mut definitions = Vec::new();',4,'labels structure and declarations'),
('        let document_value = portable::pages::document_value',5,'DOCUMENT digest'),
('        let requirements = prepare::requirements',6,'requirements'),
('    let mut links = Vec::new();',7,'links'),
('    Ok(CheckedPages {',8,'resolve complete')])
marks('crates/languages/doc/html/src/pages.rs',[
('    let checked = pages::resolve',10,'render resolve'),
('    let mut unresolved = false;',11,'external URI checks'),
('        let prepared = crate::prepare::prepare_rendering',12,'render prepare'),
('        let fragment =\n',13,'render build'),
('    // Reject a link whose semantic destination',14,'output anchors'),
('    Ok(RenderedPages {',15,'render complete')],prefix='nepl3_doc_core')
def host(s):
 needle='    let review_result = render_pages'
 s=s.replace(needle,'    nepl3_doc_core::REVIEW_ENABLED.store(true, std::sync::atomic::Ordering::Relaxed);\n'+needle)
 needle='    let rendered = review_result.map_err(err)?;'
 extra='    use std::sync::atomic::Ordering::Relaxed;\n    let count=nepl3_doc_core::REVIEW_COUNT.load(Relaxed);assert!(count<=512);\n    for i in 0..count {let d=&nepl3_doc_core::REVIEW_DATA[i*4..i*4+4];eprintln!("REVIEW_PHASE {} {} {} {}",d[0].load(Relaxed),d[1].load(Relaxed),d[2].load(Relaxed),d[3].load(Relaxed));}\n'
 return s.replace(needle,extra+needle)
edit('tools/src/doc/export/pages.rs',host)
(R/'phase-instrumentation.json').write_text(json.dumps(dict(files=rows,labels=labels),indent=2)+'\n',encoding='utf-8')
