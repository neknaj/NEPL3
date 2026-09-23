from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

from journal import append, load
from journal import remote, store
from test_journal import event, initialize


class RemoteJournalTests(unittest.TestCase):
    def test_changed_remote_after_push_is_not_confirmed_or_rolled_back(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, (a, _) = self.setup_repositories(directory)
            first = append(a, None, event(), b'{}')
            original = store.git
            successor: list[str] = []
            def advance_after_push(repo: Path, *args: str) -> bytes:
                result = original(repo, *args)
                if args and args[0] == 'push':
                    successor.append(append(server, first, event('later-writer'), b'{}'))
                return result
            with patch.object(remote, 'git', side_effect=advance_after_push), self.assertRaisesRegex(ValueError, 'state unknown'):
                _ = remote.publish(a, str(server), None, first)
            self.assertEqual(load(server).head, successor[0])

    def test_follow_tags_configuration_does_not_publish_other_refs(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, (a, _) = self.setup_repositories(directory)
            first = append(a, None, event(), b'{}')
            _ = store.git(a, 'tag', '-a', 'unrelated-tag', first, '-m', 'Keep local')
            _ = store.git(a, 'config', 'push.followTags', 'true')
            _ = remote.publish(a, str(server), None, first)
            self.assertIsNone(store.git(server, 'show-ref', '--tags', absent=True))
            self.assertEqual(load(server).head, first)

    def setup_repositories(self, directory: str) -> tuple[Path, tuple[Path, Path]]:
        base = Path(directory).resolve()
        server = base / 'server.git'; initialize(server)
        mirrors = (base / 'a.git', base / 'b.git')
        for repo in mirrors:
            initialize(repo)
            _ = store.git(repo, 'remote', 'add', 'origin', str(server))
        return server, mirrors

    def test_actual_push_confirm_and_lost_ack_reconciliation(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, (a, b) = self.setup_repositories(directory)
            first = append(a, None, event(), b'{}')
            receipt = remote.publish(a, str(server), None, first)
            self.assertEqual(load(server).head, first)
            self.assertFalse(receipt.already_present)
            self.assertFalse(receipt.publication_verified)
            calls: list[tuple[str, ...]] = []
            original = store.git
            def record(repo: Path, *args: str) -> bytes:
                calls.append(args)
                return original(repo, *args)
            with patch.object(remote, 'git', record):
                replay = remote.publish(a, str(server), None, first)
                self.assertTrue(replay.already_present)
                self.assertFalse(any('push' in args for args in calls))
            _ = store.git(b, 'fetch', 'origin', store.REF + ':' + store.REF)
            second = append(b, first, event('next'), b'{}')
            _ = remote.publish(b, str(server), first, second)
            self.assertEqual(len(load(server).events), 2)
            # A stale writer cannot replace the remote winner.
            rival = append(a, first, event('rival'), b'{}')
            with self.assertRaisesRegex(ValueError, 'remote journal changed'):
                _ = remote.publish(a, str(server), first, rival)
            self.assertEqual(load(server).head, second)

    def test_competitor_between_observation_and_push_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, (a, b) = self.setup_repositories(directory)
            first = append(a, None, event(), b'{}'); _ = remote.publish(a, str(server), None, first)
            _ = store.git(b, 'fetch', 'origin', store.REF + ':' + store.REF)
            loser = append(a, first, event('loser'), b'{}')
            winner = append(b, first, event('winner'), b'{}')
            original = store.git
            def racing_git(repo: Path, *args: str) -> bytes:
                if args and args[0] == 'push':
                    _ = store.git(b, 'push', 'origin', winner + ':' + store.REF)
                return original(repo, *args)
            with patch.object(remote, 'git', side_effect=racing_git), self.assertRaises(ValueError):
                _ = remote.publish(a, str(server), first, loser)
            self.assertEqual(load(server).head, winner)

    def test_different_push_destination_is_rejected_without_write(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            server, (a, b) = self.setup_repositories(directory)
            first = append(a, None, event(), b'{}')
            _ = store.git(a, 'remote', 'set-url', '--push', 'origin', str(b))
            with self.assertRaisesRegex(ValueError, 'pinned fetch/push URL'):
                _ = remote.publish(a, str(server), None, first)
            self.assertIsNone(load(server).head)
            self.assertIsNone(load(b).head)


if __name__ == '__main__': _ = unittest.main()
