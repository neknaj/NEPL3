import sys
sys.path.insert(0, 'C:/projects/NEPL3-runtime/.tmp/review-markup-html/python')
from playwright.sync_api import sync_playwright
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from functools import partial
from pathlib import Path
from threading import Thread
import json

root = Path('C:/projects/NEPL3-runtime/.tmp')
server = ThreadingHTTPServer(('127.0.0.1', 0), partial(SimpleHTTPRequestHandler, directory=str(root)))
Thread(target=server.serve_forever, daemon=True).start()
results = {}
try:
    with sync_playwright() as p:
        for name, executable in [
            ('chromium', 'C:/Users/bem/AppData/Local/ms-playwright/chromium-1234/chrome-win64/chrome.exe'),
            ('firefox', 'C:/Users/bem/AppData/Local/ms-playwright/firefox-1538/firefox/firefox.exe'),
        ]:
            browser = getattr(p, name).launch(executable_path=executable, headless=True)
            context = browser.new_context(java_script_enabled=False, viewport={'width':1200,'height':800})
            page = context.new_page()
            errors = []
            page.on('pageerror', lambda error: errors.append(str(error)))
            response = page.goto(f'http://127.0.0.1:{server.server_port}/linear-combination-export/document.html')
            data = page.evaluate('''() => ({
              ruby:document.querySelectorAll('.nepl-ruby').length,
              anno:document.querySelectorAll('.nepl-anno').length,
              sections:document.querySelectorAll('section').length,
              tables:document.querySelectorAll('table').length,
              scripts:document.scripts.length,
              rubyDisplay:getComputedStyle(document.querySelector('.nepl-ruby')).display,
              background:getComputedStyle(document.querySelector('.nepl-doc')).backgroundColor,
              styles:document.styleSheets.length,
              horizontalOverflow:document.documentElement.scrollWidth > innerWidth
            })''')
            data.update(status=response.status, browser=browser.version, errors=errors)
            page.screenshot(path=str(root/'export-browser'/f'{name}.png'), full_page=True)
            results[name] = data
            browser.close()
    (root/'export-browser'/'result.json').write_text(json.dumps(results, indent=2)+'\n', encoding='utf-8')
    for data in results.values():
        assert data['status'] == 200 and not data['errors']
        assert (data['ruby'], data['anno'], data['sections'], data['tables']) == (76,51,4,1), data
        assert data['scripts'] == 0 and data['styles'] == 1
        assert data['rubyDisplay'] == 'inline-grid' and data['background'] == 'rgb(13, 17, 23)'
    print(json.dumps(results, indent=2))
finally:
    server.shutdown()
