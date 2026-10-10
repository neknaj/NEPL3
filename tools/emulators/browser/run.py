"""Execute the source-position Wasm slice in three supervised browser processes."""
import argparse
import base64
import importlib.metadata
import json
import os
from pathlib import Path
import subprocess
import sys
import time
from collections.abc import Mapping, Sequence

import checks
from checks import ENGINES, Engine, digest, oracle, read_json, verify
from processes import Process, cleanup, remember, snapshot

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))
from tools.serialization.json import JsonValue, decode, object_value, string

SCRIPT = Path(__file__).resolve()
ORACLE = Path("conformance/inputs/browser/source-position.json")
MANIFEST = Path("conformance/targets/browser/Cargo.toml")
EVALUATE = """async ({bytes, cases, fixtures}) => {
    const array = Uint8Array.from(atob(bytes), c => c.charCodeAt(0));
    const module = await WebAssembly.compile(array);
    if (WebAssembly.Module.imports(module).length !== 0) throw Error('unexpected imports');
    const instance = await WebAssembly.instantiate(module, {});
    const e = instance.exports;
    for (const name of ['abi_version', 'fixture_count', 'source_length', 'source_lines', 'source_byte', 'source_position', 'source_offset']) {
        if (typeof e[name] !== 'function') throw Error('missing export ' + name);
    }
    if (e.abi_version() !== 1 || e.fixture_count() !== fixtures.length) throw Error('ABI mismatch');
    const u64 = value => {
        if (typeof value !== 'bigint') throw Error('not an i64 result');
        return BigInt.asUintN(64, value).toString();
    };
    const results = cases.map(c => {
        const args = c.args.map(x => BigInt(x));
        const result = c.operation === 'position'
            ? e.source_position(c.fixture, c.mismatch, c.encoding, args[0])
            : e.source_offset(c.fixture, c.mismatch, c.encoding, args[0], args[1]);
        return {id: c.id, value: u64(result)};
    });
    return JSON.stringify({abi: e.abi_version(), fixtures: fixtures.map(f => ({id: f.id,
        length: u64(e.source_length(f.id)), lines: u64(e.source_lines(f.id)),
        bytes: Array.from(new TextEncoder().encode(f.text), (_, at) => u64(e.source_byte(f.id, BigInt(at))))})), results});
}"""


def write_json(path: Path, value: JsonValue) -> None:
    with path.open("x", encoding="utf-8") as stream:
        json.dump(value, stream, ensure_ascii=False, indent=2)
        _ = stream.write("\n")


def supervise(argv: Sequence[str], directory: Path, name: str, timeout: float,
              env: Mapping[str, str] | None = None, cwd: Path | None = None) -> None:
    """A hung synchronous Wasm call cannot defeat this parent process deadline."""
    if timeout <= 0 or sys.platform != "linux":
        raise ValueError("positive timeout and Linux process tracking required")
    with (directory / f"{name}.stdout").open("xb") as out, (directory / f"{name}.stderr").open("xb") as err:
        process = subprocess.Popen(argv, stdout=out, stderr=err, env=env, cwd=cwd, start_new_session=True)
        root = snapshot().get(process.pid)
        known: dict[int, Process] = {} if root is None else {root.pid: root}
        deadline = time.monotonic() + timeout
        expired = False
        try:
            while process.poll() is None:
                remember(known)
                if time.monotonic() >= deadline:
                    expired = True
                    break
                time.sleep(0.02)
        finally:
            try:
                cleanup(known)
            finally:
                if process.poll() is None:
                    process.kill()
                code = process.wait(timeout=5)
        if expired:
            write_json(directory / f"{name}.status.json", {"argv": list(argv), "status": "timeout"})
            raise RuntimeError(f"{name}: deadline exceeded")
    write_json(directory / f"{name}.status.json", {"argv": list(argv), "status": "exited", "exit_code": code})
    if code != 0:
        raise RuntimeError(f"{name}: exit {code}")


def clean_revision(repository: Path) -> str:
    def git(*args: str) -> str:
        return subprocess.check_output(["git", "-C", str(repository), *args], text=True).strip()
    if git("status", "--porcelain", "--untracked-files=all"):
        raise ValueError("source must be committed and clean")
    return git("rev-parse", "HEAD")


def worker(engine: Engine, wasm: Path, expected_path: Path) -> None:
    from playwright.sync_api import sync_playwright
    expected = oracle(read_json(expected_path))
    version = importlib.metadata.version("playwright")
    if version != "1.62.0":
        raise ValueError("unpinned Playwright")
    payload = wasm.read_bytes()
    with sync_playwright() as playwright:
        browser_type = {"chromium": playwright.chromium, "firefox": playwright.firefox, "webkit": playwright.webkit}[engine]
        browser = browser_type.launch()
        try:
            page = browser.new_page()
            arguments = dict(object_value(expected.representation()))
            arguments["bytes"] = base64.b64encode(payload).decode("ascii")
            raw: object = page.evaluate(EVALUATE, arguments)  # pyright: ignore[reportAny]
            if not isinstance(raw, str):
                raise ValueError("browser did not return serialized observations")
            result = object_value(decode(raw, reject_duplicates=True, reject_nonfinite=True))
            record: dict[str, JsonValue] = {"version": 1, "engine": engine, "engine_version": browser.version,
                      "playwright": version, "wasm_sha256": digest(payload), **result}
            # Preserve observations even when independent comparison rejects them.
            print(json.dumps(record, ensure_ascii=False), flush=True)
            _ = verify(record, engine, digest(payload), expected)
        finally:
            browser.close()


def run(repository: Path, output: Path, timeout: float) -> JsonValue:
    repository = repository.resolve()
    if repository != SCRIPT.parents[3] or Path(checks.__file__).resolve() != SCRIPT.with_name("checks.py"):
        raise ValueError("runner and repository must be the same checkout")
    revision = clean_revision(repository)
    expected_path = repository / ORACLE
    expected_bytes = expected_path.read_bytes()
    expected = oracle(read_json(expected_path))
    output.mkdir(parents=True, exist_ok=False)
    try:
        env = dict(os.environ, CARGO_TARGET_DIR=str(output / "build"))
        build = ["cargo", "build", "--locked", "--release", "--target", "wasm32-unknown-unknown",
                 "--manifest-path", str(repository / MANIFEST)]
        supervise(["rustc", "--version", "--verbose"], output, "rustc", 30, env, repository)
        supervise(["cargo", "--version", "--verbose"], output, "cargo", 30, env, repository)
        supervise(build, output, "build", 300, env, repository)
        if clean_revision(repository) != revision:
            raise ValueError("source changed during build")
        wasm = output / "build/wasm32-unknown-unknown/release/nepl3_conformance_browser.wasm"
        payload = wasm.read_bytes()
        if not 8 <= len(payload) <= 5 * 1024 * 1024 or payload[:8] != b"\0asm\1\0\0\0":
            raise ValueError("invalid Wasm payload")
        wasm_hash = digest(payload)
        with (output / "fixture.wasm").open("xb") as stream:
            _ = stream.write(payload)
        wasm = output / "fixture.wasm"
        summaries: list[JsonValue] = []
        for engine in ENGINES:
            argv = [sys.executable, str(SCRIPT), "--engine", engine, "--wasm", str(wasm), "--oracle", str(expected_path)]
            supervise(argv, output, engine, timeout, cwd=repository)
            raw = read_json(output / f"{engine}.stdout")
            count = verify(raw, engine, wasm_hash, expected)
            summaries.append({"engine": engine, "version": string(object_value(raw)["engine_version"]), "cases": count})
            if digest(wasm.read_bytes()) != wasm_hash or expected_path.read_bytes() != expected_bytes:
                raise ValueError("execution input changed")
        if clean_revision(repository) != revision:
            raise ValueError("source changed during execution")
        inputs = [ORACLE, MANIFEST, MANIFEST.with_name("Cargo.lock"),
                  MANIFEST.parent / "src/lib.rs", Path("tools/emulators/browser/run.py"),
                  Path("tools/emulators/browser/checks.py"), Path("tools/emulators/browser/processes.py"), Path("tools/audit/doc_html/requirements.txt")]
        logs = {p.name: digest(p.read_bytes()) for p in output.iterdir() if p.is_file()}
        receipt: dict[str, JsonValue] = {"version": 1, "scope": "browser-foundation-source-position-slice",
                   "acceptance_decision": False, "result": "passed", "source_commit": revision,
                   "source_identity_scope": "clean checkout before/after build and execution; not a cryptographic execution attestation",
                   "wasm_sha256": wasm_hash, "engines": summaries,
                   "inputs": {str(p): digest((repository / p).read_bytes()) for p in inputs}, "logs": dict(logs)}
        write_json(output / "receipt.json", receipt)
        return receipt
    except Exception as exc:
        write_json(output / "failure.json", {"result": "failed", "acceptance_decision": False, "error": str(exc)})
        raise


class Arguments(argparse.Namespace):
    repository: Path | None = None
    output: Path | None = None
    timeout: int = 90
    engine: Engine | None = None
    wasm: Path | None = None
    oracle: Path | None = None


def main() -> None:
    parser = argparse.ArgumentParser()
    _ = parser.add_argument("--repository", type=Path)
    _ = parser.add_argument("--output", type=Path)
    _ = parser.add_argument("--timeout", type=int, default=90)
    _ = parser.add_argument("--engine", choices=ENGINES)
    _ = parser.add_argument("--wasm", type=Path)
    _ = parser.add_argument("--oracle", type=Path)
    args = parser.parse_args(namespace=Arguments())
    if args.engine:
        if not args.wasm or not args.oracle:
            parser.error("worker requires Wasm and oracle")
        worker(args.engine, args.wasm, args.oracle)
    else:
        if not args.repository or not args.output:
            parser.error("repository and fresh output required")
        print(json.dumps(run(args.repository, args.output.resolve(), args.timeout), ensure_ascii=False))


if __name__ == "__main__":
    main()
