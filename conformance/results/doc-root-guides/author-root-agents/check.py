from pathlib import Path
import re,json,hashlib,subprocess
r=Path(__file__).parent;repo=r.parents[1];commit=subprocess.check_output(['git','-C',str(repo),'rev-parse','b83716c']).decode().strip()
source=subprocess.check_output(['git','-C',str(repo),'show',commit+':AGENTS.md']);assert source==(repo/'AGENTS.md').read_bytes();(r/'AGENTS.md').write_bytes(source)
raw=(repo/'doc/migration/authored/project/AGENTS.nepld').read_bytes();assert b'\r' not in raw;(r/'AGENTS.nepld').write_bytes(raw)
arities={'article':3,'section':3,'body':1,'paragraph':1,'list':2,'item':2,'cons':2,'nil':0,'none':0,'sentence':1,'text':1,'ruby':2,'code':1,'link':2,'relative':2,'external':1,'concat':1}
tokens=re.findall(r'"(?:[^"\\]|\\.)*"|[^\s]+',raw.decode('utf-8'));i=0
def parse():
 global i
 t=tokens[i];i+=1
 if t.startswith('"'):return json.loads(t)
 if t not in arities:return t
 return (t,[parse()for _ in range(arities[t])])
tree=parse();assert i==len(tokens)
def seq(n):
 if n[0]=='nil':return []
 assert n[0]=='cons';return [n[1][0]]+seq(n[1][1])
readings=[];links=[];codes=[];sentences=[]
def literal(s):
 def ruby(m):readings.append((m[1],m[2]));return m[1]
 remainder=re.sub(r'\[([^\[\]/]+)/([^\[\]/]+)\]', '',s);assert not re.search('[一-龯々]',remainder),remainder
 return re.sub(r'\[([^\[\]/]+)/([^\[\]/]+)\]',ruby,s)
def inline(n):
 k,a=n
 if k=='text':assert not re.search('[一-龯々]',a[0]),a[0];return a[0]
 if k=='ruby':assert a[0][0]==a[1][0]=='text';readings.append((a[0][1][0],a[1][1][0]));return a[0][1][0]
 if k=='code':codes.append(a[0]);return a[0]
 if k=='concat':return ''.join(inline(x)for x in seq(a[0]))
 if k=='link':label=inline(a[1]);links.append((label,a[0][1][0]));return label
 raise AssertionError(k)
def sentence(n):
 s=literal(n) if isinstance(n,str)else ''.join(inline(x)for x in seq(n[1][0]));assert s.count('。')==1;sentences.append(s);return s
blocks=[('h1',literal(tree[1][1]))]
def body(n,item=False):
 assert n[0]=='body'
 for k,a in seq(n[1][0]):
  if k=='section':blocks.append(('h2',literal(a[1])));body(a[2])
  elif k=='paragraph':blocks.append(('li'if item else'p',''.join(sentence(x)for x in seq(a[0]))))
  elif k=='list':
   assert a[0]=='unordered'
   for entry in seq(a[1]):assert entry[0]=='item'and entry[1][0]==('none',[]);body(entry[1][1],True)
  else:raise AssertionError(k)
body(tree[1][2])
mdlinks=[];mdcodes=[];expected=[]
def flatten(s):
 def l(m):mdlinks.append((m[1],m[2]));return m[1]
 def c(m):mdcodes.append(m[1]);return m[1]
 return re.sub(r'`([^`]+)`',c,re.sub(r'\[([^\]]+)\]\(([^)]+)\)',l,s))
for line in source.decode().splitlines():
 if not line:continue
 if line.startswith('## '):expected.append(('h2',line[3:]))
 elif line.startswith('# '):expected.append(('h1',line[2:]))
 elif line.startswith('- '):expected.append(('li',flatten(line[2:])))
 else:expected.append(('p',flatten(line)))
assert blocks==expected,[(a,b)for a,b in zip(blocks,expected)if a!=b]
assert links==mdlinks and codes==mdcodes
assert all(re.fullmatch('[一-龯々]+',base)and re.fullmatch('[ぁ-ゖー]+',reading)for base,reading in readings)
result={'source_commit':commit,'source_sha256':hashlib.sha256(source).hexdigest(),'draft_sha256':hashlib.sha256(raw).hexdigest(),'draft_bytes':len(raw),'blocks':blocks,'sentences':sentences,'readings':readings,'links':links,'codes':codes,'exact_text_and_structure':True}
(r/'checks.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(len(blocks),len(sentences),len(readings),len(links),len(codes))
