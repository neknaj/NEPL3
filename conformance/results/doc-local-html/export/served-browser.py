from pathlib import Path
from http.server import ThreadingHTTPServer,SimpleHTTPRequestHandler
from functools import partial
from threading import Thread
import sys,json,hashlib
D=Path(__file__).resolve().parent;sys.path.insert(0,str(D.parent/'review-markup-html/python'))
from playwright.sync_api import sync_playwright
server=ThreadingHTTPServer(('127.0.0.1',0),partial(SimpleHTTPRequestHandler,directory=str(D/'filesystem')));Thread(target=server.serve_forever,daemon=True).start();result={}
try:
 with sync_playwright()as p:
  for name,exe in [('chromium','C:/Users/bem/AppData/Local/ms-playwright/chromium-1234/chrome-win64/chrome.exe'),('firefox','C:/Users/bem/AppData/Local/ms-playwright/firefox-1538/firefox/firefox.exe')]:
   browser=getattr(p,name).launch(executable_path=exe,headless=True);ctx=browser.new_context(java_script_enabled=False,viewport={'width':1200,'height':800});page=ctx.new_page();errors=[];requests=[];page.on('pageerror',lambda e:errors.append(str(e)));page.on('request',lambda r:requests.append(r.url));response=page.goto(f'http://127.0.0.1:{server.server_port}/large/document.html');assert response.status==200
   data=page.evaluate('''() => ({ scripts:document.scripts.length,styles:document.styleSheets.length,sections:document.querySelectorAll('section').length,tables:document.querySelectorAll('table').length,ruby:document.querySelectorAll('.nepl-ruby').length,anno:document.querySelectorAll('.nepl-anno').length,rubyDisplay:getComputedStyle(document.querySelector('.nepl-ruby')).display,brokenLocalLinks:[...document.querySelectorAll('a[href^="#"]')].filter(a=>!document.getElementById(a.getAttribute('href').slice(1))).length,background:getComputedStyle(document.querySelector('.nepl-doc')).backgroundColor,text:document.querySelector('article').textContent})''');assert data['scripts']==0 and data['styles']==1 and data['brokenLocalLinks']==0;assert data['sections']==4 and data['tables']==1;assert data['ruby']>0 and data['anno']>0;assert data['rubyDisplay']=='inline-grid';assert not errors;assert all(x.startswith(f'http://127.0.0.1:{server.server_port}/large/')for x in requests)
   data['text_sha256']=hashlib.sha256(data.pop('text').encode()).hexdigest();data.update(browser=browser.version,errors=errors,requests=requests);result[name]=data;browser.close()
 assert result['chromium']['text_sha256']==result['firefox']['text_sha256']
 (D/'served-export-browser.json').write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8');print('Independent served actual export: both JS-disabled browsers, CSS loaded, all local links valid, text hashes equal')
finally:server.shutdown();server.server_close()
