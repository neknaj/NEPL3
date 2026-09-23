"""Validate downloaded Actions inputs and write the original Pages tar.

Obtain metadata and archive through an authenticated download stage. All identity
arguments must be selected by the publisher, not copied from untrusted inputs.
This command does not establish run eligibility or perform a deployment.
"""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from deployment.artifact import MAX_ARCHIVE, selected
from payload import Receipt, checked, real_directory


def bounded(path: Path, maximum: int) -> bytes:
    with path.open('rb') as stream:
        data = stream.read(maximum + 1)
    checked(len(data) <= maximum, 'input size limit')
    return data


def export(metadata: Path, archive: Path, output: Path, *, owner: str, repository: str,
           artifact_id: int, run_id: int, repository_id: int, source_commit: str,
           expected_tar: str, expected_manifest: str) -> Receipt:
    # Validate all bytes before creating output. The x mode also refuses an
    # existing symlink, and the parent must have no linked ancestors.
    real_directory(output.parent)
    result = selected(bounded(metadata, 65536), bounded(archive, MAX_ARCHIVE), owner=owner,
                      repository=repository, artifact_id=artifact_id, run_id=run_id,
                      repository_id=repository_id, source_commit=source_commit,
                      expected_tar=expected_tar, expected_manifest=expected_manifest)
    with output.open('xb') as stream:
        _ = stream.write(result.data)
    return Receipt(result.manifest_sha256, result.tar_sha256, len(result.data), result.files)


class Arguments(argparse.Namespace):
    metadata: Path = Path()
    archive: Path = Path()
    output: Path = Path()
    owner: str = ''
    repository: str = ''
    source_commit: str = ''
    expected_tar: str = ''
    expected_manifest: str = ''
    artifact_id: int = 0
    run_id: int = 0
    repository_id: int = 0


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument('metadata', type=Path)
    _ = parser.add_argument('archive', type=Path)
    _ = parser.add_argument('output', type=Path)
    for option in ('owner', 'repository', 'source-commit', 'expected-tar', 'expected-manifest'):
        _ = parser.add_argument('--' + option, required=True)
    for option in ('artifact-id', 'run-id', 'repository-id'):
        _ = parser.add_argument('--' + option, required=True, type=int)
    args = parser.parse_args(namespace=Arguments())
    try:
        report = export(args.metadata, args.archive, args.output, owner=args.owner,
                        repository=args.repository, source_commit=args.source_commit,
                        expected_tar=args.expected_tar, expected_manifest=args.expected_manifest,
                        artifact_id=args.artifact_id, run_id=args.run_id, repository_id=args.repository_id)
    except Exception as error:
        # No token, remote download URL, or untrusted exception text in logs.
        print(json.dumps(dict(result='failed', reason=type(error).__name__, publication_verified=False)))
        return 1
    print(json.dumps(report.representation(), sort_keys=True))
    return 0


if __name__ == '__main__':
    sys.exit(main())
