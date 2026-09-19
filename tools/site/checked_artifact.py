"""Revalidate the successful CI's original docs-only payload before upload."""
import argparse
import json
from pathlib import Path

from payload import checked, snapshot, tar_bytes
from recovery import verify


def validate(root, commit):
    report = json.loads((root / 'site-results.json').read_text(encoding='utf-8'))
    receipt = json.loads((root / 'pages-receipt.json').read_text(encoding='utf-8'))
    checked(report['result'] == 'passed' and report['source_commit'] == commit,
            'CI report does not match successful source')
    identity = report['manifest_sha256']
    checked(receipt['manifest_sha256'] == identity, 'receipt differs from CI report')
    files = snapshot(root / 'site', identity)
    build = json.loads(files['build.json'])
    checked(build['source_commit'] == commit and build['base_path'] == '/NEPL3/',
            'wrong publication source or base')
    data = (root / 'pages.tar').read_bytes()
    verify(data, receipt['tar_sha256'], identity)
    checked(tar_bytes(files) == data, 'site and saved payload differ')
    return identity


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('root', type=Path)
    parser.add_argument('commit')
    args = parser.parse_args()
    print(validate(args.root, args.commit))
