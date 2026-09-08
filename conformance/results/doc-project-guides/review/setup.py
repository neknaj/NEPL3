from pathlib import Path
r=Path(__file__).parent
old=Path('C:/projects/NEPL3-doc-guide-refresh/.tmp/review-project-guides/check.py').read_text(encoding='utf-8')
old=old.replace('efa4847ce10d200d88f3b0a65300bc97dfba4a8c','dcb1e2f04820a24c5761cf53a6d14ecb1b8e23e1').replace('58e3e969a6d55ff2470b6a125229da29af45f985','2b48d5')
old=old.replace("paths=['AGENTS.md'","paths=['README.md','implementation-status.json','AGENTS.md'")
old=old.replace(".tmp/authoring-refresh",".tmp/authoring").replace('30305d26b4f44cb15fd2c2435fdb292c657b3267c5c4378c537b21807fdc39ba','880ea953543644eaa7027f9eefb7aad1cc8166a700823f209c2efe769dab775b')
(r/'check.py').write_text(old,encoding='utf-8',newline='\n')
