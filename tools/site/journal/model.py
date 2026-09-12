from dataclasses import asdict, dataclass
import json
import re

from payload import checked, unique_object

KINDS = frozenset(['DeployIntent', 'DeployReceipt', 'Observation', 'SmokePassed',
                  'SmokeFailed', 'RecoveryIntent', 'RecoveryReceipt', 'HealthyUncommitted',
                  'Healthy', 'Recovered', 'Superseded', 'RecoveryBlocked', 'RecoveryUnknown',
                  'BootstrapFailed', 'RecoveryFailed'])
MAX_EVIDENCE = 64 * 1024
MAX_EVENTS = 1024
MAX_TOTAL = 32 * 1024 * 1024


def hex_id(value, size):
    checked(isinstance(value, str) and re.fullmatch('[0-9a-f]{' + str(size) + '}', value), 'invalid identity')


def encode(value):
    return (json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False,
                       allow_nan=False) + '\n').encode('utf-8')


def decode(data):
    return json.loads(data.decode('utf-8'), object_pairs_hook=unique_object,
                      parse_constant=lambda value: checked(False, 'nonfinite JSON'))


@dataclass(frozen=True)
class Event:
    kind: str
    transaction: str
    run_id: int
    attempt: int
    source_commit: str
    payload_sha256: str

    def validate(self):
        checked(isinstance(self.kind, str) and self.kind in KINDS, 'unknown journal event')
        checked(isinstance(self.transaction, str) and re.fullmatch('[a-zA-Z0-9-]{1,80}', self.transaction), 'invalid transaction')
        checked(type(self.run_id) is int and 0 < self.run_id < 2**64, 'invalid run ID')
        checked(type(self.attempt) is int and 0 < self.attempt < 2**32, 'invalid attempt')
        hex_id(self.source_commit, 40)
        hex_id(self.payload_sha256, 64)

    def record(self, sequence, evidence_sha256):
        self.validate()
        return dict(version=1, sequence=sequence, evidence_sha256=evidence_sha256, **asdict(self))


@dataclass(frozen=True)
class Snapshot:
    head: str | None
    events: tuple[Event, ...]
    evidence: tuple[bytes, ...]
