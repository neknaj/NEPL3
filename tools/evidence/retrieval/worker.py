"""Isolated bounded artifact transport worker. Never print input or exceptions."""
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parents[3]))

import base64
import json
from tools.evidence.retrieval.network import direct
from tools.evidence.retrieval.pin import Pin
from tools.serialization.json import decode, object_value, string


def main() -> None:
    raw = sys.stdin.buffer.read(16385)
    if len(raw) > 16384:
        raise ValueError('worker input limit')
    request = object_value(decode(raw, reject_duplicates=True, reject_nonfinite=True))
    if set(request) != {'pin', 'token'}:
        raise ValueError('worker input fields')
    result = direct(Pin.read(request['pin']), string(request['token']))
    response = {key: base64.b64encode(value).decode('ascii') for key, value in
                [('artifact', result.artifact), ('run', result.run), ('archive', result.archive)]}
    _ = sys.stdout.write(json.dumps(response))


if __name__ == '__main__':
    try:
        main()
    except Exception:
        raise SystemExit(1) from None
