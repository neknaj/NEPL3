from pathlib import Path
import json,hashlib,subprocess
r=Path(__file__).parent;repo=r.parents[1];sha=lambda b:hashlib.sha256(b).hexdigest()
prior=Path('C:/projects/NEPL3-doc-guide-refresh/.tmp/review-project-guides')
assert sha((prior/'manifest.json').read_bytes())=='c58b7c0d9fac30cc1829d21291dad5aefbde6da21aaf10a4b4c8ec4c45f28a2a'
for item in json.loads((prior/'manifest.json').read_text(encoding='utf-8'))['files']:
 assert sha((prior/item['path']).read_bytes())==item['sha256']
def blob(commit,path):return subprocess.check_output(['git','-C',str(repo),'show',commit+':'+path])
path='doc/migration/authored/project/CONTRIBUTING.nepld'
assert blob('efa4847',path)==blob('dcb1e2f',path)
(r/'delta.diff').write_bytes(subprocess.check_output(['git','-C',str(repo),'diff','2b48d5','dcb1e2f']))
(r/'review.md').write_text('''# Independent SECURITY / CONTRIBUTING content review

PASS dcb1e2f04820a24c5761cf53a6d14ecb1b8e23e1, base2b48d5. Four changed paths only: SECURITY.md, CONTRIBUTING.md and their project nepld drafts. Entire original and draft prose, each Ruby reading, links and inline code were read directly against fixed AGENTS.md, authoring.md, README.md and implementation-status.json. Contributor role correction matches explicit user instruction and current AGENTS: root implements, independent subagent reviews, explicitly authorized Doc parallel authoring is a restricted exception.

SECURITY introductory correction follows README common-runtime implementation and status foundation-runtime/in-progress tasks. It retains no supported runtime release and fixes on main; it does not claim product completion, released security support, all tests passed or implemented defenses. Remaining warning, private-report URL, required reproduction context and distinction between designed versus implemented defenses remain intact. This review checks consistency with repository statements, not remote advisory settings or independently published release inventory.

Full independent subset structural parser (reviewer's previous check.py, not author's converter) ran again on fixed original/draft Git blobs. Exact title and paragraph text including spaces agrees after projecting only Ruby; ordered link labels/targets and code bytes agree. SECURITY:3 paragraphs7 sentences4 links0 code37 Ruby. CONTRIBUTING:5 paragraphs13 sentences6 links3 code91 Ruby. Manual reading confirmed context readings and kanji-only bases/okurigana, no unresolved kanji body, one natural sentence per literal/prefix unit. Prefix is used for code/link sentences; ordinary prose uses literal. Neither original has translations or extra semantic notes; no artificial Anno/parallel was inserted.

CONTRIBUTING draft is byte-identical to reviewed efa4847. SECURITY differs from that reviewed draft in the first sentence only, matching its canonical original correction. Previous independent manifest c58b7c0d9fac30cc1829d21291dad5aefbde6da21aaf10a4b4c8ec4c45f28a2a and all payloads rehashed. Author manifest880ea953543644eaa7027f9eefb7aad1cc8166a700823f209c2efe769dab775b and every actual payload also rehashed; these were supporting provenance, not substituted for independent full-content checking.

No content blocking finding. No production files edited. This scope does not execute production parser/lower/HTML, link registry resolution, browser or Pages deployment. Earlier isolated export NeedsResolution is not promoted to success. Markdown remains canonical; no T21 completion claimed. Relative targets are retained verbatim for later registry connection.
''',encoding='utf-8')
files=[p for p in r.rglob('*') if p.is_file() and p.name!='manifest.json']
entries=[{'path':p.relative_to(r).as_posix(),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())}for p in sorted(files)]
(r/'manifest.json').write_text(json.dumps({'files':entries},indent=2)+'\n',encoding='utf-8')
print(sha((r/'manifest.json').read_bytes()),len(entries))
