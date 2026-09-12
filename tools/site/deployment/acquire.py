"""Connect authenticated observations, downloads and candidate validation."""
from dataclasses import dataclass
import time

from deployment import candidate
from deployment.download import download
from deployment.observations import Query, Kind, read
from deployment.receipt import endpoint
from journal.model import hex_id
from payload import checked
from recovery import Payload


@dataclass(frozen=True)
class Evidence:
    main_before: bytes
    run_before: bytes
    jobs_before: bytes
    diagnostic_metadata: bytes
    diagnostic_archive: bytes
    upload_metadata: bytes
    upload_archive: bytes
    main_after: bytes
    run_after: bytes
    jobs_after: bytes


@dataclass(frozen=True)
class Acquired:
    candidate: candidate.Candidate
    payload: Payload
    evidence: Evidence


def main_identity(raw, *, owner, repository, expected):
    value = candidate.document(raw)
    checked(value.get('ref') == 'refs/heads/main'
            and value.get('url') == f'https://api.github.com/repos/{owner}/{repository}/git/refs/heads/main',
            'main ref identity mismatch')
    obj = value.get('object')
    checked(isinstance(obj, dict) and obj.get('type') == 'commit' and obj.get('sha') == expected,
            'main source changed')


def acquire(token, *, owner, repository, repository_id, workflow_id, run_id,
            attempt, current_main, artifact_id, upload_id, expected_tar,
            expected_manifest, timeout=300):
    """Prepare one selected CI upload using fresh authenticated observations.

    Caller must hold the publisher lock and supply independently selected pins.
    This does not supply permission/current-publication/recovery authorization.
    Even the final observations are not an atomic server snapshot; mutation
    still requires the publisher's state checks. Raw evidence is retained on
    success; failures raise without returning partial observations.
    """
    endpoint(owner, repository, '1')
    for value in (repository_id, workflow_id, run_id, attempt, artifact_id, upload_id):
        candidate.integer(value)
    checked(artifact_id != upload_id, 'diagnostic artifact is not a Pages upload')
    for value, length in ((current_main,40),(expected_tar,64),(expected_manifest,64)):
        hex_id(value,length)
    checked(type(timeout) in (int,float) and 0 < timeout <= 300, 'invalid acquisition timeout')
    deadline = time.monotonic() + timeout
    def left(cap):
        remaining = deadline - time.monotonic()
        checked(remaining > 0, 'acquisition deadline exceeded')
        return min(cap, remaining)
    def observe(kind, identity=0, number=0):
        return read(Query(owner,repository,kind,identity,number), token, timeout=left(10))
    pins = dict(owner=owner,repository=repository,repository_id=repository_id,
                workflow_id=workflow_id,run_id=run_id,attempt=attempt,current_main=current_main)
    def checks():
        run = observe(Kind.RUN,run_id)
        jobs = observe(Kind.JOBS,run_id,attempt)
        result = candidate.verify(run,jobs,**pins)
        main = observe(Kind.MAIN)
        main_identity(main,owner=owner,repository=repository,expected=current_main)
        return main,run,jobs,result
    before_main,before_run,before_jobs,_ = checks()
    def fetch(identity):
        raw = observe(Kind.ARTIFACT,identity)
        value = candidate.document(raw)
        checked(type(value.get('id')) is int and value['id'] == identity
                and value.get('expired') is False, 'artifact metadata identity mismatch')
        digest = value.get('digest')
        checked(isinstance(digest,str) and digest.startswith('sha256:'), 'missing archive digest')
        data = download(Query(owner,repository,Kind.ARTIFACT,identity),token,
                        size=value.get('size_in_bytes'),sha256=digest[7:],timeout=left(60))
        return raw,data
    diagnostic,archive = fetch(artifact_id)
    upload,upload_archive = fetch(upload_id)
    _,payload = candidate.prepare_upload(before_run,before_jobs,diagnostic,archive,upload,upload_archive,
                    **pins,artifact_id=artifact_id,upload_id=upload_id,
                    expected_tar=expected_tar,expected_manifest=expected_manifest)
    after_main,after_run,after_jobs,result = checks()
    left(300)
    return Acquired(result,payload,Evidence(before_main,before_run,before_jobs,diagnostic,archive,
                            upload,upload_archive,after_main,after_run,after_jobs))
