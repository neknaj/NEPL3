from pathlib import Path
from html.parser import HTMLParser
import subprocess,hashlib,json
p=Path(__file__).resolve().parent;d=p/'cases';d.mkdir(exist_ok=True);exe=p/'target/debug/nepl3-tools.exe';rows=[]
source=b'article en "Title" body cons paragraph cons "Text" nil nil'
def run(name,data=source,input_name='source.nepld',existing=False,missing_parent=False,arg=None):
 root=d/name;root.mkdir(exist_ok=True);f=root/input_name
 if arg is None:f.write_bytes(data)
 out=root/('missing/out.md' if missing_parent else 'out.md')
 if existing:out.write_bytes(b'KEEP')
 args=[str(exe),'doc-markdown',arg if arg is not None else str(f),str(out)]
 r=subprocess.run(args,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=60);(root/'run.log').write_bytes(r.stdout)
 row={'name':name,'exit':r.returncode,'output_exists':out.exists(),'stdout':r.stdout.decode('utf-8',errors='replace')};rows.append(row)
 if r.returncode==0:
  text=out.read_text(encoding='utf-8');assert 'source SHA-256 '+hashlib.sha256(data).hexdigest() in text;assert 'renderer nepl3-tools.markdown/1' in text
  first=text.split('\n\n',1)[0];assert first.count('<!--')==1 and first.count('-->')==1
  class Comments(HTMLParser):
   def __init__(self):super().__init__();self.comments=[];self.other=[]
   def handle_comment(self,s):self.comments.append(s)
   def handle_starttag(self,t,a):self.other.append(t)
  c=Comments();c.feed(first);assert len(c.comments)==1 and not c.other
  row['sha256']=hashlib.sha256(out.read_bytes()).hexdigest();row['comment']=first
 elif existing:assert out.read_bytes()==b'KEEP'
 else:assert not out.exists()
 print(name,r.returncode,flush=True);return out
run('basic')
run('comment',input_name='source--&.nepld')
run('unicode',input_name='\u6587\u66f8--&.nepld',data='article ja "\u6587\u66f8" body nil'.encode())
run('crlf',data=b'article en "Title"\r\nbody nil\r\n')
run('existing',existing=True)
run('missing-parent',missing_parent=True)
run('invalid-utf8',data=b'\xff')
run('truncated-utf8',data=source+b'\xe3\x81')
run('oversize',data=b' '*10000001)
run('trailing',data=source+b' garbage')
run('broken',data=b'article en "unterminated')
run('unsupported',data=b'article en "Title" body cons paragraph cons "[Word/read]" nil nil')
run('path-limit',arg='a'*4097)
run('path-control',arg='bad\nname.nepld')
run('missing-file',arg=str(d/'missing.nepld'))
raw=(p/'workspace/doc/migration/00-contract.nepld').read_bytes();run('candidate',data=raw)
(p/'cli-results.json').write_text(json.dumps(rows,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
