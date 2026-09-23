"""Cargo runner: execute the same component under native codegen and Pulley64."""
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import uuid

from records import Backend, Completed, Failure, Identity, Outcome, Run, Success, TimedOut, representation, success


def normalized(output: bytes) -> bytes:
    # Only libtest's final elapsed duration is nondeterministic.
    return re.sub(rb"(?m)(test result: .*; finished in )\d+\.\d+s$", rb"\1<TIME>s", output)


def test_count(stdout: bytes) -> int:
    matches = list(re.finditer(rb"(?m)^test result: ok\. (\d+) passed; (\d+) failed;", stdout))
    if len(matches) != 1 or matches[0].group(2) != b"0":
        raise ValueError("missing/ambiguous libtest result")
    count = int(matches[0].group(1))
    if len(re.findall(rb"(?m)^test .+ \.\.\. ok$", stdout)) != count:
        raise ValueError("test count mismatch")
    return count


def compare(artifact: Path, arguments: list[str], destination: Path) -> int:
    destination = destination / uuid.uuid4().hex
    destination.mkdir(parents=True, exist_ok=True)
    identity: Identity | None = None
    runs: list[Run] = []
    completed: list[Completed] = []
    outputs: list[tuple[bytes, bytes]] = []
    outcome: Outcome
    try:
        binary = shutil.which("wasmtime")
        if not binary:
            raise ValueError("Wasmtime unavailable")
        version = subprocess.check_output([binary, "--version"], timeout=10).decode().strip()
        if not version.startswith("wasmtime 44.0.1 "):
            raise ValueError("unqualified Wasmtime version")
        digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
        identity = Identity(digest, version, hashlib.sha256(Path(binary).read_bytes()).hexdigest())
        for backend in Backend:
            options = [] if backend is Backend.NATIVE else ["--target", "pulley64"]
            if hashlib.sha256(artifact.read_bytes()).hexdigest() != digest:
                raise ValueError("component changed between executions")
            command = [binary, "run", *options, str(artifact), *arguments]
            try:
                result = subprocess.run(command, capture_output=True, timeout=180)
            except subprocess.TimeoutExpired as error:
                _ = (destination / f"{backend.value}.stdout.log").write_bytes(error.stdout or b"")
                _ = (destination / f"{backend.value}.stderr.log").write_bytes(error.stderr or b"")
                runs.append(TimedOut(backend, tuple(command)))
                raise
            _ = (destination / f"{backend.value}.stdout.log").write_bytes(result.stdout)
            _ = (destination / f"{backend.value}.stderr.log").write_bytes(result.stderr)
            run = Completed(backend, tuple(command), result.returncode,
                            hashlib.sha256(result.stdout).hexdigest(), hashlib.sha256(result.stderr).hexdigest())
            runs.append(run)
            completed.append(run)
            result.check_returncode()
            if hashlib.sha256(artifact.read_bytes()).hexdigest() != digest:
                raise ValueError("component changed during execution")
            outputs.append((normalized(result.stdout), result.stderr))
        if outputs[0] != outputs[1]:
            raise ValueError("backend output mismatch")
        count = test_count(outputs[0][0])
        # Cargo invokes empty library test binaries too. Record them as empty,
        # not proof of tests; the CI aggregate requires a positive total.
        outcome = Success(identity, (completed[0], completed[1]), count)
        _ = sys.stdout.buffer.write(outputs[0][0])
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        outcome = Failure(identity, tuple(runs), str(error))
    _ = (destination / "result.json").write_text(json.dumps(representation(tuple(arguments), outcome), indent=2) + "\n", encoding="utf-8")
    return 1 if isinstance(outcome, Failure) else 0


def check_results(destination: Path) -> None:
    records = [directory / "result.json" for directory in destination.iterdir()]
    total = 0
    for path in records:
        record = success(path.read_text(encoding="utf-8"))
        outputs: list[list[bytes]] = []
        for run in record.runs:
            streams: list[bytes] = []
            for stream, expected in [("stdout", run.stdout_sha256), ("stderr", run.stderr_sha256)]:
                data = (path.parent / f"{run.backend.value}.{stream}.log").read_bytes()
                if hashlib.sha256(data).hexdigest() != expected:
                    raise ValueError("log identity mismatch")
                streams.append(normalized(data) if stream == "stdout" else data)
            outputs.append(streams)
        count = test_count(outputs[0][0])
        if outputs[0] != outputs[1] or count != record.passed:
            raise ValueError("backend/log count mismatch")
        total += count
    if not records or total == 0:
        raise ValueError("missing or empty current execution evidence")
    print(f"{len(records)} component invocations; {total} tests per backend")


def main() -> None:
    destination = Path(os.environ.get("NEPL3_WASM_EVIDENCE", "target/pulley-evidence"))
    try:
        if sys.argv[1:] == ["--begin"]:
            destination.mkdir(parents=True, exist_ok=True)
            session = uuid.uuid4().hex
            (destination / session).mkdir()
            _ = (destination / "active.txt").write_text(session, encoding="utf-8")
        else:
            session = (destination / "active.txt").read_text(encoding="utf-8")
            if not re.fullmatch("[0-9a-f]{32}", session):
                raise ValueError("invalid execution session")
            destination = destination / session
            if sys.argv[1:] == ["--check-results"]:
                check_results(destination)
            elif len(sys.argv) >= 2:
                sys.exit(compare(Path(sys.argv[1]).resolve(), sys.argv[2:], destination))
            else:
                raise ValueError("usage: compare.py --begin | component.wasm [args] | --check-results")
    except (OSError, ValueError, TypeError, AttributeError, KeyError, subprocess.SubprocessError) as error:
        sys.exit(str(error))


if __name__ == "__main__":
    main()
