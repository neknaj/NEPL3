"""Immutable HTTP observations and their existing external JSON representation."""
from dataclasses import dataclass
from typing import Literal, assert_never

from tools.serialization.json import JsonValue, array, decode, integer, object_value, string

type Transport = Literal['loopback-http', 'https']


@dataclass(frozen=True, slots=True)
class Context:
    url: str
    manifest_sha256: str
    transport: Transport

    def representation(self) -> dict[str, JsonValue]:
        return dict(url=self.url, manifest_sha256=self.manifest_sha256,
                    transport=self.transport, publication_verified=False)


@dataclass(frozen=True, slots=True)
class Missing:
    route: str
    url: str

    def representation(self) -> dict[str, JsonValue]:
        return dict(route=self.route, url=self.url, status=404)


@dataclass(frozen=True, slots=True)
class Content:
    route: str
    url: str
    bytes: int
    sha256: str
    mime: str

    def representation(self) -> dict[str, JsonValue]:
        return dict(route=self.route, url=self.url, status=200, bytes=self.bytes,
                    sha256=self.sha256, mime=self.mime)


type Observation = Missing | Content


@dataclass(frozen=True, slots=True)
class Passed:
    context: Context
    source_commit: str
    observations: tuple[Observation, ...]

    def representation(self) -> dict[str, JsonValue]:
        return dict(version=1, result='passed', kind='docs-only-http-byte-check',
                    source_commit=self.source_commit,
                    observations=[row.representation() for row in self.observations],
                    omitted_control_files=['.nojekyll'], **self.context.representation())


@dataclass(frozen=True, slots=True)
class Deadline:
    pass


@dataclass(frozen=True, slots=True)
class WorkerExit:
    exit_code: int


@dataclass(frozen=True, slots=True)
class ObservationError:
    reason: str
    detail: str


type Cause = Deadline | WorkerExit | ObservationError


@dataclass(frozen=True, slots=True)
class Failed:
    cause: Cause
    # Isolated worker errors acquire the caller's context at the process boundary.
    context: Context | None = None

    def representation(self) -> dict[str, JsonValue]:
        result: dict[str, JsonValue] = dict(version=1, result='failed', publication_verified=False)
        match self.cause:
            case Deadline():
                result['reason'] = 'deadline'
            case WorkerExit(exit_code):
                result.update(reason='worker-failed', exit_code=exit_code)
            case ObservationError(reason, detail):
                result.update(reason=reason, detail=detail)
            case _:
                assert_never(self.cause)
        if self.context is not None:
            result.update(self.context.representation())
        return result


type Report = Passed | Failed


def worker_report(raw: bytes, context: Context) -> Report:
    """Validate the process response before transferring it into host logic."""
    value = object_value(decode(raw, reject_duplicates=True, reject_nonfinite=True))
    if integer(value.get('version')) != 1 or value.get('publication_verified') is not False:
        raise ValueError('invalid smoke worker report')
    if value.get('result') == 'failed':
        return Failed(ObservationError(string(value.get('reason')), string(value.get('detail'))), context)
    if value.get('result') != 'passed' or value.get('kind') != 'docs-only-http-byte-check':
        raise ValueError('invalid smoke worker result')
    if value.get('omitted_control_files') != ['.nojekyll']:
        raise ValueError('invalid smoke omitted files')
    rows: list[Observation] = []
    for item in array(value.get('observations')):
        row = object_value(item)
        route, url = string(row.get('route')), string(row.get('url'))
        status = integer(row.get('status'))
        if status == 404:
            rows.append(Missing(route, url))
        elif status == 200:
            rows.append(Content(route, url, integer(row.get('bytes')),
                                string(row.get('sha256')), string(row.get('mime'))))
        else:
            raise ValueError('invalid smoke observation status')
    return Passed(context, string(value.get('source_commit')), tuple(rows))
