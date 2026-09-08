from pathlib import Path
import difflib,hashlib,json,re,subprocess,sys
root=Path(__file__).resolve().parents[2];out=Path(__file__).resolve().parent
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8')
helpers=helper[helper.index('han=re.compile'):helper.index('results=[]\nfor chapter')]
sys.path.insert(0,str(root))
from tools.audit.structure import Parser,load_forms
exec(helpers)
old=(out/'before.nepld').read_bytes();new=(root/'doc/migration/authored/21-doc-pages.nepld').read_bytes()
assert b'\r' not in old+new and not new.startswith(b'\xef\xbb\xbf')
start_marker=b'  cons paragraph\n    cons sentence cons code "interfaces/doc.json"'
end_marker=b'  cons list unordered'
os=old.index(start_marker);ns=new.index(start_marker);oe=old.index(end_marker,os);ne=new.index(end_marker,ns)
assert os==ns and old[:os]==new[:ns]
old_sentence='[未登録/みとうろく]の[論理/ろんり]ページ、[非/ひ]Doc fileへのリンク、[未解決/みかいけつ]assetをこの[設定/せってい]だけで[解決/かいけつ]したとは[扱/あつか]わない。'
new_sentence='[未登録/みとうろく]の[論理/ろんり]ページや[未解決/みかいけつ]assetをinput[設定/せってい]だけで[解決/かいけつ]したとは[扱/あつか]わない。'
assert old.count(old_sentence.encode())==1 and new.count(new_sentence.encode())==1
prior=old[:os]+new[ns:ne]+old[oe:]
prior=prior.replace(old_sentence.encode(),new_sentence.encode())
marker=('        cons "'+new_sentence+'"\n        nil\n').encode();pos=prior.index(marker)+len(marker)
assert new[:pos]==prior[:pos] and new.endswith(prior[pos:])
addition=new[pos:len(new)-len(prior[pos:])]
(out/'addition.nepld').write_bytes(addition)
forms=load_forms(root);before=Parser(old.decode(),forms).complete('Doc/Article');after=Parser(new.decode(),forms).complete('Doc/Article');audit_text(after)
extra=Parser('article ja "addition" body\n'+addition.decode()+'nil',forms).complete('Doc/Article')
extra_paras=[n for n in walk(normalize(extra)) if n['kind']=='Paragraph'];assert len(extra_paras)==3
source=(out/'source.md').read_text(encoding='utf-8');start=source.index('非Doc fileはmanifest');end=source.index('\n\n全pageを同じproduction API',start)
actual=[plain(n) for n in extra_paras];expected=[p.replace('\n','').replace('`','') for p in source[start:end].split('\n\n')]
assert actual==expected,list(zip(actual,expected))
first=[n for n in walk(normalize(after)) if n['kind']=='Paragraph'][1]
srcfirst=source[source.index('`interfaces/doc.json`'):source.index('\n\n- idは')]
old_first=[n for n in walk(normalize(before)) if n['kind']=='Paragraph'][1]
assert first['fields']['items'][:2]==old_first['fields']['items'][:2]
assert ''.join(plain(s) for s in first['fields']['items'][2:])==srcfirst[srcfirst.index('`PageDocument`'):].replace('\n','').replace('`','')
replacement=Parser('article ja "replacement" body cons paragraph cons "'+new_sentence+'" nil nil',forms).complete('Doc/Article')
rp=plain([n for n in walk(normalize(replacement)) if n['kind']=='Paragraph'][0])
assert rp==next(l for l in source.splitlines() if l.startswith('未登録の論理ページや'))
codes=[n['fields']['text'] for p in [first]+extra_paras for n in walk(p) if n['kind']=='InlineCode']
assert codes==re.findall(r'`([^`]+)`',srcfirst+source[start:end])
pairs.clear();normalize(extra,True);normalize(replacement,True)
affected_ruby=len(pairs)
result={'source_commit':'2a448b0e8bafb64e8c7a0cd019ee76c197f51fcd','source_sha256':hashlib.sha256((out/'source.md').read_bytes()).hexdigest(),'old_draft_sha256':hashlib.sha256(old).hexdigest(),'draft_sha256':hashlib.sha256(new).hexdigest(),'scope':'manual author self-check; no independent review or runtime','unrelated_bytes_unchanged':True,'record_paragraph_sentences':len(first['fields']['items']),'added_paragraphs':3,'added_sentences':[len(p['fields']['items']) for p in extra_paras],'single_sentence_replacement':True,'source_text_exact':True,'inline_codes_exact':codes,'added_and_single_replacement_ruby_count':affected_ruby,'structure_and_ruby_audit':True,'utf8_lf':True}
(out/'result.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n',encoding='utf-8')
(out/'paragraphs.txt').write_text(plain(first)+'\n\n'+rp+'\n\n'+'\n\n'.join(actual)+'\n',encoding='utf-8')
(out/'readings.txt').write_text('\n'.join(a+' / '+b for a,b in pairs)+'\n',encoding='utf-8')
(out/'after.nepld').write_bytes(new)
(out/'draft.diff').write_text(''.join(difflib.unified_diff(old.decode().splitlines(True),new.decode().splitlines(True),fromfile='before.nepld',tofile='after.nepld')),encoding='utf-8')
(out/'source.diff').write_bytes(subprocess.check_output(['git','diff','6622699','2a448b0','--','doc/spec/21-doc-pages.md'],cwd=root))
for path in ['AGENTS.md','doc/authoring.md','design/forms.json','tools/audit/structure.py']:
    p=out/'context'/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((root/path).read_bytes())
(out/'context/helpers.py').write_text(helpers,encoding='utf-8')
print(json.dumps(result,ensure_ascii=False,indent=2))
