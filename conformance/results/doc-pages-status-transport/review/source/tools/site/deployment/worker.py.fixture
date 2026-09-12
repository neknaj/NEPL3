"""Private subprocess entry: credentials arrive only through stdin."""
import base64
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from deployment.receipt import Receipt
from deployment.transport import _status
from journal.model import decode


def main():
    try:
        raw = sys.stdin.buffer.read(12001)
        if len(raw) > 12000:
            return 1
        request = decode(raw)
        result = _status(Receipt(**request["receipt"]), request["token"], timeout=request["timeout"])
        sys.stdout.buffer.write(base64.b64encode(result.raw_response))
        return 0
    except Exception:
        # No traceback, server body or request/credential dump crosses this IPC.
        return 1


if __name__ == "__main__":
    sys.exit(main())
