from pathlib import Path
from html.parser import HTMLParser
import re,json,hashlib,importlib.util,mistune
p=Path(__file__).resolve().parent;w=p/'workspace'
class Blocks(HTMLParser):
 def __init__(self):super().__init__();self.rows=[];self.active=None;self.text=[];self.lists=[];self.code=0
 def handle_starttag(self,t,a):
  if t=='ul':self.lists.append(t)
  if t in ['h1','h2','li'] or (t=='p' and self.active is None):
   assert self.active is None
   self.active=t;self.text=[]
  if t=='code' and self.active:self.text.append('<CODE>');self.code+=1
  if t=='br' and self.active:self.text.append('<BREAK>')
 def handle_endtag(self,t):
  if t=='code' and self.active:self.text.append('</CODE>')
  if t==self.active:
   self.rows.append((t,re.sub(r'\s+',' ',''.join(self.text)).strip()));self.active=None
 def handle_data(self,s):
  if self.active:self.text.append(s)
source=(w/'doc/spec/00-contract.md').read_text(encoding='utf-8')
a=Blocks();a.feed(mistune.html(source))
for name in ['output/document.html','site/reference/contract/index.html']:
 b=Blocks();b.feed((p/name).read_text(encoding='utf-8'));assert a.rows==b.rows and a.code==b.code and a.lists==b.lists
 print(name,'ordered blocks/code/list match',len(b.rows),b.code,len(b.lists))
spec=importlib.util.spec_from_file_location('candidate',w/'tools/migration/contract.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)
assert m.generate(source).encode()==(w/'doc/migration/00-contract.nepld').read_bytes()
for row in json.loads((p/'negative-cases.json').read_text(encoding='utf-8')):
 try:m.generate(source.replace('\n## 1. ','\n'+row['markdown']+'\n\n## 1. ',1))
 except ValueError:continue
 raise AssertionError(row['name'])
try:m.generate((p/'loose-list.md').read_text(encoding='utf-8'))
except ValueError:pass
else:raise AssertionError('loose list')
for folder in ['output','site']:
 manifest=json.loads((p/folder/'manifest.json').read_text(encoding='utf-8'))
 for f in manifest['files']:assert hashlib.sha256((p/folder/f['path']).read_bytes()).hexdigest()==f['sha256']
print('14 independent negatives rejected; candidate and both artifacts hash/content checked; mistune',mistune.__version__)
