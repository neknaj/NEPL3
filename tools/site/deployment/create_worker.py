"""Isolated creation attempt; credentials only cross stdin, never argv or logs."""
import base64
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from deployment.create import _create
from journal.model import decode


def main():
    try:
        raw = sys.stdin.buffer.read(24001)
        if len(raw) > 24000:
            return 1
        response = _create(**decode(raw))
        sys.stdout.buffer.write(base64.b64encode(response))
        return 0
    except Exception:
        return 1


if __name__ == "__main__":
    sys.exit(main())
