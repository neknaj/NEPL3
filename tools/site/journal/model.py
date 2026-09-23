from dataclasses import dataclass
import json
import re
from typing import Literal

from payload import checked
from tools.serialization.json import JsonValue, decode as decode_json, object_value, integer, string

type Kind = Literal['DeployIntent', 'DeployReceipt', 'Observation', 'SmokePassed',
                    'SmokeFailed', 'RecoveryIntent', 'RecoveryReceipt', 'HealthyUncommitted',
                    'Healthy', 'Recovered', 'Superseded', 'RecoveryBlocked', 'RecoveryUnknown',
                    'BootstrapFailed', 'RecoveryFailed']
KINDS: frozenset[Kind] = frozenset(['DeployIntent', 'DeployReceipt', 'Observation', 'SmokePassed',
                  'SmokeFailed', 'RecoveryIntent', 'RecoveryReceipt', 'HealthyUncommitted',
                  'Healthy', 'Recovered', 'Superseded', 'RecoveryBlocked', 'RecoveryUnknown',
                  'BootstrapFailed', 'RecoveryFailed'])
MAX_EVIDENCE = 64 * 1024
MAX_EVENTS = 1024
MAX_TOTAL = 32 * 1024 * 1024


def hex_id(value: str, size: int) -> None:
    checked(re.fullmatch('[0-9a-f]{' + str(size) + '}', value), 'invalid identity')


def encode(value: JsonValue) -> bytes:
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False,
                       allow_nan=False) + '\n').encode('utf-8')


def decode(data: bytes) -> JsonValue:
    return decode_json(data, reject_duplicates=True, reject_nonfinite=True)


@dataclass(frozen=True, slots=True)
class Event:
    kind: Kind
    transaction: str
    run_id: int
    attempt: int
    source_commit: str
    payload_sha256: str

    def validate(self) -> None:
        checked(self.kind in KINDS, 'unknown journal event')
        checked(re.fullmatch('[a-zA-Z0-9-]{1,80}', self.transaction), 'invalid transaction')
        checked(type(self.run_id) is int and 0 < self.run_id < 2**64, 'invalid run ID')
        checked(type(self.attempt) is int and 0 < self.attempt < 2**32, 'invalid attempt')
        hex_id(self.source_commit, 40)
        hex_id(self.payload_sha256, 64)

    def record(self, sequence: int, evidence_sha256: str) -> dict[str, JsonValue]:
        self.validate()
        return dict(version=1, sequence=sequence, evidence_sha256=evidence_sha256,
                    kind=self.kind, transaction=self.transaction, run_id=self.run_id,
                    attempt=self.attempt, source_commit=self.source_commit, payload_sha256=self.payload_sha256)


def event_record(data: bytes, sequence: int, evidence_sha256: str) -> Event:
    record = object_value(decode(data))
    checked(integer(record.get('version')) == 1, 'journal version')
    checked(integer(record.get('sequence')) == sequence, 'journal sequence')
    checked(set(record) == {'version', 'sequence', 'evidence_sha256', 'kind', 'transaction',
                           'run_id', 'attempt', 'source_commit', 'payload_sha256'}, 'journal event fields')
    checked(data == encode(dict(record)), 'noncanonical journal envelope')
    name = string(record['kind'])
    kind: Kind | None = next((kind for kind in KINDS if kind == name), None)
    if kind is None:
        raise ValueError('unknown journal event')
    event = Event(kind, string(record['transaction']), integer(record['run_id']),
                  integer(record['attempt']), string(record['source_commit']), string(record['payload_sha256']))
    event.validate()
    checked(record['evidence_sha256'] == evidence_sha256, 'journal evidence digest')
    return event


@dataclass(frozen=True, slots=True)
class Snapshot:
    head: str | None
    events: tuple[Event, ...]
    evidence: tuple[bytes, ...]
