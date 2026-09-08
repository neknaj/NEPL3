from pathlib import Path
import subprocess,io,tarfile,json,hashlib
r=Path(__file__).parent;root=r.parent.parent;head='16f9f7651cf2d7cae2ea0f06baab281a4547f7bf'
raw=subprocess.check_output(['git','archive',head],cwd=root);w=r/'workspace';w.mkdir(parents=True,exist_ok=False);tarfile.open(fileobj=io.BytesIO(raw)).extractall(w,filter='data')
(r/'source.json').write_text(json.dumps(dict(head=head,archive_sha256=hashlib.sha256(raw).hexdigest()))+'\n',encoding='utf-8')
p=w/'tools/src/bootstrap/runtime.rs';s=p.read_text(encoding='utf-8');s=s.replace('.map_err(boundary)', '.map_err(|error| { eprintln!("REVIEW_BOUNDARY {} {:?}",line!(),error); boundary(error) })');s=s.replace('        let reply = if metrics.inline_host {','        eprintln!("REVIEW_PARSER_BEGIN {:?}",budget.usage());\n        let reply = if metrics.inline_host {');p.write_text(s,encoding='utf-8',newline='\n')
p=w/'tools/src/bootstrap/tests.rs';s=p.read_text(encoding='utf-8');s=s.replace('    for cap in (total.saturating_sub(32768)..total).step_by(512) {','    eprintln!("REVIEW_TOTAL {total}");\n    for cap in (total.saturating_sub(32768)..total).step_by(512) {\n        eprintln!("REVIEW_CAP {cap}");');p.write_text(s,encoding='utf-8',newline='\n')
