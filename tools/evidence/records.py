"""Validated command-evidence records; JSON is confined to this boundary."""

from dataclasses import dataclass
from enum import Enum
from pathlib import Path
import re

from tools.serialization.json import JsonValue, array, integer, object_value, string


@dataclass(frozen=True, slots=True)
class Command:
    id: str
    argv: tuple[str, ...]
    cwd: str
    timeout_seconds: int

    def representation(self) -> dict[str, JsonValue]:
        return dict(id=self.id, argv=list(self.argv), cwd=self.cwd, timeout_seconds=self.timeout_seconds)


@dataclass(frozen=True, slots=True)
class Specification:
    scope: str
    commands: tuple[Command, ...]


def specification(value: JsonValue) -> Specification:
    record = object_value(value)
    if set(record) != {'version', 'scope', 'commands'} or integer(record['version']) != 1:
        raise ValueError('invalid specification')
    scope = string(record['scope'])
    if not scope.strip():
        raise ValueError('scope required')
    commands = array(record['commands'])
    if not commands:
        raise ValueError('commands required')
    names: set[str] = set()
    result: list[Command] = []
    for value in commands:
        command = object_value(value)
        if set(command) != {'id', 'argv', 'cwd', 'timeout_seconds'}:
            raise ValueError('invalid command')
        name = string(command['id'])
        if not re.fullmatch('[a-z0-9-]{1,80}', name) or name in names:
            raise ValueError('invalid command ID')
        names.add(name)
        argv = tuple(string(arg) for arg in array(command['argv']))
        if not argv or any(not arg or '\0' in arg for arg in argv):
            raise ValueError('invalid argv')
        if any('conformance/results/' in arg.replace('\\', '/') for arg in argv):
            raise ValueError('do not execute historical evidence')
        cwd = string(command['cwd'])
        if Path(cwd).is_absolute() or '..' in Path(cwd).parts:
            raise ValueError('invalid cwd')
        normalized = '/'.join(Path(cwd).parts).replace('\\', '/').lower()
        if normalized == 'conformance/results' or normalized.startswith('conformance/results/'):
            raise ValueError('historical evidence is not an execution directory')
        timeout = integer(command['timeout_seconds'])
        if not 0 < timeout <= 3600:
            raise ValueError('invalid timeout')
        result.append(Command(name, argv, cwd, timeout))
    return Specification(scope, tuple(result))


class Incomplete(Enum):
    UNKNOWN = 'unknown'
    NOT_RUN = 'not-run'


@dataclass(frozen=True, slots=True)
class Exited:
    code: int


@dataclass(frozen=True, slots=True)
class Interrupted:
    state: Incomplete
    reason: str


@dataclass(frozen=True, slots=True)
class Result:
    command: Command
    outcome: Exited | Interrupted

    @property
    def passed(self) -> bool:
        return isinstance(self.outcome, Exited) and self.outcome.code == 0

    def representation(self) -> dict[str, JsonValue]:
        value = self.command.representation()
        if isinstance(self.outcome, Exited):
            value.update(outcome='passed' if self.passed else 'failed', exit_code=self.outcome.code)
        else:
            value.update(outcome=self.outcome.state.value, exit_code=None, reason=self.outcome.reason)
        return value


@dataclass(frozen=True, slots=True)
class File:
    path: str
    bytes: int
    sha256: str

    def representation(self) -> dict[str, JsonValue]:
        return dict(path=self.path, bytes=self.bytes, sha256=self.sha256)


@dataclass(frozen=True, slots=True)
class Environment:
    python: str
    python_executable: str
    platform: str
    machine: str

    def representation(self) -> dict[str, JsonValue]:
        return dict(python=self.python, python_executable=self.python_executable,
                    platform=self.platform, machine=self.machine)


@dataclass(frozen=True, slots=True)
class Report:
    scope: str
    source_revision: str
    runner_sha256: str
    source_changed: bool
    source_check: str
    environment: Environment
    commands: tuple[Result, ...]
    files: tuple[File, ...]

    def representation(self) -> dict[str, JsonValue]:
        return dict(version=1, kind='command-evidence', scope=self.scope, source_revision=self.source_revision,
                    runner_sha256=self.runner_sha256, source_changed=self.source_changed,
                    source_check=self.source_check, environment=self.environment.representation(),
                    commands=[row.representation() for row in self.commands],
                    files=[file.representation() for file in self.files], acceptance_decision=False)


def report(value: JsonValue) -> Report:
    record = object_value(value)
    if (integer(record['version']) != 1 or record['kind'] != 'command-evidence'
            or record['acceptance_decision'] is not False):
        raise ValueError('wrong evidence kind')
    changed = record['source_changed']
    if not isinstance(changed, bool):
        raise ValueError('source change state missing')
    revision = string(record['source_revision'])
    if not re.fullmatch('[0-9a-f]{40}', revision):
        raise ValueError('source revision missing')
    environment = object_value(record['environment'])
    rows: list[Result] = []
    for value in array(record['commands']):
        row = object_value(value)
        command = Command(string(row['id']), tuple(string(arg) for arg in array(row['argv'])),
                          string(row['cwd']), integer(row['timeout_seconds']))
        state = string(row['outcome'])
        outcome: Exited | Interrupted
        if state in ('passed', 'failed'):
            code = integer(row['exit_code'])
            if (code == 0) != (state == 'passed'):
                raise ValueError('contradictory outcome')
            outcome = Exited(code)
        else:
            if row['exit_code'] is not None:
                raise ValueError('unexecuted exit code')
            outcome = Interrupted(Incomplete(state), string(row['reason']))
        rows.append(Result(command, outcome))
    files: list[File] = []
    for value in array(record['files']):
        file = object_value(value)
        files.append(File(string(file['path']), integer(file['bytes']), string(file['sha256'])))
    return Report(string(record['scope']), revision, string(record['runner_sha256']), changed,
                  string(record['source_check']), Environment(string(environment['python']),
                  string(environment['python_executable']), string(environment['platform']),
                  string(environment['machine'])), tuple(rows), tuple(files))
