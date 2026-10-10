"""Typed oracle and strict independent checks for the bounded browser slice."""
from collections.abc import Iterable, Mapping
from dataclasses import dataclass
import hashlib
from pathlib import Path
import re
import sys
from typing import Literal

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue, array, decode, integer, object_value, string

type Engine = Literal["chromium", "firefox", "webkit"]
ENGINES: tuple[Engine, ...] = ("chromium", "firefox", "webkit")
ERRORS = {1: "Bounds", 2: "ScalarBoundary", 3: "LineTerminator", 4: "Position", 5: "SnapshotMismatch"}

@dataclass(frozen=True, slots=True)
class Fixture:
    id: int
    text: str
    length: int
    lines: int

@dataclass(frozen=True, slots=True)
class Case:
    id: str
    operation: Literal["position", "offset"]
    fixture: int
    mismatch: int
    encoding: int
    args: tuple[str, ...]
    expected: Mapping[str, int | str]

@dataclass(frozen=True, slots=True)
class Oracle:
    fixtures: tuple[Fixture, ...]
    cases: tuple[Case, ...]

    def representation(self) -> JsonValue:
        fixtures: list[JsonValue] = [{"id": f.id, "text": f.text, "length": f.length, "lines": f.lines} for f in self.fixtures]
        cases: list[JsonValue] = [{"id": c.id, "operation": c.operation, "fixture": c.fixture,
                                 "mismatch": c.mismatch, "encoding": c.encoding, "args": list(c.args),
                                 "expected": dict(c.expected)} for c in self.cases]
        return {"version": 1, "fixtures": fixtures, "cases": cases}


def digest(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def unsigned(value: JsonValue) -> int:
    value = string(value)
    if not re.fullmatch(r"0|[1-9][0-9]*", value):
        raise ValueError("noncanonical integer")
    number = int(value)
    if number >= 1 << 64:
        raise ValueError("integer overflow")
    return number


def fields(value: JsonValue, expected: Iterable[str]) -> Mapping[str, JsonValue]:
    result = object_value(value)
    if set(result) != set(expected):
        raise ValueError("unexpected fields")
    return result


def oracle(value: JsonValue) -> Oracle:
    data = fields(value, ("version", "fixtures", "cases"))
    if integer(data["version"]) != 1:
        raise ValueError("oracle version")
    items = array(data["fixtures"])
    if len(items) != 3:
        raise ValueError("fixture count")
    fixtures: list[Fixture] = []
    for index, item in enumerate(items):
        f = fields(item, ("id", "text", "length", "lines"))
        fixture = Fixture(integer(f["id"]), string(f["text"]), integer(f["length"]), integer(f["lines"]))
        if fixture.id != index or len(fixture.text.encode()) != fixture.length or fixture.lines < 1:
            raise ValueError("fixture identity or size")
        fixtures.append(fixture)
    cases: list[Case] = []
    identifiers: set[str] = set()
    for item in array(data["cases"]):
        c = fields(item, ("id", "operation", "fixture", "mismatch", "encoding", "args", "expected"))
        name = string(c["id"])
        if not name or name in identifiers:
            raise ValueError("case identity")
        identifiers.add(name)
        fixture, mismatch, encoding = integer(c["fixture"]), integer(c["mismatch"]), integer(c["encoding"])
        if not 0 <= fixture <= 2 or not 0 <= mismatch <= 3 or not 0 <= encoding <= 2:
            raise ValueError("case selector")
        operation = string(c["operation"])
        if operation not in ("position", "offset"):
            raise ValueError("case operation")
        args = tuple(string(a) for a in array(c["args"]))
        if len(args) != (1 if operation == "position" else 2):
            raise ValueError("case arguments")
        for argument in args:
            _ = unsigned(argument)
        expected: dict[str, int | str] = {}
        result = object_value(c["expected"])
        if set(result) == {"error"}:
            error = string(result["error"])
            if error not in ERRORS.values():
                raise ValueError("expected error")
            expected["error"] = error
        else:
            result = fields(c["expected"], ("line", "character") if operation == "position" else ("offset",))
            for key, raw in result.items():
                number = integer(raw)
                if not 0 <= number < 1 << 32:
                    raise ValueError("expected position")
                expected[key] = number
        cases.append(Case(name, operation, fixture, mismatch, encoding, args, expected))
    if not cases:
        raise ValueError("empty cases")
    return Oracle(tuple(fixtures), tuple(cases))


def observed(operation: str, raw: JsonValue) -> Mapping[str, int | str]:
    number = unsigned(raw)
    if number & (1 << 63):
        code = number ^ (1 << 63)
        if code not in ERRORS:
            raise ValueError("unknown error tag")
        return {"error": ERRORS[code]}
    if operation == "position":
        return {"line": number >> 32, "character": number & 0xffffffff}
    if operation != "offset":
        raise ValueError("unknown operation")
    return {"offset": number}


def verify(value: JsonValue, engine: Engine, wasm_hash: str, expected: Oracle) -> int:
    data = fields(value, ("version", "engine", "engine_version", "playwright", "wasm_sha256", "abi", "fixtures", "results"))
    if integer(data["version"]) != 1 or data["engine"] != engine or engine not in ENGINES:
        raise ValueError("result version or engine identity")
    if not string(data["engine_version"]):
        raise ValueError("engine version")
    if data["playwright"] != "1.62.0" or data["wasm_sha256"] != wasm_hash:
        raise ValueError("tool or binary identity")
    if integer(data["abi"]) != 1:
        raise ValueError("ABI mismatch")
    fixtures = array(data["fixtures"])
    if len(fixtures) != len(expected.fixtures):
        raise ValueError("fixture count")
    for item, f in zip(fixtures, expected.fixtures, strict=True):
        actual = fields(item, ("id", "length", "lines", "bytes"))
        if (integer(actual["id"]) != f.id or unsigned(actual["length"]) != f.length
                or unsigned(actual["lines"]) != f.lines
                or tuple(unsigned(b) for b in array(actual["bytes"])) != tuple(f.text.encode())):
            raise ValueError("fixture observations")
    rows = array(data["results"])
    if len(rows) != len(expected.cases):
        raise ValueError("missing cases")
    for item, case in zip(rows, expected.cases, strict=True):
        row = fields(item, ("id", "value"))
        if row["id"] != case.id:
            raise ValueError("missing, reordered, duplicate or unknown case")
        if observed(case.operation, row["value"]) != case.expected:
            raise ValueError(f"unexpected result: {case.id}")
    return len(rows)


def read_json(path: Path) -> JsonValue:
    return decode(path.read_text(encoding="utf-8"), reject_duplicates=True, reject_nonfinite=True)
