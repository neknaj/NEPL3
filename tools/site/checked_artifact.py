"""Revalidate the successful CI's original docs-only payload before upload."""
import argparse
from dataclasses import dataclass
from pathlib import Path

from payload import checked, snapshot, tar_bytes
from recovery import verify
from tools.serialization.json import decode, object_value, string


@dataclass(frozen=True, slots=True)
class Identities:
    manifest: str
    tar: str


def checked_identities(report_source: bytes, receipt_source: bytes, commit: str) -> Identities:
    report = object_value(decode(report_source))
    receipt = object_value(decode(receipt_source))
    checked(report['result'] == 'passed' and report['source_commit'] == commit,
            'CI report does not match successful source')
    identity = string(report['manifest_sha256'])
    checked(receipt['manifest_sha256'] == identity, 'receipt differs from CI report')
    return Identities(identity, string(receipt['tar_sha256']))


def check_build(source: bytes, commit: str) -> None:
    build = object_value(decode(source))
    checked(build['source_commit'] == commit and build['base_path'] == '/NEPL3/',
            'wrong publication source or base')


def validate(root: Path, commit: str) -> str:
    identities = checked_identities((root / 'site-results.json').read_bytes(),
                                    (root / 'pages-receipt.json').read_bytes(), commit)
    identity = identities.manifest
    files = snapshot(root / 'site', identity)
    check_build(files['build.json'], commit)
    data = (root / 'pages.tar').read_bytes()
    _ = verify(data, identities.tar, identity)
    checked(tar_bytes(files) == data, 'site and saved payload differ')
    return identity


class Arguments(argparse.Namespace):
    root: Path = Path()
    commit: str = ''


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    _ = parser.add_argument('root', type=Path)
    _ = parser.add_argument('commit')
    args = parser.parse_args(namespace=Arguments())
    print(validate(args.root, args.commit))


if __name__ == '__main__':
    main()
