from pathlib import Path
r=Path(__file__).resolve().parent
s=(r.parent/'review-reader-failure/setup.py').read_text().replace('NEPL3-reader-owned-failure','NEPL3-tokenizer-owned-failure').replace('be6aa8b','ccafb1e').replace('0e0b365','be6aa8b');(r/'setup.py').write_text(s,encoding='utf-8')
s=(r.parent/'review-reader-failure/run.py').read_text().replace('review_owned::independent_','independent_review::independent_');(r/'run.py').write_text(s,encoding='utf-8')
