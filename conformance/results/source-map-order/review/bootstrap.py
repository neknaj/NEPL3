from pathlib import Path
r=Path(__file__).resolve().parent
s=(r.parent/'review-revision-locality/setup.py').read_text()
s=s.replace('NEPL3-source-revision-locality','NEPL3-source-map-ordered-locality').replace('a1c72e0','4be6b34').replace('461d627','58d622c')
(r/'setup.py').write_text(s,encoding='utf-8')
(r/'run.py').write_bytes((r.parent/'review-revision-locality/run.py').read_bytes())
