"""Inventory historical Python evidence without executing it.

Classification is triage based on content and path, not a claim that every
historical probe was independently revalidated. Uncertain entries stay visible.
"""
import argparse
import ast
from collections import Counter
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
from typing import Literal

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from tools.serialization.json import JsonValue

type Role = Literal['source-snapshot', 'evidence-sealing', 'command-runner', 'test-or-reproducer', 'needs-content-review']


@dataclass(frozen=True, slots=True)
class Entry:
    path: str
    git_blob: str
    size: int
    sha256: str
    roles: tuple[Role, ...]
    definitions: tuple[str, ...]
    purpose: str | None
    source_copy: bool
    duplicate_of: str | None

    def representation(self) -> JsonValue:
        return {
            'path': self.path, 'git_blob': self.git_blob, 'bytes': self.size,
            'sha256': self.sha256, 'roles': list(self.roles),
            'definitions': list(self.definitions), 'purpose': self.purpose,
            'production': False,
            'reuse_candidates': [r for r in self.roles if r in ('evidence-sealing', 'command-runner')],
            'one_off_harness': 'source copy' if self.source_copy else 'not established',
            'duplicate_of': self.duplicate_of,
            'future_policy': 'reference Git source' if self.source_copy else
                'shared runner/seal; move substantive probes to managed tests; justify exceptional reproducer',
        }


@dataclass(frozen=True, slots=True)
class Inventory:
    revision: str
    entries: tuple[Entry, ...]

    def counts(self) -> JsonValue:
        roles = Counter(role for row in self.entries for role in row.roles)
        return {
            'files': len(self.entries), 'bytes': sum(row.size for row in self.entries),
            'duplicate_copies': sum(row.duplicate_of is not None for row in self.entries),
            'roles': {role: count for role, count in roles.items()},
        }

    def representation(self) -> JsonValue:
        return {
            'version': 1, 'source_revision': self.revision,
            'scope': 'tracked historical Python and Python fixtures',
            'preservation': 'retain historical bytes; do not execute from results',
            'assessment': 'content/path triage; mixed roles and reuse candidates are not proof of one-off use or correctness',
            'counts': self.counts(), 'files': [row.representation() for row in self.entries],
        }


def git(*args: str, input: bytes | None = None) -> bytes:
    return subprocess.run(['git', *args], input=input, check=True, capture_output=True).stdout


def classify(path: str, blob: str, data: bytes, duplicate_of: str | None) -> Entry:
    """Classify one blob by content and path; uncertain roles remain explicit."""
    text = data.decode('utf-8-sig')
    syntax: ast.Module | None = None
    try:
        syntax = ast.parse(text)
    except SyntaxError:
        pass
    definitions = tuple(sorted({
        node.name for node in ast.walk(syntax)
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
    })) if syntax is not None else ()
    source_copy = any(part in Path(path).parts for part in ('source', 'sources', 'snapshot', 'context'))
    sealing = any(s in text for s in ('sha256(', 'hashlib.sha256', 'SHA256')) and any(
        s in text for s in ('manifest', 'files.json', 'sealed'))
    executes = any(s in text for s in ('subprocess.run', 'subprocess.Popen', 'check_call', 'check_output'))
    probes = any(s in text for s in ('unittest', 'assert ', 'assertEqual', 'mock.patch', 'with patch('))
    roles: list[Role] = []
    if source_copy:
        roles.append('source-snapshot')
    if sealing:
        roles.append('evidence-sealing')
    if executes:
        roles.append('command-runner')
    if probes:
        roles.append('test-or-reproducer')
    if not roles:
        roles.append('needs-content-review')
    return Entry(
        path, blob, len(data), hashlib.sha256(data).hexdigest(), tuple(roles),
        definitions, ast.get_docstring(syntax) if syntax is not None else None,
        source_copy, duplicate_of,
    )


def inventory(revision: str) -> Inventory:
    if not re.fullmatch('[0-9a-f]{40}', revision):
        raise ValueError('full commit required')
    tracked = git('ls-tree', '-r', '--name-only', '-z', revision, 'conformance/results').decode('utf-8').split('\0')[:-1]
    paths = [path for path in tracked if path.endswith(('.py', '.py.fixture'))]
    if any('\n' in path or '\r' in path for path in paths):
        raise ValueError('unsupported batch path')
    raw = git('cat-file', '--batch', input=''.join(revision + ':' + path + '\n' for path in paths).encode('utf-8'))
    offset = 0
    rows: list[Entry] = []
    seen: dict[str, str] = {}
    for path in paths:
        end = raw.index(b'\n', offset)
        header = raw[offset:end].split()
        if len(header) != 3 or header[1] != b'blob':
            raise ValueError('expected Git blob')
        size = int(header[2])
        data = raw[end + 1:end + 1 + size]
        offset = end + 2 + size
        sha = hashlib.sha256(data).hexdigest()
        rows.append(classify(path, header[0].decode(), data, seen.get(sha)))
        _ = seen.setdefault(sha, path)
    return Inventory(revision, tuple(rows))


class Arguments(argparse.Namespace):
    revision: str = ''
    output: Path = Path()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument('revision')
    _ = parser.add_argument('output', type=Path)
    args = parser.parse_args(namespace=Arguments())
    result = inventory(args.revision)
    with args.output.open('x', encoding='utf-8', newline='\n') as output:
        json.dump(result.representation(), output, ensure_ascii=False, indent=2)
        _ = output.write('\n')
    print(json.dumps(result.counts()))


if __name__ == '__main__':
    main()
