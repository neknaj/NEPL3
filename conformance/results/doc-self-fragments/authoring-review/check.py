from pathlib import Path
import subprocess,json,hashlib,re,importlib.util
R=Path(__file__).resolve().parent;repo=R.parents[1]
HEAD='3c67cc10246df1533f4185dd38e0b39a79801ac8';P='doc/migration/authored/21-doc-pages.nepld';S='doc/spec/21-doc-pages.md'
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,b):
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
def blob(rev,p,out):
 b=git('show',rev+':'+p);save(out,b);return b
BASE=git('rev-parse','379c85b').decode().strip();assert git('rev-parse',HEAD+'^').decode().strip()==BASE
assert git('diff','--name-only',BASE,HEAD).decode().splitlines()==[P]
old=blob(BASE,P,'before.nepld');new=blob(HEAD,P,'after.nepld');source=blob(BASE,S,'source.md');prior=blob(BASE+'^',S,'previous-source.md')
assert old==git('show','721afd9:'+P) and source==git('show',HEAD+':'+S)
text=new.decode();code=text.index('          cons code "#fragment"');start=text.rfind('      cons paragraph\n',0,code);end=text.index('      cons paragraph\n        cons sentence\n          cons code "PageLinkPlan"',code);addition=text[start:end]
assert text.replace(addition,'',1).encode()==old
md=source.decode();start=md.index('`Relative` のpathが空でfragmentが非空の場合だけ');end=md.index('`PageLinkPlan` のidentity',start);added_md=md[start:end]
assert md.replace(added_md,'',1).encode()==prior
save('addition.nepld',addition.encode());save('addition.md',added_md.encode())
for p in ['AGENTS.md','doc/authoring.md','tools/audit/structure.py','design/forms.json']:blob(HEAD,p,'context/'+p)
helper=Path('C:/projects/NEPL3-doc-phase-limits/.tmp/review-phase-limits-authoring/check.py').read_text(encoding='utf8');helper=helper[helper.index('def walk(x):'):helper.index('paragraphs=[x for x in walk(tree)')];save('context/review_helpers.py',helper.encode());exec(helper)
spec=importlib.util.spec_from_file_location('structure',R/'context/tools/audit/structure.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);forms=m.load_forms(R/'context');m.Parser(text,forms).complete('Doc/Article')
tree=m.Parser('article ja "" body\n'+addition+'nil\n',forms).complete('Doc/Article');paras=[p for p in walk(tree) if p['kind']=='Paragraph'];assert len(paras)==1
actual=plain(paras[0]);expected=re.sub(r'`([^`]+)`',r'\1',added_md.replace('\n',''));assert actual==expected
units=paras[0]['fields']['items'];assert len(units)==5
for unit in units:
 n=len(readings);s=plain(unit);del readings[n:];assert s.count('。')==1 and s.endswith('。')
codes=[x['fields']['text'] for x in walk(paras[0]) if x['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',added_md)==['Relative','#fragment','InvalidRelative']
assert sum(s['kind']=='Sentence' for s in units)==3 and sum(s['kind']=='leaf' for s in units)==2
assert b'\r' not in new and not new.startswith(b'\xef\xbb\xbf')
save('paragraph.txt',(actual+'\n').encode());save('readings.json',(json.dumps(readings,ensure_ascii=False,indent=2)+'\n').encode());save('draft.diff',git('diff',BASE,HEAD,'--',P));save('source.diff',git('diff',BASE+'^',BASE,'--',S))
author=repo/'.tmp/author-self-fragments';raw=(author/'manifest.json').read_bytes();assert sha(raw)=='94da1d583831ad1bd4fed808964d84779d94edcb792469dacbc72244c7d17521';save('author-manifest.json',raw)
for e in json.loads(raw)['files']:
 b=(author/e['path']).read_bytes();assert len(b)==e['bytes'] and sha(b)==e['sha256']
assert (author/'addition.nepld').read_bytes()==addition.encode() and (author/'after.nepld').read_bytes()==new and (author/'source.md').read_bytes()==source
git('diff','--check',BASE,HEAD)
result={'head':HEAD,'source':BASE,'before_sha256':sha(old),'after_sha256':sha(new),'source_sha256':sha(source),'addition_sha256':sha(addition.encode()),'old_draft_exact_721afd9':True,'all_old_bytes_preserved':True,'sentences':5,'paragraphs':1,'inline_code':codes,'prefix_sentences':3,'literal_sentences':2,'ruby':len(readings),'ruby_unique':len(set(map(tuple,readings))),'author_payloads_verified':len(json.loads(raw)['files']),'full_structure_audit':True,'runtime_executed':False}
save('result.json',(json.dumps(result,indent=2)+'\n').encode());print(json.dumps(result,indent=2))
