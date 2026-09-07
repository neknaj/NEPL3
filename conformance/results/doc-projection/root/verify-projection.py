import json, os, subprocess
from pathlib import Path

root = Path(__file__).resolve().parents[1]
env = dict(os.environ)
env['CARGO_TARGET_DIR'] = str(root / 'target')
env['CARGO_TARGET_WASM32_WASIP2_RUNNER'] = 'wasmtime run'
commands = [
    ['cargo', 'test', '--locked', '-p', 'nepl3-tools', '--test', 'doc', 'projection::', '--', '--test-threads=1'],
    ['cargo', 'test', '--locked', '-p', 'nepl3-tools', '--test', 'doc', 'projection::', '--target', 'wasm32-wasip2', '--', '--test-threads=1'],
    ['cargo', 'fmt', '--all', '--', '--check'],
    ['cargo', 'clippy', '--workspace', '--all-targets', '--locked', '--', '-D', 'warnings'],
    ['python', 'tools/migration/contract.py'],
    ['python', '-m', 'unittest', 'discover', '-s', 'tools/migration', '-p', 'test_contract.py'],
    ['cargo', 'run', '--locked', '-p', 'nepl3-tools', '--', 'check'],
]
results = []
for i, command in enumerate(commands):
    path = root / '.tmp' / f'projection-check-{i}.log'
    with path.open('wb') as log:
        result = subprocess.run(command, cwd=root, env=env, stdout=log, stderr=subprocess.STDOUT)
    results.append(dict(command=command, exit_code=result.returncode, log=path.name))
    (root / '.tmp/projection-checks.json').write_text(json.dumps(results, indent=2)+'\n', encoding='utf-8', newline='\n')
    print(i, result.returncode, flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
