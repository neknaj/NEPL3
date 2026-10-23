"""Prefer the runner's existing primary Ubuntu mirror without changing trust."""
from __future__ import annotations

import argparse
import os
from pathlib import Path
import stat
import sys


RUNNER_MIRRORS = (
    b"http://azure.archive.ubuntu.com/ubuntu/\tpriority:1\n"
    b"https://archive.ubuntu.com/ubuntu/\tpriority:2\n"
    b"https://security.ubuntu.com/ubuntu/\tpriority:3\n"
)
PRIMARY_MIRRORS = (
    b"http://azure.archive.ubuntu.com/ubuntu/\tpriority:2\n"
    b"https://archive.ubuntu.com/ubuntu/\tpriority:1\n"
    b"https://security.ubuntu.com/ubuntu/\tpriority:3\n"
)


class MirrorConfigurationError(ValueError):
    """The input is not the narrowly supported runner mirror list."""


def prefer_primary(contents: bytes) -> bytes:
    """Swap priorities only in the known image list; reject unknown input."""
    if contents == PRIMARY_MIRRORS:
        return contents
    if contents != RUNNER_MIRRORS:
        raise MirrorConfigurationError("unexpected Ubuntu runner mirror list")
    return PRIMARY_MIRRORS


def update(path: Path) -> bool:
    """Edit the existing regular file in place, preserving its permissions."""
    if sys.platform != "linux":
        raise MirrorConfigurationError("Ubuntu mirror selection requires Linux")
    descriptor = os.open(path, os.O_RDWR | os.O_NOFOLLOW | os.O_NONBLOCK)
    try:
        if not stat.S_ISREG(os.fstat(descriptor).st_mode):
            raise MirrorConfigurationError("mirror list must be a regular file")
        with open(descriptor, "r+b", closefd=False) as stream:
            before = stream.read(len(RUNNER_MIRRORS) + 1)
            after = prefer_primary(before)
            if after == before:
                return False
            # The supported edit changes two one-byte priorities, never the URIs.
            _ = stream.seek(0)
            if stream.write(after) != len(after):
                raise OSError("incomplete mirror-list write")
            stream.flush()
    finally:
        os.close(descriptor)
    return True


class Arguments(argparse.Namespace):
    mirror_list: Path | None = None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument("mirror_list", type=Path)
    arguments = parser.parse_args(namespace=Arguments())
    if arguments.mirror_list is None:
        parser.error("mirror list is required")
    try:
        changed = update(arguments.mirror_list)
    except (OSError, MirrorConfigurationError) as error:
        print(f"Ubuntu mirror preference failed: {error}", file=sys.stderr)
        return 1
    print("Ubuntu primary mirror priority " + ("selected" if changed else "already selected"))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
