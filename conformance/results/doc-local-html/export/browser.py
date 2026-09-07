from pathlib import Path
import subprocess,os,json,importlib.util,sys
D=Path(__file__).resolve().parent;W=D/'workspace';old=D.parent/'review-doc-html-current';env=os.environ.copy();env['PYTHONPATH']=str(D.parent/'review-markup-html/python');sys.path.insert(0,env['PYTHONPATH']);script=W/'tools/audit/doc_html/browser.py'
spec=importlib.util.spec_from_file_location('fixed_browser',script);mod=importlib.util.module_from_spec(spec);spec.loader.exec_module(mod)
raw=(old/'tools-native.log').read_bytes();expected=mod.extract_cases((old/'normalized-corpus.log').read_bytes());assert mod.extract_cases(raw)==expected
lines=(old/'normalized-corpus.log').read_text(encoding='utf-8').splitlines();cases={
 'missing':'\n'.join(lines[1:]),'duplicate':'\n'.join(lines+[lines[0]]),'arbitrary_prefix':'\n'.join(['noise '+lines[0]]+lines[1:]),'invalid_test_prefix':'\n'.join(['test x/name ... '+lines[0]]+lines[1:]),'missing_dots':'\n'.join(['test x '+lines[0]]+lines[1:]),'bad_hex':'\n'.join(['DOC_HTML_CASE ruby xx']+lines[1:]),'bad_utf8':'\n'.join(['DOC_HTML_CASE ruby ff']+lines[1:]),'extra_field':'\n'.join([lines[0]+' extra']+lines[1:]),'unknown_case':'\n'.join(lines+['DOC_HTML_CASE other 41']), 'missing_payload':'\n'.join(['DOC_HTML_CASE ruby']+lines[1:])}
results=[]
for name,value in cases.items():
 try:mod.extract_cases(value.encode());raise AssertionError(name+' accepted')
 except (ValueError,UnicodeError)as e:results.append({'case':name,'rejected':type(e).__name__})
for prefix in ['', '\ufeff']:
 assert mod.extract_cases((prefix+'\n'.join(lines)).encode('utf-8'))==expected
(D/'browser-extraction.json').write_text(json.dumps({'raw_unchanged_equal':True,'malformed':results,'plain_and_bom':True},indent=2),encoding='utf-8')
cmd=['python',str(script),'--corpus',str(old/'tools-native.log'),'--css',str(W/'crates/languages/doc/html/assets/doc.css'),'--chromium','C:/Users/bem/AppData/Local/ms-playwright/chromium-1234/chrome-win64/chrome.exe','--firefox','C:/Users/bem/AppData/Local/ms-playwright/firefox-1538/firefox/firefox.exe','--output',str(D/'browser-result.json')]
p=subprocess.run(cmd,cwd=W,env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180);(D/'browser-raw-fixed.log').write_bytes(p.stdout);(D/'browser-raw-fixed-run.json').write_text(json.dumps({'command':cmd,'exit':p.returncode,'deadline':180},indent=2),encoding='utf-8');assert p.returncode==0;print(p.stdout.decode())
