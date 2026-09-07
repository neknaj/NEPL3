import sys
sys.path.insert(0, 'C:/projects/NEPL3-runtime/.tmp/review-markup-html/python')
from playwright.sync_api import sync_playwright
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from functools import partial
from pathlib import Path
from threading import Thread
import json

root = Path(__file__).resolve().parents[1]
site = root / '.tmp/migration-external'
output = root / '.tmp/external-browser'
output.mkdir(exist_ok=True)
server = ThreadingHTTPServer(('127.0.0.1', 0), partial(SimpleHTTPRequestHandler, directory=str(root / '.tmp')))
Thread(target=server.serve_forever, daemon=True).start()
results = []
try:
    with sync_playwright() as p:
        for name, executable in [
            ('chromium', 'C:/Users/bem/AppData/Local/ms-playwright/chromium-1234/chrome-win64/chrome.exe'),
            ('firefox', 'C:/Users/bem/AppData/Local/ms-playwright/firefox-1538/firefox/firefox.exe'),
        ]:
            browser = getattr(p, name).launch(executable_path=executable, headless=True)
            for mode, url in [('http-subpath', f'http://127.0.0.1:{server.server_port}/migration-external/index.html'),
                              ('file', (site / 'index.html').as_uri())]:
                context = browser.new_context(java_script_enabled=False, viewport={'width': 1200, 'height': 800})
                page = context.new_page()
                requests = []
                page.on('request', lambda request: requests.append(request.url))
                page.goto(url)
                assert page.locator('a').count() == 2
                assert page.locator('a[href="https://github.com/neknaj/NEPL3"]').inner_text() == 'NEPL3 repository'
                assert not any(u.startswith('https://') for u in requests), requests
                page.locator('a[href="reference/contract/index.html"]').click()
                page.wait_for_url('**/reference/contract/index.html')
                data = page.evaluate('''() => ({
                    title: document.title,
                    sections: document.querySelectorAll('section').length,
                    lists: document.querySelectorAll('ul').length,
                    items: document.querySelectorAll('li').length,
                    codes: document.querySelectorAll('code').length,
                    scripts: document.scripts.length,
                    styles: document.styleSheets.length,
                    background: getComputedStyle(document.querySelector('.nepl-doc')).backgroundColor,
                    overflow: document.documentElement.scrollWidth > innerWidth
                })''')
                assert (data['sections'], data['lists'], data['items'], data['codes']) == (6, 1, 6, 10), data
                assert data['scripts'] == 0 and data['styles'] == 1, data
                assert data['background'] == 'rgb(13, 17, 23)' and not data['overflow'], data
                page.screenshot(path=str(output / f'{name}-{mode}.png'), full_page=True)
                results.append(dict(engine=name, version=browser.version, mode=mode, result=data, requests=requests))
                context.close()
            browser.close()
    (output / 'result.json').write_text(json.dumps(results, ensure_ascii=False, indent=2)+'\n', encoding='utf-8', newline='\n')
    print('4 script-disabled browser navigation/stylesheet checks passed')
finally:
    server.shutdown()
