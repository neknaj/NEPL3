"""Typed outer-artifact pins shared by command and review log retrieval."""
from dataclasses import dataclass
import re
from tools.evidence.archive import checked
from tools.evidence.artifact import member_name
from tools.serialization.json import JsonValue, integer, object_value, string


@dataclass(frozen=True, slots=True)
class Pin:
    repository_id: int
    run_id: int
    run_attempt: int
    source_commit: str
    artifact_id: int
    artifact_name: str
    archive_sha256: str
    archive_bytes: int
    member: str
    expires_at_unix: int

    @classmethod
    def read(cls, value: JsonValue) -> 'Pin':
        fields = object_value(value)
        checked(set(fields) == {'schema', 'repository', 'repository_id', 'run_id', 'run_attempt',
                                'source_commit', 'artifact_id', 'artifact_name', 'archive_sha256',
                                'archive_bytes', 'member', 'expires_at_unix'}, 'artifact pin fields')
        checked(fields['schema'] == 'nepl3.github-artifact/1' and fields['repository'] == 'neknaj/NEPL3',
                'unsupported artifact pin source')
        result = cls(integer(fields['repository_id']), integer(fields['run_id']), integer(fields['run_attempt']),
                     string(fields['source_commit']), integer(fields['artifact_id']), string(fields['artifact_name']),
                     string(fields['archive_sha256']), integer(fields['archive_bytes']), string(fields['member']),
                     integer(fields['expires_at_unix']))
        result.validate()
        return result

    def validate(self) -> None:
        for value in (self.repository_id, self.run_id, self.run_attempt, self.artifact_id):
            checked(type(value) is int and 0 < value < 2**53, 'artifact identifier bound')
        checked(re.fullmatch(r'[0-9a-f]{40}', self.source_commit), 'artifact source commit')
        checked(re.fullmatch(r'[A-Za-z0-9_.-]{1,128}', self.artifact_name), 'artifact name')
        checked(re.fullmatch(r'[0-9a-f]{64}', self.archive_sha256), 'artifact archive digest')
        checked(type(self.archive_bytes) is int and 22 <= self.archive_bytes <= 32 * 1024 * 1024,
                'artifact archive bound')
        _ = member_name(self.member)
        checked(self.member.endswith(('.stdout', '.stderr', '.txt', '.log')), 'artifact log member suffix')
        checked(type(self.expires_at_unix) is int and 1 <= self.expires_at_unix <= 253402300799,
                'artifact expiry bound')

    @property
    def artifact_url(self) -> str:
        return f'https://api.github.com/repos/neknaj/NEPL3/actions/artifacts/{self.artifact_id}'

    @property
    def run_url(self) -> str:
        return f'https://api.github.com/repos/neknaj/NEPL3/actions/runs/{self.run_id}/attempts/{self.run_attempt}'
