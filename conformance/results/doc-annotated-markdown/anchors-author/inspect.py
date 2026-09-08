from pathlib import Path
import re
import json
import html

p = Path(__file__).resolve().parent
s = (p / 'github-page.html').read_text(encoding='utf-8')
rows = []
for level, heading, attrs in re.findall(r'<h([1-6])\b[^>]*class="heading-element"[^>]*>(.*?)</h\1><a ([^>]+)>', s):
    attrs = dict(re.findall(r'([a-z-]+)="([^"]*)"', attrs))
    rows.append({'level': int(level), 'heading': html.unescape(re.sub('<[^>]+>', '', heading)), 'id': attrs.get('id'), 'href': attrs.get('href')})
(p / 'observed-anchors.json').write_text(json.dumps(rows, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps(rows, ensure_ascii=True))
