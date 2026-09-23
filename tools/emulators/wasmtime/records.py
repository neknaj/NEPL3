"""Immutable execution records and their external JSON representation."""

from dataclasses import dataclass
from enum import Enum
from pathlib import Path
import re
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue, array, decode, integer, object_value, string


class Backend(Enum):
    NATIVE = "native"
    PULLEY = "pulley64"


@dataclass(frozen=True, slots=True)
class Identity:
    artifact_sha256: str
    wasmtime: str
    runner_sha256: str


@dataclass(frozen=True, slots=True)
class Completed:
    backend: Backend
    command: tuple[str, ...]
    exit_code: int
    stdout_sha256: str
    stderr_sha256: str


@dataclass(frozen=True, slots=True)
class TimedOut:
    backend: Backend
    command: tuple[str, ...]


type Run = Completed | TimedOut


@dataclass(frozen=True, slots=True)
class Success:
    identity: Identity
    runs: tuple[Completed, Completed]
    passed: int


@dataclass(frozen=True, slots=True)
class Failure:
    identity: Identity | None
    runs: tuple[Run, ...]
    error: str


type Outcome = Success | Failure


def representation(arguments: tuple[str, ...], outcome: Outcome) -> JsonValue:
    runs: list[JsonValue] = []
    for run in outcome.runs:
        record: dict[str, JsonValue] = {"backend": run.backend.value, "command": list(run.command)}
        if isinstance(run, TimedOut):
            record["timeout"] = True
        else:
            record.update(exit_code=run.exit_code, stdout_sha256=run.stdout_sha256, stderr_sha256=run.stderr_sha256)
        runs.append(record)
    result: dict[str, JsonValue] = {
        "arguments": list(arguments),
        "result": "failed" if isinstance(outcome, Failure) else "passed" if outcome.passed else "empty",
        "runs": runs,
    }
    if outcome.identity is not None:
        result.update(artifact_sha256=outcome.identity.artifact_sha256,
                      wasmtime=outcome.identity.wasmtime, runner_sha256=outcome.identity.runner_sha256)
    if isinstance(outcome, Failure):
        result["error"] = outcome.error
    else:
        result["passed"] = outcome.passed
    return result


def success(source: str) -> Success:
    """Validate an aggregate input before its records reach execution logic."""
    value = object_value(decode(source))
    passed = integer(value.get("passed"))
    state = value.get("result")
    version = string(value.get("wasmtime"))
    artifact = string(value.get("artifact_sha256"))
    runner = string(value.get("runner_sha256"))
    _ = tuple(string(item) for item in array(value.get("arguments")))
    if (state not in ("passed", "empty") or passed < 0 or (state == "empty") != (passed == 0)
            or not version.startswith("wasmtime 44.0.1 ")
            or not re.fullmatch("[0-9a-f]{64}", artifact) or not re.fullmatch("[0-9a-f]{64}", runner)):
        raise ValueError("malformed or failed execution record")
    items = array(value.get("runs"))
    if len(items) != 2:
        raise ValueError("missing backend execution")
    runs: list[Completed] = []
    for item, backend in zip(items, Backend):
        record = object_value(item)
        command = tuple(string(arg) for arg in array(record.get("command")))
        code = integer(record.get("exit_code"))
        if record.get("backend") != backend.value or code != 0 or not command:
            raise ValueError("missing backend execution")
        runs.append(Completed(backend, command, code, string(record.get("stdout_sha256")), string(record.get("stderr_sha256"))))
    return Success(Identity(artifact, version, runner), (runs[0], runs[1]), passed)
