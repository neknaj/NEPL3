from pathlib import Path
import json,posixpath,sys
D=Path(__file__).resolve().parent;sys.path.insert(0,str(D.parent/'review-markup-html/python'));from playwright.sync_api import sync_playwright
rows=[]
for line in (D/'probe-native.log').read_text(encoding='utf-8').splitlines():
 if not line.startswith('ROUTE\t'):continue
 _,source,target,fragment,href=line.split('\t');expected=posixpath.relpath(target,posixpath.dirname(source)or'.')+('#'+fragment if fragment else'');assert href==expected,(source,target,href,expected);rows.append(dict(source=source,target=target,fragment=fragment,href=href))
assert len(rows)==392
bases=['https://example.invalid/NEPL3/','https://example.invalid/other/nested/base/','https://example.invalid/','file:///C:/offline/site/'];results={}
with sync_playwright()as p:
 for name,exe in [('chromium','C:/Users/bem/AppData/Local/ms-playwright/chromium-1234/chrome-win64/chrome.exe'),('firefox','C:/Users/bem/AppData/Local/ms-playwright/firefox-1538/firefox/firefox.exe')]:
  browser=getattr(p,name).launch(executable_path=exe,headless=True);page=browser.new_page();values=page.evaluate('''({rows,bases})=>bases.flatMap(base=>rows.map(row=>({actual:new URL(row.href,base+row.source).href,expected:base+row.target+(row.fragment?'#'+row.fragment:''),source:row.source,target:row.target,base})))''',dict(rows=rows,bases=bases));assert all(v['actual']==v['expected']for v in values),[v for v in values if v['actual']!=v['expected']];results[name]={'browser':browser.version,'url_resolutions':len(values),'bases':bases,'all_expected':True};browser.close()
(D/'routes.json').write_text(json.dumps(rows,indent=2),encoding='utf-8');(D/'browser-url.json').write_text(json.dumps(results,indent=2),encoding='utf-8');print('392 exact relative paths, 1568 URL resolutions per browser, 4 bases including /NEPL3/ and offline file')
