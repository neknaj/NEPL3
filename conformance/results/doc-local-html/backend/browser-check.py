from pathlib import Path
import os,subprocess,json,time
D=Path('C:/projects/NEPL3-runtime/.tmp/review-doc-html-current');W=D/'workspace';E=os.environ.copy();E['PYTHONPATH']=str(D.parent/'review-markup-html/python')
cmd=['python',str(W/'tools/audit/doc_html/browser.py'),'--corpus',str(D/'tools-native.log'),'--css',str(W/'assets/doc-html.css'),'--chromium','C:/Users/bem/AppData/Local/ms-playwright/chromium-1234/chrome-win64/chrome.exe','--firefox','C:/Users/bem/AppData/Local/ms-playwright/firefox-1538/firefox/firefox.exe','--output',str(D/'browser-result.json')]
cmd[cmd.index('--css')+1]=str(W/'crates/languages/doc/html/assets/doc.css')
p=subprocess.run(cmd,cwd=W,env=E,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180);(D/'browser-raw-corpus.log').write_bytes(p.stdout);(D/'browser-raw-corpus-run.json').write_text(json.dumps({'command':cmd,'exit':p.returncode,'deadline':180},indent=2),encoding='utf-8');print(p.returncode,p.stdout.decode('utf-8',errors='replace'))
lines=[]
for line in (D/'tools-native.log').read_text(encoding='utf-8').splitlines():
 if 'DOC_HTML_CASE ' in line:lines.append(line[line.index('DOC_HTML_CASE '):])
(D/'normalized-corpus.log').write_text('\n'.join(lines)+'\n',encoding='utf-8',newline='\n');cmd[cmd.index('--corpus')+1]=str(D/'normalized-corpus.log')
p=subprocess.run(cmd,cwd=W,env=E,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,timeout=180);(D/'browser-normalized-corpus.log').write_bytes(p.stdout);(D/'browser-normalized-corpus-run.json').write_text(json.dumps({'command':cmd,'exit':p.returncode,'deadline':180,'normalization':'Removed Cargo test status prefix preceding first exact DOC_HTML_CASE marker; HTML hex unchanged'},indent=2),encoding='utf-8');print('normalized',p.returncode,p.stdout.decode('utf-8',errors='replace'))
