"""Normal fast-forward journal push plus explicit remote acknowledgement.

The caller owns the production publisher lock and verified remote protection.
No force option, automatic retry, deployment or permission change is provided.
"""
from dataclasses import dataclass

from payload import checked
from .model import hex_id
from .store import REF, git, load, repository


@dataclass(frozen=True)
class Confirmation:
    head: str
    already_present: bool
    publication_verified: bool = False


def origin(repo, expected_url):
    checked(isinstance(expected_url, str) and 0 < len(expected_url) <= 2048 and
            not any(c in expected_url for c in '\r\n\0'), 'invalid expected origin')
    for args in [('remote', 'get-url', '--all', 'origin'),
                 ('remote', 'get-url', '--push', '--all', 'origin')]:
        urls = git(repo, *args).decode('utf-8').splitlines()
        checked(urls == [expected_url], 'journal origin differs from pinned fetch/push URL')


def head(repo, expected_url):
    repository(repo)
    origin(repo, expected_url)
    rows = git(repo, 'ls-remote', '--refs', 'origin', REF).decode('ascii').splitlines()
    if not rows:
        return None
    checked(len(rows) == 1, 'ambiguous remote journal ref')
    fields = rows[0].split('\t')
    checked(len(fields) == 2 and fields[1] == REF, 'unexpected remote journal ref')
    hex_id(fields[0], 40)
    return fields[0]


def publish(repo, expected_url, expected_remote_head, new_head):
    hex_id(new_head, 40)
    if expected_remote_head is not None:
        hex_id(expected_remote_head, 40)
    state = load(repo)
    checked(state.head == new_head, 'local journal differs from requested head')
    parents = git(repo, 'rev-list', '--parents', '-n', '1', new_head).decode('ascii').split()
    checked(parents == [new_head] + ([expected_remote_head] if expected_remote_head else []),
            'journal publication must append exactly one commit')
    before = head(repo, expected_url)
    if before == new_head:
        # Supports reconciliation after the server accepted a push but the
        # previous process lost its acknowledgement. No new write is issued.
        return Confirmation(new_head, True)
    checked(before == expected_remote_head, 'remote journal changed; reconcile before writing')
    # A competing descendant of expected_remote_head makes this a non-fast-
    # forward push. Never force or substitute the mutable local branch name.
    git(repo, 'push', '--porcelain', '--no-follow-tags', 'origin', new_head + ':' + REF)
    checked(head(repo, expected_url) == new_head, 'remote journal acknowledgement differs; state unknown')
    return Confirmation(new_head, False)
