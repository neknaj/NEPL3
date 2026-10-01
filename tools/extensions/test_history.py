"""Fixed Git consumer extraction, including hostile path/object failure cases."""
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools.extensions import history
from tools.extensions.execution import Record, Scope

COMMIT = "a" * 40
OID = "b" * 40


class HistoryTests(unittest.TestCase):
    def test_only_full_commit_ids_are_accepted(self) -> None:
        self.assertEqual(history.revision(COMMIT), COMMIT)
        for value in ("HEAD", "main", "a" * 7, "A" * 40, "-" + "a" * 39, COMMIT + "^{tree}"):
            with self.subTest(value=value), self.assertRaises(ValueError):
                _ = history.revision(value)

    def test_real_git_ignores_working_tree_changes(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "repo"
            root.mkdir()
            def git(*args: str) -> bytes:
                return subprocess.check_output(["git", *args], cwd=root, stderr=subprocess.PIPE)
            _ = git("init")
            fixture = root / history.PREFIX
            (fixture / "src").mkdir(parents=True)
            for path, contents in (("Cargo.toml", "old manifest"), ("Cargo.lock", "old lock"), ("src/lib.rs", "old API")):
                _ = (fixture / path).write_text(contents, encoding="utf-8")
            _ = git("add", ".")
            _ = git("-c", "user.name=Fixture", "-c", "user.email=fixture@example.invalid", "-c", "commit.gpgsign=false", "commit", "-m", "fixed baseline")
            commit = git("rev-parse", "HEAD").decode().strip()
            _ = (fixture / "src/lib.rs").write_text("new API hides break", encoding="utf-8")
            out = history.materialize(root, Path(temporary) / "fixed", commit)
            self.assertEqual((out / "src/lib.rs").read_text(), "old API")
            self.assertEqual((out / "Cargo.lock").read_text(), "old lock")
            with self.assertRaises(FileExistsError):
                _ = history.materialize(root, out, commit)
            tree = git("rev-parse", "HEAD^{tree}").decode().strip()
            with self.assertRaises(subprocess.CalledProcessError):
                _ = history.materialize(root, Path(temporary) / "tree", tree)
            with self.assertRaises(subprocess.CalledProcessError):
                _ = history.materialize(root, Path(temporary) / "missing", "0" * 40)

    def test_unsafe_entries_are_rejected_before_destination_creation(self) -> None:
        cases = [
            ("120000", "blob", "src/link", "1"),
            ("160000", "commit", "src/submodule", "-"),
            ("100644", "blob", "../escape", "1"),
            ("100644", "blob", ".. /escape", "1"),
            ("100644", "blob", "src\\escape", "1"),
            ("100644", "blob", ".git/config", "1"),
            ("100644", "blob", "CON", "1"),
            ("100644", "blob", "src/x", str(history.MAX_FILE_BYTES + 1)),
        ]
        for mode, kind, path, size in cases:
            with self.subTest(path=path, mode=mode), tempfile.TemporaryDirectory() as temporary:
                destination = Path(temporary) / "out"
                entry = f"{mode} {kind} {OID} {size}\t{history.PREFIX}{path}\0".encode()
                with patch.object(history, "git", side_effect=[COMMIT.encode(), entry]), self.assertRaises(ValueError):
                    _ = history.materialize(Path(temporary), destination, COMMIT)
                self.assertFalse(destination.exists())

    def test_case_collisions_and_missing_required_inputs_fail(self) -> None:
        for paths in (("src/A.rs", "src/a.rs"), ("README.md",)):
            with self.subTest(paths=paths), tempfile.TemporaryDirectory() as temporary:
                listing = b"".join(f"100644 blob {OID} 1\t{history.PREFIX}{path}\0".encode() for path in paths)
                with patch.object(history, "git", side_effect=[COMMIT.encode(), listing]), self.assertRaises(ValueError):
                    _ = history.materialize(Path(temporary), Path(temporary) / "out", COMMIT)

    def test_failure_record_retains_fixed_revision_without_wire_claim(self) -> None:
        record = Record(Scope.DIRECT, "new", {}, (), False, True, None, COMMIT).representation()
        self.assertEqual(record["result"], "failed")
        self.assertEqual(record["consumer_revision"], COMMIT)
        self.assertIn("wire compatibility", str(record["compatibility_kind"]))


if __name__ == "__main__":
    _ = unittest.main()
