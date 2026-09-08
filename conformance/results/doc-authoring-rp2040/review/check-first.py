from pathlib import Path
from html.parser import HTMLParser
import re,json,hashlib
p=Path('.tmp/review-rp2040-guide')
class H(HTMLParser):
 def __init__(self):super().__init__();self.ps=[];self.current=None;self.codes=[];self.pre=None;self.links=[];self.heads=[];self.h=None
 def handle_starttag(self,t,a):
  a=dict(a)
  if t=='p':self.current=''
  if t=='pre':self.pre=''
  if t in ['h1','h2']:self.h=''
  if t=='a':self.links.append(a['href'])
 def handle_data(self,d):
  if self.current is not None:self.current+=d
  if self.pre is not None:self.pre+=d
  if self.h is not None:self.h+=d
 def handle_endtag(self,t):
  if t=='p':self.ps.append(self.current);self.current=None
  if t=='pre':self.codes.append(self.pre);self.pre=None
  if t in ['h1','h2']:self.heads.append(self.h);self.h=None
h=H();h.feed((p/'output/index.html').read_text(encoding='utf8'))
s=(p/'source.md').read_text();codes=re.findall(r'```sh\n(.*?)```',s,re.S);print(repr(codes),repr(h.codes));assert codes==h.codes
assert h.heads==['ARMv6-M build and RP2040 execution','Reproduce','Scope and constraints']
assert re.findall(r'\]\(([^)]+)\)',s)==h.links
s=re.sub(r'```sh\n.*?```','',s,flags=re.S)
paragraphs=[]
for b in re.split(r'\n\s*\n',s):
 if not b.strip() or b.startswith('#'):continue
 for item in re.split(r'\n(?=- )',b):
  item=re.sub(r'^- ','',item)
  item=re.sub(r'\[([^]]+)\]\([^)]+\)',r'\1',item).replace('`','').replace('**','')
  paragraphs.append(re.sub(r'\s+',' ',item).strip())
assert len(paragraphs)==len(h.ps),(len(paragraphs),len(h.ps))
diffs=[{'paragraph':n+1,'source':a,'html':b}for n,(a,b) in enumerate(zip(paragraphs,h.ps)) if a!=b]
for a,b in zip(paragraphs,h.ps):assert a.replace(' ','')==b.replace(' ',''),(a,b)
(p/'comparison.json').write_text(json.dumps({'paragraph_count':len(paragraphs),'differences':diffs,'rawcode_byte_equal':True,'headings_equal':True,'link_targets_equal':True,'all_text_equal_if_ASCII_spaces_removed':True},indent=2)+'\n',encoding='utf8');print(len(paragraphs),len(diffs))
