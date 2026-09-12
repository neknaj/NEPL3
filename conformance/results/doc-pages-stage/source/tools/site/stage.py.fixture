"""Stage the original checked tar for the pinned Pages artifact upload action."""
import argparse
import json
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent))
from artifact import bounded
from payload import real_directory
from recovery import verify


def stage(source, output, expected_tar, expected_manifest):
    real_directory(output.parent)
    payload = verify(bounded(source, 40 * 1024 * 1024), expected_tar, expected_manifest)
    with output.open('xb') as stream:
        stream.write(payload.data)
    return dict(version=1, tar_sha256=payload.tar_sha256,
                manifest_sha256=payload.manifest_sha256, tar_bytes=len(payload.data),
                publication_verified=False)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('source', type=Path)
    parser.add_argument('output', type=Path)
    parser.add_argument('--expected-tar', required=True)
    parser.add_argument('--expected-manifest', required=True)
    try:
        report = stage(**vars(parser.parse_args()))
    except Exception as error:
        print(json.dumps(dict(result='failed', reason=type(error).__name__, publication_verified=False)))
        return 1
    print(json.dumps(report, sort_keys=True))
    return 0


if __name__ == '__main__':
    sys.exit(main())
