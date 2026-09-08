from pathlib import Path
import subprocess,json,hashlib,re,importlib.util
R=Path(__file__).resolve().parent;repo=R.parents[1]
HEAD='721afd9205ef21855e4cfff8bd3431498bd52f3d';BASE='2a448b0e8bafb64e8c7a0cd019ee76c197f51fcd';P='doc/migration/authored/21-doc-pages.nepld';S='doc/spec/21-doc-pages.md'
def git(*a):return subprocess.check_output(['git','-C',str(repo),*a])
def sha(b):return hashlib.sha256(b).hexdigest()
def save(p,b):
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
def blob(rev,p,out):
 b=git('show',rev+':'+p);save(out,b);return b
assert git('rev-parse',HEAD+'^').decode().strip()==BASE
assert git('diff','--name-only',BASE,HEAD).decode().splitlines()==[P]
old=blob(BASE,P,'before.nepld');new=blob(HEAD,P,'after.nepld');source=blob(BASE,S,'source.md');prior=blob(BASE+'^',S,'previous-source.md')
assert source==git('show',HEAD+':'+S)
for path in ['doc/authoring.md','AGENTS.md','tools/audit/structure.py','design/forms.json']:blob(HEAD,path,'context/'+path)
helper=Path('C:/projects/NEPL3-doc-phase-limits/.tmp/review-phase-limits-authoring/check.py').read_text(encoding='utf8')
helpers=helper[helper.index('def walk(x):'):helper.index('paragraphs=[x for x in walk(tree)')]
save('context/review_helpers.py',helpers.encode());exec(helpers)
spec=importlib.util.spec_from_file_location('structure',R/'context/tools/audit/structure.py');m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m);forms=m.load_forms(R/'context')
a=m.Parser(old.decode(),forms).complete('Doc/Article');b=m.Parser(new.decode(),forms).complete('Doc/Article')
para_a=[p for p in walk(a) if p['kind']=='Paragraph'];para_b=[p for p in walk(b) if p['kind']=='Paragraph']
reg_a=para_a[1];reg_b=para_b[1]
assert reg_a['fields']['items'][:2]==reg_b['fields']['items'][:2]
start=b'  cons paragraph\n    cons sentence cons code "interfaces/doc.json"';end=b'  cons list unordered'
os=old.index(start);oe=old.index(end,os);ns=new.index(start);ne=new.index(end,ns)
old_line='        cons "[未登録/みとうろく]の[論理/ろんり]ページ、[非/ひ]Doc fileへのリンク、[未解決/みかいけつ]assetをこの[設定/せってい]だけで[解決/かいけつ]したとは[扱/あつか]わない。"\n'
new_line='        cons "[未登録/みとうろく]の[論理/ろんり]ページや[未解決/みかいけつ]assetをinput[設定/せってい]だけで[解決/かいけつ]したとは[扱/あつか]わない。"\n'
insert_at=new.index(new_line.encode())+len(new_line.encode())+len(b'        nil\n')
end_at=new.index(b'      cons paragraph\n        cons sentence\n          cons text "',insert_at)
# Locate the unchanged following paragraph by its original complete source suffix.
baseline=old[:os]+new[ns:ne]+old[oe:];baseline=baseline.replace(old_line.encode(),new_line.encode(),1)
assert new[:insert_at]==baseline[:insert_at] and new.endswith(baseline[insert_at:])
addition=new[insert_at:len(new)-len(baseline[insert_at:])]
assert new.replace(addition,b'',1).replace(new_line.encode(),old_line.encode(),1).replace(new[ns:ne],old[os:oe],1)==old
save('addition.nepld',addition);save('registration.nepld',new[ns:ne])
extra=m.Parser('article ja "" body\n'+addition.decode()+'nil\n',forms).complete('Doc/Article');paras=[x for x in walk(extra) if x['kind']=='Paragraph']
md=source.decode();i=md.index('非Doc fileはmanifest');j=md.index('\n\n全pageを同じproduction API',i);added_md=md[i:j]
expected=[re.sub(r'`([^`]+)`',r'\1',p.replace('\n','')) for p in added_md.split('\n\n')];actual=[plain(p) for p in paras];assert actual==expected
reg_md=md[md.index('`interfaces/doc.json`'):md.index('\n\n- idは')]
assert plain(reg_b['fields']['items'][2:])==re.sub(r'`([^`]+)`',r'\1',reg_md[reg_md.index('`PageDocument`'):].replace('\n',''))
changed=m.Parser('article ja "" body cons paragraph\n'+new_line+'nil nil',forms).complete('Doc/Article')
cp=next(x for x in walk(changed) if x['kind']=='Paragraph');assert plain(cp)==next(x for x in md.splitlines() if x.startswith('未登録の論理ページや'))
counts=[];units=[]
for p in paras:
 u=p['fields']['items'];counts.append(len(u));units+=u
assert counts==[9,6,5]
units+=reg_b['fields']['items'][2:]+cp['fields']['items']
for unit in units:
 n=len(readings);s=plain(unit);del readings[n:];assert s.count('。')==1 and s.endswith('。')
codes=[x['fields']['text'] for p in [reg_b]+paras for x in walk(p) if x['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',reg_md+added_md)
assert len(codes)==21 and len(units)==25
assert b'\r' not in new and not new.startswith(b'\xef\xbb\xbf')
save('addition.md',added_md.encode());save('registration.md',reg_md.encode());save('paragraphs.txt',('\n\n'.join(actual)+'\n').encode());save('readings.json',(json.dumps(readings,ensure_ascii=False,indent=2)+'\n').encode())
save('draft.diff',git('diff',BASE,HEAD,'--',P));save('source.diff',git('diff',BASE+'^',BASE,'--',S))
author=repo/'.tmp/author-resource-links';raw=(author/'manifest.json').read_bytes();assert sha(raw)=='05a2f974a8c740645449e99632d13d3238b4ab833809481d3235a25801034eaa';save('author-manifest.json',raw)
for e in json.loads(raw)['files']:
 data=(author/e['path']).read_bytes();assert len(data)==e['bytes'] and sha(data)==e['sha256']
assert (author/'addition.nepld').read_bytes()==addition and (author/'after.nepld').read_bytes()==new and (author/'source.md').read_bytes()==source
git('diff','--check',BASE,HEAD)
result={'head':HEAD,'source':BASE,'before_sha256':sha(old),'after_sha256':sha(new),'source_sha256':sha(source),'added_paragraph_sentences':counts,'affected_registration_sentences':4,'single_qualification_sentence':1,'affected_sentences':25,'first_two_registration_sentences_unchanged':True,'other_bytes_preserved':True,'inline_codes_exact':codes,'ruby_occurrences_reviewed':len(readings),'ruby_unique':len(set(map(tuple,readings))),'author_payloads_verified':len(json.loads(raw)['files']),'structure':True,'runtime_executed':False}
save('result.json',(json.dumps(result,indent=2)+'\n').encode());print(json.dumps(result,indent=2))
