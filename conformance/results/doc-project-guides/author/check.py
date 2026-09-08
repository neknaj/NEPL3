from pathlib import Path
import sys,re,json,hashlib,collections
r=Path(__file__).resolve().parent;repo=r.parents[1]
helper=Path('C:/projects/NEPL3-doc-authoring-delivery/.tmp/review-b-ruby/check.py').read_text(encoding='utf-8').split('results=[]\nfor chapter')[0]
helper=helper.replace("sys.path.insert(0,str(root/'context'))","sys.path.insert(0,"+repr(str(repo))+")").replace("forms=load_forms(root/'context')","forms=load_forms(Path("+repr(str(repo))+"))")
ns={'__file__':str(r/'structure-helper.py')};(r/'structure-helper.py').write_text(helper,encoding='utf-8',newline='\n');exec(helper,ns)
results=[]
for name in sys.argv[1:]:
 p=repo/'doc/migration/authored/project'/(name+'.nepld');data=p.read_bytes();assert b'\r' not in data and not data.startswith(b'\xef\xbb\xbf')
 md=(repo/(name+'.md')).read_text(encoding='utf-8');ast=ns['Parser'](data.decode(),ns['forms']).complete('Doc/Article');ns['audit_text'](ast);start=len(ns['pairs']);n=ns['normalize'](ast,True);nodes=list(ns['walk'](n))
 expected=[re.sub(r'`([^`]+)`',r'\1',re.sub(r'\[([^\]]+)\]\([^)]+\)',r'\1',x.replace('\n',''))) for x in md.strip().split('\n\n') if not x.startswith('#')]
 actual=[ns['plain'](x) for x in nodes if x['kind']=='Paragraph'];assert actual==expected,(name,actual,expected)
 links=[x['fields']['target']['fields'] for x in nodes if x['kind']=='Link'];targets=[f.get('path',f.get('uri')) for f in links]
 assert targets==re.findall(r'\[[^\]]+\]\(([^)]+)\)',md)
 codes=[x['fields']['text'] for x in nodes if x['kind']=='InlineCode'];assert codes==re.findall(r'`([^`]+)`',md)
 item=dict(name=name,source_sha256=hashlib.sha256((repo/(name+'.md')).read_bytes()).hexdigest(),draft_sha256=hashlib.sha256(data).hexdigest(),paragraphs=len(actual),codes=len(codes),links=len(links),ruby=len(ns['pairs'])-start,kinds=dict(collections.Counter(x['kind'] for x in nodes)),production_runtime_executed=False)
 results.append(item);(r/(name+'-paragraphs.txt')).write_text('\n\n'.join(actual)+'\n',encoding='utf-8',newline='\n')
 (r/(name+'-readings.txt')).write_text('\n'.join(a+' / '+b for a,b in sorted(set(ns['pairs'][start:])))+'\n',encoding='utf-8',newline='\n')
(r/'checks.json').write_text(json.dumps(results,ensure_ascii=False,indent=2)+'\n',encoding='utf-8');print(json.dumps(results,ensure_ascii=False,indent=2))
