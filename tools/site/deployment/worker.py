"""Private subprocess entry: credentials arrive only through stdin."""
import base64
from pathlib import Path
import sys
from dataclasses import dataclass, field

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from deployment.receipt import Receipt
from deployment.transport import direct_status
from journal.model import decode
from tools.serialization.json import object_value, string


@dataclass(frozen=True, slots=True)
class Request:
    receipt: Receipt
    token: str = field(repr=False)
    timeout: float


def request_value(raw: bytes) -> Request:
    value = object_value(decode(raw))
    receipt = object_value(value['receipt'])
    if set(receipt) != {'deployment_id', 'status_endpoint', 'response_sha256'}:
        raise ValueError('invalid receipt fields')
    timeout = value['timeout']
    if isinstance(timeout, bool) or not isinstance(timeout, (int, float)):
        raise ValueError('invalid timeout type')
    return Request(Receipt(string(receipt['deployment_id']), string(receipt['status_endpoint']),
                           string(receipt['response_sha256'])), string(value['token']), timeout)


def main() -> int:
    try:
        raw = sys.stdin.buffer.read(12001)
        if len(raw) > 12000:
            return 1
        request = request_value(raw)
        result = direct_status(request.receipt, request.token, timeout=request.timeout)
        _ = sys.stdout.buffer.write(base64.b64encode(result.raw_response))
        return 0
    except Exception:
        # No traceback, server body or request/credential dump crosses this IPC.
        return 1


if __name__ == "__main__":
    sys.exit(main())
