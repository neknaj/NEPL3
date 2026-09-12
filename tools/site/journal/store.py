"""Append immutable event/evidence blobs without touching a source worktree.

This is a local bare-mirror storage boundary. A successful local CAS is not
remote durability or permission to deploy; the publisher must push normally,
confirm the protected remote ref, and interpret the event/evidence contracts.
"""
import os
from pathlib import Path
import subprocess

from payload import checked, digest
from .model import Event, Snapshot, MAX_EVIDENCE, MAX_EVENTS, MAX_TOTAL, decode, encode, hex_id

REF = 'refs/heads/pages-state'


def git(repo, *args, data=None, absent=False):
    env = {key: value for key, value in os.environ.items() if not key.startswith('GIT_')}
    env.update(GIT_AUTHOR_NAME='NEPL3 Pages Journal', GIT_COMMITTER_NAME='NEPL3 Pages Journal',
               GIT_AUTHOR_EMAIL='pages-journal@invalid', GIT_COMMITTER_EMAIL='pages-journal@invalid')
    result = subprocess.run(['git', '--no-replace-objects', '-C', str(repo), *args], input=data, capture_output=True,
                            env=env, timeout=30)
    if absent and result.returncode == 1:
        return None
    if result.returncode != 0:
        raise ValueError('Git journal operation failed: ' + result.stderr.decode('utf-8', errors='replace')[:2048])
    return result.stdout


def repository(repo):
    checked(git(repo, 'rev-parse', '--is-bare-repository').strip() == b'true', 'journal requires a bare mirror')
    checked(git(repo, 'rev-parse', '--show-object-format').strip() == b'sha1', 'unsupported Git object format')
    checked(git(repo, 'rev-parse', '--is-shallow-repository').strip() == b'false', 'shallow journal history')
    checked(git(repo, 'symbolic-ref', '-q', REF, absent=True) is None, 'journal ref must not be symbolic')


def blob(repo, oid, maximum):
    checked(int(git(repo, 'cat-file', '-s', oid)) <= maximum, 'journal object size limit')
    data = git(repo, 'cat-file', 'blob', oid)
    checked(len(data) <= maximum, 'journal blob size limit')
    return data


def history(repo, head, entries):
    commits = git(repo, 'rev-list', '--parents', '--max-count=' + str(MAX_EVENTS + 1), head).splitlines()
    checked(len(commits) == len(entries) // 2, 'journal history count')
    previous = None
    for sequence, line in enumerate(reversed(commits), 1):
        ids = line.decode('ascii').split()
        checked(len(ids) == (2 if previous else 1), 'journal must have a single-parent history')
        for oid in ids: hex_id(oid, 40)
        checked(previous is None or ids[1] == previous, 'journal history parent mismatch')
        changes = git(repo, 'diff-tree', '--root', '--no-commit-id', '--no-renames', '--no-abbrev', '-r', '--raw', '-z', ids[0]).split(b'\0')
        checked(len(changes) == 5 and changes[-1] == b'', 'journal commit must add exactly two files')
        expected_names = {f'{sequence:08}.event.json', f'{sequence:08}.evidence.json'}
        for index in (0, 2):
            fields = changes[index].decode('ascii').split()
            name = changes[index + 1].decode('ascii')
            checked(name in expected_names, 'journal changed an earlier or unknown record')
            expected_names.remove(name)
            checked(fields == [':000000', '100644', '0' * 40, entries[name], 'A'], 'journal history is not append-only')
        previous = ids[0]


def read(repo):
    repository(repo)
    raw = git(repo, 'rev-parse', '--verify', '--quiet', REF, absent=True)
    if raw is None:
        return Snapshot(None, (), ()), {}
    head = raw.decode('ascii').strip(); hex_id(head, 40)
    checked(git(repo, 'cat-file', '-t', head).strip() == b'commit', 'journal ref is not a commit')
    checked(int(git(repo, 'cat-file', '-s', head + '^{tree}')) <= MAX_EVENTS * 256, 'journal tree size limit')
    tree = git(repo, 'ls-tree', '-z', head).split(b'\0')
    entries = {}
    for row in filter(None, tree):
        header, name = row.split(b'\t', 1)
        mode, kind, oid = header.split()
        checked(mode == b'100644' and kind == b'blob', 'journal requires regular blobs')
        name = name.decode('ascii'); oid = oid.decode('ascii'); hex_id(oid, 40)
        checked(name not in entries, 'duplicate journal path'); entries[name] = oid
    checked(0 < len(entries) <= MAX_EVENTS * 2 and len(entries) % 2 == 0, 'journal event count')
    history(repo, head, entries)
    events, evidence = [], []
    total = 0
    for sequence in range(1, len(entries) // 2 + 1):
        event_path = f'{sequence:08}.event.json'
        proof_path = f'{sequence:08}.evidence.json'
        checked(event_path in entries and proof_path in entries, 'journal sequence gap or unknown path')
        event_bytes = blob(repo, entries[event_path], 4096)
        proof = blob(repo, entries[proof_path], MAX_EVIDENCE)
        total += len(event_bytes) + len(proof); checked(total <= MAX_TOTAL, 'journal total limit')
        record = decode(event_bytes)
        checked(isinstance(record, dict) and type(record.get('version')) is int and record['version'] == 1,
                'journal version')
        checked(type(record.get('sequence')) is int and record['sequence'] == sequence, 'journal sequence')
        expected = {'version', 'sequence', 'evidence_sha256', *Event.__dataclass_fields__}
        checked(set(record) == expected, 'journal event fields')
        checked(event_bytes == encode(record), 'noncanonical journal envelope')
        event = Event(**{key: record[key] for key in Event.__dataclass_fields__}); event.validate()
        checked(record['evidence_sha256'] == digest(proof), 'journal evidence digest')
        checked(isinstance(decode(proof), dict), 'journal evidence must be an object')
        events.append(event); evidence.append(proof)
    return Snapshot(head, tuple(events), tuple(evidence)), entries


def load(repo):
    return read(Path(repo))[0]


def append(repo, expected_head, event, evidence):
    repo = Path(repo)
    if expected_head is not None: hex_id(expected_head, 40)
    checked(isinstance(event, Event), 'typed Event required'); event.validate()
    checked(isinstance(evidence, bytes) and 0 < len(evidence) <= MAX_EVIDENCE, 'journal evidence size')
    checked(isinstance(decode(evidence), dict), 'journal evidence must be an object')
    before, entries = read(repo)
    checked(before.head == expected_head, 'stale journal head')
    sequence = len(before.events) + 1
    checked(sequence <= MAX_EVENTS, 'journal event limit')
    event_bytes = encode(event.record(sequence, digest(evidence)))
    # Account for event envelopes as well as attached evidence before any ref change.
    total = sum(len(encode(e.record(i + 1, digest(p)))) + len(p)
                for i, (e, p) in enumerate(zip(before.events, before.evidence)))
    checked(total + len(event_bytes) + len(evidence) <= MAX_TOTAL, 'journal total limit')
    for name, data in [(f'{sequence:08}.event.json', event_bytes), (f'{sequence:08}.evidence.json', evidence)]:
        entries[name] = git(repo, 'hash-object', '-w', '--stdin', data=data).decode('ascii').strip()
    tree_input = b''.join(f'100644 blob {oid}\t{name}\0'.encode('ascii') for name, oid in sorted(entries.items()))
    tree = git(repo, 'mktree', '-z', data=tree_input).decode('ascii').strip()
    parents = ['-p', expected_head] if expected_head else []
    commit = git(repo, 'commit-tree', tree, *parents,
                 data=f'Pages journal event {sequence}: {event.kind}\n'.encode('ascii')).decode('ascii').strip()
    # All existing blobs are retained. The new commit has exactly the expected
    # parent; the old-value argument also rejects writers racing after read().
    git(repo, 'update-ref', '--no-deref', REF, commit, expected_head or '0' * 40)
    return commit
