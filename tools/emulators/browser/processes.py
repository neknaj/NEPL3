"""Linux process-tree tracking, including detached Playwright browser children.

This bounds trusted test runners, not adversarial processes escaping supervision.
PID start times reduce PID-reuse risk; check and signal are not atomic.
"""
from dataclasses import dataclass
from pathlib import Path
import os
import signal
import time

@dataclass(frozen=True, slots=True)
class Process:
    pid: int
    parent: int
    started: str
    state: str


def snapshot() -> dict[int, Process]:
    result: dict[int, Process] = {}
    for path in Path("/proc").iterdir():
        if not path.name.isdecimal():
            continue
        try:
            # comm can contain spaces and parentheses; fields follow its final ')'.
            fields = (path / "stat").read_text().rsplit(")", 1)[1].split()
            process = Process(int(path.name), int(fields[1]), fields[19], fields[0])
            result[process.pid] = process
        except (OSError, ValueError, IndexError):
            continue  # A process can exit between directory enumeration and read.
    return result


def same(current: Process | None, old: Process) -> bool:
    return current is not None and current.pid == old.pid and current.started == old.started


def remember(known: dict[int, Process]) -> None:
    current = snapshot()
    roots = {pid for pid, old in known.items() if same(current.get(pid), old)}
    while True:
        found = {pid for pid, p in current.items() if p.parent in roots}
        expanded = roots | found
        if expanded == roots:
            break
        roots = expanded
    for pid in roots:
        if pid in current:
            known[pid] = current[pid]


def send(known: dict[int, Process], sig: signal.Signals) -> None:
    current = snapshot()
    for pid, old in known.items():
        if same(current.get(pid), old):
            try:
                os.kill(pid, sig)
            except ProcessLookupError:
                pass


def cleanup(known: dict[int, Process]) -> None:
    remember(known)
    send(known, signal.SIGTERM)
    time.sleep(0.2)
    remember(known)
    send(known, signal.SIGKILL)
    for _ in range(50):
        current = snapshot()
        if not any(same(current.get(pid), old) and current[pid].state != "Z" for pid, old in known.items()):
            return
        time.sleep(0.02)
    raise RuntimeError("supervised descendants survived cleanup")
