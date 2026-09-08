from pathlib import Path
import subprocess,tarfile,io,json,hashlib
r=Path(__file__).parent;root=r.parent.parent;head=subprocess.check_output(['git','rev-parse','459dcbc'],cwd=root,text=True).strip()
raw=subprocess.check_output(['git','archive',head],cwd=root);w=r/'workspace';w.mkdir(parents=True,exist_ok=False);tarfile.open(fileobj=io.BytesIO(raw)).extractall(w,filter='data')
(r/'source.json').write_text(json.dumps(dict(head=head,archive_sha256=hashlib.sha256(raw).hexdigest()))+'\n',encoding='utf-8')
p=w/'tools/src/bootstrap/tests.rs';s=p.read_text(encoding='utf-8');s=s.replace('    for cap in (total.saturating_sub(32768)..total).step_by(512) {','    eprintln!("REVIEW_TOTAL {total}");\n    for cap in (total.saturating_sub(32768)..total).step_by(512) {\n        eprintln!("REVIEW_CAP {cap}");');s=s.replace('            Err(runtime::RuntimeError::PreparationStopped { reason, usage }) => {','            Err(runtime::RuntimeError::PreparationStopped { reason, usage }) => {\n                eprintln!("REVIEW_PREPARATION cap={cap} usage={usage:?}");');s=s.replace('                    stopped_after_parse += 1;','                    stopped_after_parse += 1;\n                    eprintln!("REVIEW_COMPLETE_STOP cap={cap} usage={:?}",b.usage());');p.write_text(s,encoding='utf-8',newline='\n')
# Direct real with_tree: invalid compiled root must remain Boundary, never a fake stop.
extra='''
#[test]
fn reviewer_preparation_nonstop_error_remains_boundary() -> crate::Result<()> {
    let mut b=budget();
    let mut compiled=lower_fixture(&mut b)?;
    compiled.package.root="missing-root-context".into();
    let source=SourceSnapshot::new(SourceId("review".into()),0,"memory:review".into(),b"language Demo 1 Root nil".to_vec(),&mut b).map_err(|e|format!("{e:?}"))?;
    let mut operation=budget();
    let result=runtime::with_tree(&source,&compiled,nepl3_core::source::Digest::of(b"review implementation"),&mut operation,&mut SourceAdmission::default(),|_,_,_,_|Ok(()));
    eprintln!("REVIEW_NONSTOP {result:?} usage={:?}",operation.usage());
    assert!(matches!(result,Err(runtime::RuntimeError::Boundary(_))));
    assert_eq!(operation.poll(),Ok(()));
    Ok(())
}
'''
p.write_text(s+extra,encoding='utf-8',newline='\n')
