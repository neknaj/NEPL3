from pathlib import Path
import io, subprocess, tarfile

R = Path(__file__).resolve().parent
W = R / 'phase'
assert not W.exists(), 'Preserve prior evidence; use a fresh replay directory.'
raw = subprocess.check_output(['git', '-C', 'C:/projects/NEPL3-doc-page-preparation-reuse', 'archive', '8b6088e0ff818fae5d8176bc2690dbba42ed43f5'])
W.mkdir()
with tarfile.open(fileobj=io.BytesIO(raw)) as t:
    t.extractall(W, filter='data')
p = W / 'tools/src/doc/export/pages.rs'
assert p.read_bytes() == (R / 'phase-tools-original.rs').read_bytes()
p.write_bytes((R / 'phase-tools-baseline.rs').read_bytes())
print('Run phase-run.py baseline, phase-instrument.py, then phase-run.py observed.')
