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


def normalized(output):
    # Only libtest's final elapsed duration is nondeterministic.
    return re.sub(rb"(?m)(test result: .*; finished in )\d+\.\d+s$", rb"\1<TIME>s", output)


def test_count(stdout):
    matches = re.findall(rb"(?m)^test result: ok\. (\d+) passed; (\d+) failed;", stdout)
    if len(matches) != 1 or matches[0][1] != b"0":
        raise ValueError("missing/ambiguous libtest result")
    count = int(matches[0][0])
    if len(re.findall(rb"(?m)^test .+ \.\.\. ok$", stdout)) != count:
        raise ValueError("test count mismatch")
    return count


def compare(artifact, arguments, destination):
    destination = destination / uuid.uuid4().hex
    destination.mkdir(parents=True, exist_ok=True)
    record = {"arguments": arguments, "result": "failed", "runs": []}
    outputs = []
    try:
        binary = shutil.which("wasmtime")
        if not binary:
            raise ValueError("Wasmtime unavailable")
        version = subprocess.check_output([binary, "--version"], timeout=10).decode().strip()
        if not version.startswith("wasmtime 44.0.1 "):
            raise ValueError("unqualified Wasmtime version")
        digest = hashlib.sha256(artifact.read_bytes()).hexdigest()
        record.update(artifact_sha256=digest, wasmtime=version,
                      runner_sha256=hashlib.sha256(Path(binary).read_bytes()).hexdigest())
        for backend, options in [("native", []), ("pulley64", ["--target", "pulley64"])]:
            if hashlib.sha256(artifact.read_bytes()).hexdigest() != digest:
                raise ValueError("component changed between executions")
            command = [binary, "run", *options, str(artifact), *arguments]
            try:
                result = subprocess.run(command, capture_output=True, timeout=180)
            except subprocess.TimeoutExpired as error:
                (destination / f"{backend}.stdout.log").write_bytes(error.stdout or b"")
                (destination / f"{backend}.stderr.log").write_bytes(error.stderr or b"")
                record["runs"].append({"backend": backend, "command": command, "timeout": True})
                raise
            (destination / f"{backend}.stdout.log").write_bytes(result.stdout)
            (destination / f"{backend}.stderr.log").write_bytes(result.stderr)
            record["runs"].append({"backend": backend, "command": command,
                                   "exit_code": result.returncode,
                                   "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
                                   "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()})
            result.check_returncode()
            if hashlib.sha256(artifact.read_bytes()).hexdigest() != digest:
                raise ValueError("component changed during execution")
            outputs.append((normalized(result.stdout), result.stderr))
        if outputs[0] != outputs[1]:
            raise ValueError("backend output mismatch")
        count = test_count(outputs[0][0])
        # Cargo invokes empty library test binaries too. Record them as empty,
        # not proof of tests; the CI aggregate requires a positive total.
        record.update(result="passed" if count else "empty", passed=count)
        sys.stdout.buffer.write(outputs[0][0])
    except (ValueError, OSError, subprocess.SubprocessError) as error:
        record["error"] = str(error)
    (destination / "result.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    return 1 if record["result"] == "failed" else 0


def check_results(destination):
    records = [directory / "result.json" for directory in destination.iterdir()]
    total = 0
    for path in records:
        r = json.loads(path.read_text(encoding="utf-8"))
        if (not isinstance(r, dict)
                or r.get("result") not in ["passed", "empty"] or type(r.get("passed")) is not int
                or r["passed"] < 0 or (r["result"] == "empty") != (r["passed"] == 0)
                or not isinstance(r.get("runs"), list) or len(r["runs"]) != 2
                or not isinstance(r.get("arguments"), list)
                or any(not isinstance(a, str) for a in r["arguments"])
                or not isinstance(r.get("wasmtime"), str) or not r["wasmtime"].startswith("wasmtime 44.0.1 ")
                or any(not isinstance(r.get(key), str) or not re.fullmatch("[0-9a-f]{64}", r[key])
                       for key in ["artifact_sha256", "runner_sha256"])):
            raise ValueError("malformed or failed execution record")
        outputs = []
        for run, backend in zip(r["runs"], ["native", "pulley64"]):
            if (not isinstance(run, dict) or run.get("backend") != backend
                    or type(run.get("exit_code")) is not int or run["exit_code"] != 0
                    or not isinstance(run.get("command"), list) or not run["command"]
                    or any(not isinstance(a, str) for a in run["command"])):
                raise ValueError("missing backend execution")
            streams = []
            for stream in ["stdout", "stderr"]:
                data = (path.parent / f"{backend}.{stream}.log").read_bytes()
                if hashlib.sha256(data).hexdigest() != run.get(f"{stream}_sha256"):
                    raise ValueError("log identity mismatch")
                streams.append(normalized(data) if stream == "stdout" else data)
            outputs.append(streams)
        count = test_count(outputs[0][0])
        if outputs[0] != outputs[1] or count != r["passed"]:
            raise ValueError("backend/log count mismatch")
        total += count
    if not records or total == 0:
        raise ValueError("missing or empty current execution evidence")
    print(f"{len(records)} component invocations; {total} tests per backend")


if __name__ == "__main__":
    destination = Path(os.environ.get("NEPL3_WASM_EVIDENCE", "target/pulley-evidence"))
    try:
        if sys.argv[1:] == ["--begin"]:
            destination.mkdir(parents=True, exist_ok=True)
            session = uuid.uuid4().hex
            (destination / session).mkdir()
            (destination / "active.txt").write_text(session, encoding="utf-8")
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
