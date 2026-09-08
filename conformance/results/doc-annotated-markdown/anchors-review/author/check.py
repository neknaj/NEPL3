from pathlib import Path
import hashlib
import json
import re
import subprocess

out = Path(__file__).resolve().parent
root = out.parents[1]
plan = json.loads((out / 'mapping.json').read_text(encoding='utf-8'))
commit = plan['commit']
sources = []
for path, local in [(plan['canonical_path'], 'canonical.md'), (plan['draft_path'], 'draft.nepld')]:
    data = subprocess.check_output(['git', 'show', f'{commit}:{path}'], cwd=root)
    (out / local).write_bytes(data)
    sources.append({'path': path, 'payload': local, 'bytes': len(data), 'sha256': hashlib.sha256(data).hexdigest()})
md = (out / 'canonical.md').read_text(encoding='utf-8')
draft = (out / 'draft.nepld').read_text(encoding='utf-8')
headings = [(n, re.match(r'^(#+) (.*)$', line)) for n, line in enumerate(md.splitlines(), 1) if re.match(r'^#+ ', line)]
observed = json.loads((out / 'observed-anchors.json').read_text(encoding='utf-8'))
assert len(headings) == len(observed) == len(plan['mappings']) == 8
assert len({x['candidate_fragment'] for x in plan['mappings']}) == 8
assert len({x['section_id'] for x in plan['mappings'] if x['section_id'] is not None}) == 7
checked = []
for manual, (line, match), actual in zip(plan['mappings'], headings, observed):
    title = manual['original_heading']
    assert match.group(2) == title and len(match.group(1)) == manual['markdown_level']
    assert actual['heading'] == title and actual['level'] == manual['markdown_level']
    assert actual['href'] == '#' + manual['candidate_fragment']
    assert actual['id'] == 'user-content-' + manual['candidate_fragment']
    literal = manual['authored_title_literal']
    # Only these eight simple author-read heading literals, not a Doc parser.
    assert re.sub(r'\[([^/\[\]]+)/[^\[\]]+\]', r'\1', literal) == title
    prefix = 'article ja ' if manual['section_id'] is None else 'cons section ' + manual['section_id'] + ' '
    expected = prefix + json.dumps(literal, ensure_ascii=False)
    lines = [(n, text) for n, text in enumerate(draft.splitlines(), 1) if text.lstrip().startswith(expected)]
    assert len(lines) == 1
    draft_line, text = lines[0]
    if manual['section_id'] is not None:
        assert text.endswith(' body')
        assert len(text) - len(text.lstrip()) == (4 if manual['parent_section_id'] else 2)
    checked.append({'section_id': manual['section_id'], 'target_kind': manual['target_kind'], 'canonical_line': line, 'canonical_excerpt': md.splitlines()[line-1], 'draft_line': draft_line, 'draft_excerpt': text, 'observed': actual})
result = {'source_commit': commit, 'source_files': sources, 'rows': checked, 'counts': {'article_titles': 1, 'sections': 7, 'old_fragment_collisions': 0}, 'checks': {'exact_old_heading_order': True, 'literal_bases_equal_original_heading': True, 'manual_candidates_equal_actual_github_hrefs': True, 'browser_click': False, 'production_parser': False, 'new_renderer': False}}
(out / 'checked-mapping.json').write_text(json.dumps(result, ensure_ascii=False, indent=2) + '\n', encoding='utf-8', newline='\n')
print(json.dumps({'heading_count':len(checked),'source_files':sources},ensure_ascii=True))
