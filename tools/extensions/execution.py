"""Execute consumer checks and retain their typed outcomes and output digests."""

from collections.abc import Mapping, Sequence
from dataclasses import dataclass
from enum import Enum
import hashlib
from pathlib import Path
import subprocess

from tools.serialization.json import JsonValue


@dataclass(frozen=True, slots=True)
class Exited:
    code: int


@dataclass(frozen=True, slots=True)
class Interrupted:
    error: str


@dataclass(frozen=True, slots=True)
class Command:
    arguments: tuple[str, ...]
    outcome: Exited | Interrupted
    log: str
    digest: str
    stderr: str
    stderr_digest: str

    def representation(self) -> dict[str, JsonValue]:
        value: dict[str, JsonValue] = {"command": list(self.arguments)}
        match self.outcome:
            case Exited(code):
                value["exit_code"] = code
            case Interrupted(error):
                value["exit_code"] = None
                value["error"] = error
        value.update({"log": self.log, "sha256": self.digest,
                      "stderr": self.stderr, "stderr_sha256": self.stderr_digest})
        return value


class Scope(Enum):
    DIRECT = "external workspace with public path dependencies; not an independent release or process provider"
    DISTRIBUTION = "external consumer of an extracted foundation source workspace; publication and process provider remain unverified"


@dataclass(frozen=True, slots=True)
class Record:
    scope: Scope
    commit: str
    foundation: Mapping[str, str]
    commands: tuple[Command, ...]
    completed: bool
    unchanged: bool
    consumer: Mapping[str, str] | None

    def representation(self) -> dict[str, JsonValue]:
        value: dict[str, JsonValue] = {
            "scope": self.scope.value, "commit": self.commit,
            "foundation": dict(self.foundation),
            "commands": [command.representation() for command in self.commands],
            "result": "passed" if self.completed and self.unchanged else "failed",
        }
        if self.consumer is not None:
            value["consumer"] = dict(self.consumer)
        value["foundation_unchanged"] = self.unchanged
        return value


def output_bytes(value: bytes | str | None) -> bytes:
    if value is None:
        return b""
    return value.encode("utf-8") if isinstance(value, str) else value


class Execution:
    def __init__(self, output: Path) -> None:
        self.output: Path = output
        self.commands: list[Command] = []

    def record(self, arguments: Sequence[str], outcome: Exited | Interrupted, name: str,
               stdout: bytes, stderr: bytes) -> None:
        _ = (self.output / name).write_bytes(stdout)
        error_log = name + ".stderr.log"
        _ = (self.output / error_log).write_bytes(stderr)
        self.commands.append(Command(tuple(arguments), outcome, name, hashlib.sha256(stdout).hexdigest(),
                                     error_log, hashlib.sha256(stderr).hexdigest()))

    def run(self, command: Sequence[str], cwd: Path, name: str) -> bytes:
        try:
            result = subprocess.run(command, cwd=cwd, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                    timeout=600, check=False)
        except (subprocess.TimeoutExpired, OSError) as failure:
            stdout = output_bytes(failure.stdout) if isinstance(failure, subprocess.TimeoutExpired) else b""
            stderr = output_bytes(failure.stderr) if isinstance(failure, subprocess.TimeoutExpired) else b""
            log = stdout + ("\n" + str(failure) + "\n").encode("utf-8")
            self.record(command, Interrupted(type(failure).__name__), name, log, stderr)
            raise
        self.record(command, Exited(result.returncode), name, result.stdout, result.stderr or b"")
        if result.returncode:
            raise RuntimeError(f"{name}: exit {result.returncode}; see {self.output}")
        return result.stdout
