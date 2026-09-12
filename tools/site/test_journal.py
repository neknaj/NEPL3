from concurrent.futures import ThreadPoolExecutor
from dataclasses import replace
from pathlib import Path
import subprocess
import tempfile
import threading
import unittest
from unittest.mock import patch

from journal import Event, append, load
from journal import store
from journal.model import encode
from payload import digest


def event(transaction='transaction-1', kind='DeployIntent'):
    return Event(kind, transaction, 1234, 1, 'a' * 40, 'b' * 64)


def initialize(path, bare=True):
    subprocess.run(['git', 'init', '--quiet', *(['--bare'] if bare else []), str(path)], check=True)


class JournalTests(unittest.TestCase):
    def test_shallow_boundary_cannot_hide_an_invalid_parent(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve() / 'journal.git'; initialize(repo)
            first = append(repo, None, event(), b'{}')
            tree = store.git(repo, 'rev-parse', first + '^{tree}').decode().strip()
            second = store.git(repo, 'commit-tree', tree, '-p', first, data=b'No event added\n').decode().strip()
            store.git(repo, 'update-ref', store.REF, second, first)
            with self.assertRaisesRegex(ValueError, 'history count'): load(repo)
            (repo / 'shallow').write_text(second + '\n', encoding='ascii')
            with self.assertRaisesRegex(ValueError, 'shallow journal'): load(repo)
            with self.assertRaisesRegex(ValueError, 'shallow journal'): append(repo, second, event(), b'{}')

    def test_fast_forward_rewriting_old_event_is_rejected_on_reload(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve() / 'journal.git'; initialize(repo)
            first = append(repo, None, event(), b'{}')
            _, entries = store.read(repo)
            # This is a real descendant commit with a valid current tree, but
            # it changes the already recorded transaction while adding event 2.
            for name, data in [('00000001.event.json', encode(event('rewritten').record(1, digest(b'{}')))),
                               ('00000002.event.json', encode(event('new').record(2, digest(b'{}')))),
                               ('00000002.evidence.json', b'{}')]:
                entries[name] = store.git(repo, 'hash-object', '-w', '--stdin', data=data).decode().strip()
            tree = store.git(repo, 'mktree', '-z', data=b''.join(
                f'100644 blob {oid}\t{name}\0'.encode() for name, oid in sorted(entries.items()))).decode().strip()
            altered = store.git(repo, 'commit-tree', tree, '-p', first, data=b'Invalid append\n').decode().strip()
            store.git(repo, 'update-ref', store.REF, altered, first)
            with self.assertRaisesRegex(ValueError, 'not append-only'): load(repo)
            with self.assertRaises(ValueError): append(repo, altered, event(), b'{}')
            self.assertEqual(store.git(repo, 'rev-parse', store.REF).decode().strip(), altered)

    def test_real_git_reload_retains_exact_evidence_and_parent(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve() / 'journal.git'; initialize(repo)
            self.assertIsNone(load(repo).head)
            first = append(repo, None, event(), b'{"note":"before API call"}\n')
            second = append(repo, first, event(kind='DeployReceipt'), b'{"deployment_id":"42"}')
            result = load(repo)
            self.assertEqual(result.head, second)
            self.assertEqual([e.kind for e in result.events], ['DeployIntent', 'DeployReceipt'])
            self.assertEqual(result.evidence, (b'{"note":"before API call"}\n', b'{"deployment_id":"42"}'))
            self.assertEqual(store.git(repo, 'rev-parse', second + '^').decode().strip(), first)
            # A new reader sees the saved intent even if no receipt was ever written.
            self.assertIn(b'DeployIntent', store.git(repo, 'show', first + ':00000001.event.json'))
            with self.assertRaisesRegex(ValueError, 'stale journal'):
                append(repo, first, event('stale'), b'{}')
            self.assertEqual(load(repo).head, second)

    def test_racing_writers_have_one_winner_without_lost_events(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve() / 'journal.git'; initialize(repo)
            first = append(repo, None, event(), b'{}')
            read = store.read; barrier = threading.Barrier(2)
            def racing_read(path):
                result = read(path); barrier.wait(timeout=10); return result
            def writer(name):
                try: return append(repo, first, event(name), b'{}')
                except ValueError: return None
            with patch.object(store, 'read', side_effect=racing_read), ThreadPoolExecutor(max_workers=2) as pool:
                results = list(pool.map(writer, ['writer-A', 'writer-B']))
            self.assertEqual(sum(x is not None for x in results), 1)
            final = load(repo)
            self.assertEqual(len(final.events), 2)
            self.assertIn(final.head, results)
            self.assertEqual(final.events[0], event())

    def test_invalid_boundary_does_not_change_ref(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve() / 'journal.git'; initialize(repo)
            first = append(repo, None, event(), b'{}')
            for invalid in [replace(event(), run_id=True), replace(event(), attempt=0),
                            replace(event(), kind='SuccessStub'), replace(event(), source_commit='HEAD')]:
                with self.subTest(event=invalid), self.assertRaises(ValueError): append(repo, first, invalid, b'{}')
            for proof in [b'[]', b'{"a":1,"a":2}', b'{"a":NaN}', b'x' * 65537]:
                with self.subTest(proof=proof[:20]), self.assertRaises(ValueError): append(repo, first, event(), proof)
            self.assertEqual(load(repo).head, first)
            with patch.object(store, 'MAX_EVENTS', 1), self.assertRaisesRegex(ValueError, 'event limit'):
                append(repo, first, event(), b'{}')
            self.assertEqual(load(repo).head, first)

    def test_nonbare_source_repo_is_not_modified(self):
        with tempfile.TemporaryDirectory() as directory:
            repo = Path(directory).resolve() / 'source'; initialize(repo, bare=False)
            (repo / 'keep').write_bytes(b'user work')
            with self.assertRaisesRegex(ValueError, 'bare mirror'): append(repo, None, event(), b'{}')
            self.assertEqual((repo / 'keep').read_bytes(), b'user work')
            self.assertFalse((repo / '.git/refs/heads/pages-state').exists())


if __name__ == '__main__': unittest.main()
