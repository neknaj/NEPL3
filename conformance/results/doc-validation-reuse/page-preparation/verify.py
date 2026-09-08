from pathlib import Path
import gzip, hashlib, io, json, re, subprocess, tarfile

R = Path(__file__).resolve().parent
REPO = 'C:/projects/NEPL3-doc-page-preparation-reuse'
rows = json.loads((R / 'sources.json').read_text(encoding='utf-8'))
for row in rows:
    archive = subprocess.check_output(['git', '-C', REPO, 'archive', row['commit']])
    assert hashlib.sha256(archive).hexdigest() == row['archive_sha256']
    expected_rows = []
    with tarfile.open(fileobj=io.BytesIO(archive)) as t:
        for member in t:
            if not member.isfile():
                continue
            raw = t.extractfile(member).read()
            expected_rows.append(dict(path=member.name, bytes=len(raw), sha256=hashlib.sha256(raw).hexdigest()))
            actual = (R / row['name'] / member.name).read_bytes()
            if member.name == 'crates/languages/doc/html/tests/local.rs':
                assert actual == raw + b'\n' + (R / 'probe.rs').read_bytes()
            else:
                assert actual == raw, (row['name'], member.name)
    stored = json.loads(gzip.decompress((R / ('workspace-' + row['name'] + '.json.gz')).read_bytes()))
    assert expected_rows == stored

equal = None
for mode in ('before', 'after'):
    for target in ('native', 'wasi'):
        stem = mode + '-' + target
        metadata = json.loads((R / (stem + '.json')).read_text(encoding='utf-8'))
        assert metadata['exit_code'] == 0
        assert str(R / ('target-' + mode)) in metadata['command']
        s = (R / (stem + '.log')).read_text(encoding='utf-8')
        count = sum(map(int, re.findall(r'test result: ok\. (\d+) passed', s)))
        assert count == (29 if mode == 'before' else 30)
        lines = [line[line.index('REVIEW_EQUAL'):] for line in s.splitlines() if 'REVIEW_EQUAL' in line]
        if equal is None:
            equal = lines
        assert len(lines) == 14 and lines == equal
for target in ('native', 'wasi'):
    s = (R / ('tools-' + target + '.log')).read_text(encoding='utf-8')
    assert 'test result: ok. 2 passed; 0 failed' in s

baseline = (R / 'phase-baseline.log').read_text(encoding='utf-8')
observed = (R / 'phase-observed.log').read_text(encoding='utf-8')
usage = lambda s: next(line for line in s.splitlines() if line.startswith('REVIEW_OUTPUT_USAGE'))
assert usage(baseline) == usage(observed)
assert baseline.endswith('nepl3-tools: Stopped(WorkLimit)\n')
assert observed.endswith('nepl3-tools: Stopped(WorkLimit)\n')
assert 'REVIEW_PHASE 5 60502279 192490815 3120454' in observed
assert not any(line.startswith('REVIEW_PHASE 6 ') for line in observed.splitlines())
source = (R / 'phase-input/03-reader.nepld').read_bytes()
assert len(source) == 69199
assert hashlib.sha256(source).hexdigest() == '162fe9ec7faf8c671ef3769fc27342209577db38084d0201220b7871c430bab3'
print('verified: 3430 tracked files per snapshot; four test runs; 14 equal outcomes; tools native/WASI; phase Usage equality')
