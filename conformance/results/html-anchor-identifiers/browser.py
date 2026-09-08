from pathlib import Path
import sys,json,threading,http.server,functools,urllib.parse,html.parser
sys.path.insert(0,'C:/projects/NEPL3-runtime/.tmp/review-markup-html/python')
from playwright.sync_api import sync_playwright
r=Path(__file__).resolve().parent;data=json.loads((r/'probe-native.log').read_text(encoding='utf-8'));site=r/'site';site.mkdir(exist_ok=True)
class Parse(html.parser.HTMLParser):
 def __init__(self):super().__init__();self.ids=[];self.hrefs=[]
 def handle_starttag(self,tag,attrs):
  for k,v in attrs:
   if k=='id':self.ids.append(v)
   if k=='href':self.hrefs.append(v)
paths=[]
for i,c in enumerate(data['cases']):
 p=Parse();p.feed(c['html']);assert p.ids==[c['id']];encoded=urllib.parse.quote(c['id'],safe='-._~');prefix=['','target.html','../to/target.html'][c['branch']];assert p.hrefs==[prefix+'#'+encoded],(c,p.hrefs)
 for base in ['NEPL3','acceptance/nested']:
  source=f'{base}/case{i}/'+('from/input.html' if c['branch']==2 else 'index.html');target=source if c['branch']==0 else f'{base}/case{i}/'+('to/target.html' if c['branch']==2 else 'target.html')
  for path,body in [(target,data['cases'][i-c['branch']]['html']),(source,c['html'])]:
   q=site/path;q.parent.mkdir(parents=True,exist_ok=True);q.write_text('<!doctype html><html><head><meta charset="utf-8"></head><body>'+body+'</body></html>',encoding='utf-8',newline='\n')
  paths.append((source,target,c['id'],encoded))
class Quiet(http.server.SimpleHTTPRequestHandler):
 def log_message(self,*args):pass
server=http.server.ThreadingHTTPServer(('127.0.0.1',0),functools.partial(Quiet,directory=str(site)));threading.Thread(target=server.serve_forever,daemon=True).start();origin=f'http://127.0.0.1:{server.server_port}/';results=[]
with sync_playwright() as pw:
 for name,exe in [('chromium','C:/Users/bem/AppData/Local/ms-playwright/chromium-1234/chrome-win64/chrome.exe'),('firefox','C:/Users/bem/AppData/Local/ms-playwright/firefox-1538/firefox/firefox.exe')]:
  browser=getattr(pw,name).launch(executable_path=exe,headless=True);context=browser.new_context(java_script_enabled=False,viewport={'width':800,'height':420});page=context.new_page();n=0
  for source,target,id,encoded in paths:
   for root in [origin,site.as_uri()+'/']:
    page.goto(root+source);page.get_by_text('follow',exact=True).click();page.wait_for_url(root+target+'#'+encoded)
    actual=page.locator(':target').get_attribute('id');assert actual==id,(name,source,id,actual,page.url)
    assert page.locator(':target').inner_text()=='destination';assert page.evaluate('window.scrollY')>0,(name,id)
    assert page.locator('script').count()==0;n+=1
  results.append({'browser':name,'version':browser.version,'javascript':False,'navigations':n});print(name,n,flush=True);browser.close()
server.shutdown();(r/'browser-result.json').write_text(json.dumps({'independent_html_parser_cases':len(data['cases']),'results':results},indent=2)+'\n',encoding='utf-8')
