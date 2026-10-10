"""Restore existing acceptance logs; never create or promote an acceptance."""
import os
from pathlib import Path
import sys
import time

from tools.evidence.retrieval import plan
from tools.evidence.retrieval.restore import restore


def main() -> None:
    if len(sys.argv) != 1:
        raise ValueError('usage: python -m tools.evidence.retrieval')
    root = Path(__file__).resolve().parents[3]
    before = plan.identity(root)
    requests = plan.requests(root, before)
    known: set[str] = set()
    def check(commit: str) -> None:
        if commit not in known:
            if plan.identity(root, commit) != before:
                raise ValueError('artifact source/spec identity differs from current evidence inputs')
            known.add(commit)
        if plan.identity(root) != before:
            raise ValueError('current evidence inputs changed during restoration')
    count = restore(root, requests, os.environ.get('GITHUB_TOKEN', ''), check)
    if plan.identity(root) != before:
        raise ValueError('current evidence inputs changed after restoration')
    if any(request.artifact is not None and int(time.time()) >= request.artifact.expires_at_unix for request in requests):
        raise ValueError('artifact expired before retrieval completed')
    print(f'Restored {count} raw logs; acceptance status unchanged. Run the existing Rust task/repository checks.')


if __name__ == '__main__':
    try:
        main()
    except Exception as error:
        # Library failures carry no credentials; still do not print arbitrary
        # server responses, URLs or subprocess output from unexpected failures.
        print('Evidence restoration failed: ' + type(error).__name__, file=sys.stderr)
        raise SystemExit(1) from None
