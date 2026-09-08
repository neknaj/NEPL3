from pathlib import Path
r=Path(__file__).resolve().parent;old=r.parent/'review-tree-selection'
s=(old/'setup.py').read_text().replace('NEPL3-tree-selection-uniqueness','NEPL3-tree-head-source-locality').replace('b41cb03','6f84a51').replace('2681cc0','e7124f9').replace('tree-selection-probe','tree-head-probe');(r/'setup.py').write_text(s,encoding='utf-8',newline='\n')
s=(old/'run.py').read_text().replace('tree-selection-probe','tree-head-probe');(r/'run.py').write_text(s,encoding='utf-8',newline='\n')
builder=(old/'extra.rs').read_text().split('    for n in [')[0]
(r/'extra.rs').write_text(builder+(r/'cases.rs').read_text(),encoding='utf-8',newline='\n')
