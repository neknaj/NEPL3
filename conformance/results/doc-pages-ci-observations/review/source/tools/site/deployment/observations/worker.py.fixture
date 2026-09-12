"""Private isolated observation process; credentials only on stdin."""
import base64
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from deployment.observations import Query, Kind, _read
from journal.model import decode


def main():
    try:
        raw = sys.stdin.buffer.read(12001)
        if len(raw) > 12000:
            return 1
        body = decode(raw)
        query = body['query']
        query['kind'] = Kind(query['kind'])
        result = _read(Query(**query), body['token'], timeout=body['timeout'])
        sys.stdout.buffer.write(base64.b64encode(result))
        return 0
    except Exception:
        return 1


if __name__ == '__main__':
    sys.exit(main())
