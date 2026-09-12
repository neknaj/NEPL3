"""Isolated archive fetch: neither credentials nor signed URLs are logged."""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[2]))
from deployment.download import _download
from deployment.observations import Query, Kind
from journal.model import decode


def main():
    try:
        raw = sys.stdin.buffer.read(12001)
        if len(raw) > 12000:
            return 1
        body = decode(raw)
        query = body.pop('query')
        query['kind'] = Kind(query['kind'])
        result = _download(Query(**query), **body)
        sys.stdout.buffer.write(result)
        return 0
    except Exception:
        return 1


if __name__ == '__main__':
    sys.exit(main())
