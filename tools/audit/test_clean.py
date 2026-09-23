"""Exercise the generator gate against real Git index/working-tree states."""

from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from typing import override


SCRIPT = Path(__file__).with_name("clean.py").resolve()


class CleanTests(unittest.TestCase):
    def __init__(self, methodName: str = "runTest") -> None:
        super().__init__(methodName)
        self.temporary: tempfile.TemporaryDirectory[str] | None = None

    @property
    def root(self) -> Path:
        if self.temporary is None:
            raise RuntimeError("checkout fixture has not been created")
        return Path(self.temporary.name)

    @override
    def setUp(self) -> None:
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        _ = self.git("init", "--quiet")
        _ = self.git("config", "user.name", "Gate test")
        _ = self.git("config", "user.email", "gate@example.invalid")
        _ = self.git("config", "core.autocrlf", "false")
        _ = (self.root / "source.txt").write_text("original\n", encoding="utf-8")
        _ = (self.root / ".gitignore").write_text("dist/\n", encoding="utf-8")
        _ = self.git("add", ".")
        _ = self.git("commit", "--quiet", "-m", "Initial source")

    def git(self, *args: str) -> bytes:
        return subprocess.run(
            ["git", *args], cwd=self.root, check=True, capture_output=True
        ).stdout

    def gate(self) -> subprocess.CompletedProcess[bytes]:
        return subprocess.run(
            [sys.executable, str(SCRIPT)], cwd=self.root, capture_output=True
        )

    def assert_rejected_without_mutation(self) -> None:
        before = self.git("status", "--porcelain=v1", "--untracked-files=all")
        index = self.git("diff", "--cached", "--binary")
        self.assertEqual(self.gate().returncode, 1)
        self.assertEqual(before, self.git("status", "--porcelain=v1", "--untracked-files=all"))
        self.assertEqual(index, self.git("diff", "--cached", "--binary"))

    def test_clean_and_ignored_build_outputs_pass(self) -> None:
        self.assertEqual(self.gate().returncode, 0)
        (self.root / "dist").mkdir()
        _ = (self.root / "dist" / "preview.html").write_text("preview", encoding="utf-8")
        self.assertEqual(self.gate().returncode, 0)

    def test_unstaged_edit_is_rejected(self) -> None:
        _ = (self.root / "source.txt").write_text("changed\n", encoding="utf-8")
        self.assert_rejected_without_mutation()

    def test_staged_edit_is_rejected_even_when_plain_diff_passes(self) -> None:
        _ = (self.root / "source.txt").write_text("changed\n", encoding="utf-8")
        _ = self.git("add", "source.txt")
        _ = self.git("diff", "--exit-code")
        self.assert_rejected_without_mutation()

    def test_untracked_output_is_rejected_even_when_plain_diff_passes(self) -> None:
        _ = (self.root / "new source.txt").write_text("new\n", encoding="utf-8")
        _ = self.git("config", "status.showUntrackedFiles", "no")
        _ = self.git("diff", "--exit-code")
        self.assert_rejected_without_mutation()
        self.assertTrue((self.root / "new source.txt").exists())

    def test_staged_change_with_worktree_restored_to_head_is_rejected(self) -> None:
        _ = (self.root / "source.txt").write_text("changed\n", encoding="utf-8")
        _ = self.git("add", "source.txt")
        _ = (self.root / "source.txt").write_text("original\n", encoding="utf-8")
        _ = self.git("diff", "HEAD", "--exit-code")
        self.assert_rejected_without_mutation()

    def test_git_error_is_not_clean(self) -> None:
        with tempfile.TemporaryDirectory() as empty:
            result = subprocess.run(
                [sys.executable, str(SCRIPT)], cwd=empty, capture_output=True
            )
        self.assertNotEqual(result.returncode, 0)


if __name__ == "__main__":
    _ = unittest.main()
