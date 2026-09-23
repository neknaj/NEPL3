"""Collect command evidence; never decide acceptance or write a review."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import sys
from typing import Literal

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))
from tools.serialization.json import JsonValue, decode
from tools.evidence.records import (
    Command, Environment, Exited, File, Incomplete, Interrupted, Report, Result,
    report, specification as specification,
)


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def read_json(path: Path) -> JsonValue:
    return decode(path.read_bytes(), reject_duplicates=True, reject_nonfinite=True)


def git(root: Path, *args: str) -> bytes:
    return subprocess.run(['git', '-C', str(root), *args], check=True, capture_output=True).stdout


def run(root: Path, spec_path: Path, output: Path) -> bool:
    root = root.resolve(strict=True)
    spec_raw = spec_path.read_bytes()
    spec = specification(read_json(spec_path))
    revision = git(root, 'rev-parse', 'HEAD').decode().strip()
    # A reviewed source revision must exist. No full source snapshot is copied.
    if git(root, 'diff', 'HEAD', '--binary'):
        raise ValueError('commit tracked changes before recording evidence')
    if git(root, 'ls-files', '--others', '--exclude-standard', '-z'):
        raise ValueError('untracked files require review before recording evidence')
    for command in spec.commands:
        cwd = (root / command.cwd).resolve(strict=True)
        if not cwd.is_relative_to(root) or not cwd.is_dir():
            raise ValueError('cwd escapes repository')
        if cwd.is_relative_to((root / 'conformance/results').resolve()):
            raise ValueError('historical evidence is not an execution directory')
    output.mkdir(parents=True, exist_ok=False)
    _ = (output / 'spec.json').write_bytes(spec_raw)
    rows: list[Result] = []
    for command in spec.commands:
        argv = list(command.argv)
        if argv[0] == 'python':
            argv[0] = sys.executable
        executed = Command(command.id, tuple(argv), command.cwd, command.timeout_seconds)
        with (output / (command.id + '.stdout')).open('xb') as stdout, (output / (command.id + '.stderr')).open('xb') as stderr:
            outcome: Exited | Interrupted
            try:
                completed = subprocess.run(argv, cwd=root / command.cwd, stdout=stdout, stderr=stderr,
                                           timeout=command.timeout_seconds)
                outcome = Exited(completed.returncode)
            except subprocess.TimeoutExpired:
                outcome = Interrupted(Incomplete.UNKNOWN, 'timeout; descendant completion and final log bytes not established')
            except OSError:
                outcome = Interrupted(Incomplete.NOT_RUN, 'process could not start')
        rows.append(Result(executed, outcome))
        if isinstance(outcome, Interrupted):
            break
    files: list[File] = []
    for path in sorted(output.iterdir()):
        data = path.read_bytes()
        files.append(File(path.name, len(data), digest(data)))
    changed = bool(git(root, 'diff', 'HEAD', '--binary')) or git(root, 'rev-parse', 'HEAD').decode().strip() != revision
    result = Report(spec.scope, revision, digest(Path(__file__).read_bytes()), changed,
                    'tracked HEAD and diff before/after; ignored inputs and transient changes are not covered',
                    Environment(sys.version, sys.executable, platform.platform(), platform.machine()),
                    tuple(rows), tuple(files))
    _ = (output / 'manifest.json').write_text(json.dumps(result.representation(), indent=2, ensure_ascii=False) + '\n',
                                             encoding='utf-8', newline='\n')
    _ = verify(output)
    return not changed and len(rows) == len(spec.commands) and all(row.passed for row in rows)


def verify(output: Path) -> Report:
    value = report(read_json(output / 'manifest.json'))
    names: set[str] = set()
    for entry in value.files:
        name = entry.path
        if not re.fullmatch('[a-z0-9.-]+', name) or name in ('.', '..', 'manifest.json') or name in names:
            raise ValueError('invalid evidence path')
        names.add(name)
        path = output / name
        if path.is_symlink() or not path.is_file():
            raise ValueError('evidence must be regular data')
        data = path.read_bytes()
        if len(data) != entry.bytes or digest(data) != entry.sha256:
            raise ValueError('evidence hash mismatch')
    if {p.name for p in output.iterdir()} != names | {'manifest.json'}:
        raise ValueError('evidence file set changed')
    spec = specification(read_json(output / 'spec.json'))
    if value.scope != spec.scope:
        raise ValueError('scope mismatch')
    if not 0 < len(value.commands) <= len(spec.commands):
        raise ValueError('missing or extra command results')
    if len(value.commands) < len(spec.commands) and not isinstance(value.commands[-1].outcome, Interrupted):
        raise ValueError('unexplained missing results')
    for row, command in zip(value.commands, spec.commands):
        if (row.command.id != command.id or row.command.cwd != command.cwd
                or row.command.timeout_seconds != command.timeout_seconds):
            raise ValueError('command identity mismatch')
        expected = list(command.argv)
        if expected[0] == 'python':
            expected[0] = value.environment.python_executable
        if row.command.argv != tuple(expected):
            raise ValueError('command argv mismatch')
        if not {row.command.id + '.stdout', row.command.id + '.stderr'} <= names:
            raise ValueError('missing raw logs')
    return value


class Arguments(argparse.Namespace):
    operation: Literal['run', 'verify'] = 'verify'
    spec: Path = Path()
    output: Path = Path()


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest='operation', required=True)
    collect = sub.add_parser('run')
    _ = collect.add_argument('spec', type=Path)
    _ = collect.add_argument('output', type=Path)
    check = sub.add_parser('verify')
    _ = check.add_argument('output', type=Path)
    args = parser.parse_args(namespace=Arguments())
    try:
        if args.operation == 'verify':
            _ = verify(args.output)
            return 0
        return 0 if run(Path.cwd(), args.spec, args.output) else 1
    except (ValueError, OSError, KeyError, TypeError, subprocess.SubprocessError) as error:
        print(type(error).__name__ + ': evidence operation failed', file=sys.stderr)
        return 1


if __name__ == '__main__':
    sys.exit(main())
