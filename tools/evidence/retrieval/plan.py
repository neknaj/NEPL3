"""Read the status-owned transport view; Rust remains the acceptance validator."""
from collections import Counter
from pathlib import Path
import re
import subprocess
from collections.abc import Mapping

from tools.evidence.archive import checked, directory, read_file
from tools.evidence.retrieval.pin import Pin
from tools.evidence.retrieval.restore import Request, MAX_REQUESTS, path_parts
from tools.serialization.json import JsonValue, array, decode, object_value, string


def read(root: Path, name: str, limit: int) -> Mapping[str, JsonValue]:
    parts = path_parts(name)
    path = root.joinpath(*parts)
    directory(path.parent)
    return object_value(decode(read_file(path, limit), reject_duplicates=True, reject_nonfinite=True))


def identity(root: Path, commit: str | None = None) -> Mapping[str, JsonValue]:
    arguments = ['cargo', 'run', '--manifest-path', str(root / 'Cargo.toml'), '--locked',
                 '-p', 'nepl3-tools', '--', 'evidence', 'identity']
    if commit is not None:
        checked(re.fullmatch(r'[0-9a-f]{40}', commit), 'selected identity commit')
        arguments += ['--commit', commit]
    # Build/run the current checkout's tool, not a binary compiled for another
    # worktree or historical source. The Git-object reader never lazy-fetches.
    try:
        result = subprocess.run(arguments, cwd=root, capture_output=True, timeout=180, check=False)
    except (OSError, subprocess.TimeoutExpired):
        raise ValueError('current evidence identity tool unavailable or timed out') from None
    checked(result.returncode == 0 and 0 < len(result.stdout) <= 65536, 'evidence identity tool failed')
    snapshot = object_value(decode(result.stdout, reject_duplicates=True, reject_nonfinite=True))
    checked(set(snapshot) == {'design_revision', 'identity'}, 'identity snapshot fields')
    _ = string(snapshot['design_revision'])
    value = object_value(snapshot['identity'])
    checked(set(value) == {'profile', 'source_sha256', 'spec_sha256'} and
            value['profile'] == 'nepl3.repository-inputs/1', 'identity profile')
    for key in ('source_sha256', 'spec_sha256'):
        checked(re.fullmatch(r'[0-9a-f]{64}', string(value[key])), 'identity digest')
    return snapshot


def requests(root: Path, snapshot: Mapping[str, JsonValue]) -> list[Request]:
    status = read(root, 'implementation-status.json', 1024 * 1024)
    tasks = array(status.get('tasks'))
    states = array(status.get('acceptance'))
    checked(len(tasks) <= 256 and len(states) <= 256, 'status owner count')
    owners: Counter[str] = Counter()
    for state in [*tasks, *states]:
        paths = array(object_value(state).get('evidence'))
        checked(len(paths) <= MAX_REQUESTS, 'owner evidence count')
        owners.update(string(path) for path in paths)
    result: list[Request] = []
    for state in states:
        owner = object_value(state)
        state_name = string(owner.get('status'))
        paths = array(owner.get('evidence'))
        if state_name in ('not-run', 'blocked'):
            checked(not paths, 'unexecuted acceptance cannot own artifacts')
            continue
        checked(state_name in ('passed', 'failed') and bool(paths), 'invalid executed acceptance state')
        acceptance_id = string(owner.get('id'))
        for value in paths:
            path = string(value)
            checked(path.startswith('conformance/results/') and path.endswith('.json') and owners[path] == 1,
                    'artifact evidence must have one status owner')
            record = read(root, path, 65536)
            checked(record.get('schema') == 'nepl3.acceptance-evidence/1' and
                    record.get('acceptance_id') == acceptance_id and record.get('result') == state_name,
                    'artifact evidence identity or state mismatch')
            checked(record.get('design_revision') == snapshot['design_revision'] and
                    record.get('identity') == snapshot['identity'], 'stale evidence input identity')
            runs = array(record.get('runs'))
            checked(0 < len(runs) <= MAX_REQUESTS, 'artifact evidence run count')
            for value in runs:
                run = object_value(value)
                checked(run.get('kind') in ('command', 'review'), 'unsupported artifact run kind')
                pin = Pin.read(run['artifact']) if 'artifact' in run else None
                result.append(Request(string(run.get('log')), string(run.get('log_sha256')), pin))
                checked(len(result) <= MAX_REQUESTS, 'aggregate artifact request count')
    return result
