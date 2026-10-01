"""Real Rust positive/negative control for the fixed-source compatibility gate."""
from pathlib import Path
import shutil
import os
import subprocess
import tempfile
import unittest

from tools.extensions import history
from tools.extensions import cargo as cargo_input
from tools.extensions.distribution import export
from tools.extensions.manifests import external_manifest

ROOT = Path(__file__).resolve().parents[2]
BASELINE = "57ce1a006cd5017a340af0f4eea804308885a63d"
TOOLCHAIN = cargo_input.toolchain((ROOT / "rust-toolchain.toml").read_text(encoding="utf-8"))


class RustCompatibilityTests(unittest.TestCase):
    def test_fixed_consumer_detects_api_break_hidden_by_current_consumer_update(self) -> None:
        with tempfile.TemporaryDirectory(prefix="nepl3-compatibility-") as temporary:
            directory = Path(temporary)
            foundation = export(ROOT, directory / "foundation")
            frozen = history.materialize(ROOT, directory / "frozen", BASELINE)
            packages = {f"nepl3-{name}": foundation / "crates/foundation" / name
                        for name in ("core", "reader", "engine", "wire")}
            manifest = frozen / "Cargo.toml"
            _ = manifest.write_text(external_manifest(manifest.read_text(encoding="utf-8"), packages), encoding="utf-8")
            def check(consumer: Path) -> subprocess.CompletedProcess[bytes]:
                environment = dict(os.environ)
                environment["CARGO_BUILD_BUILD_DIR"] = str(consumer / "target/intermediate")
                return subprocess.run(["cargo", f"+{TOOLCHAIN}", "check", "--locked", "--all-targets", "--color", "never", "--target-dir", str(consumer / "target")],
                                      cwd=consumer, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                      timeout=600, check=False, env=environment)
            original = check(frozen)
            self.assertEqual(original.returncode, 0, original.stderr.decode(errors="replace"))
            # Source-only public path break. Internal module and wire schemas
            # remain unchanged; an alternate public facade lets a current
            # consumer adapt while the fixed old consumer must still fail.
            entry = foundation / "crates/foundation/engine/src/lib.rs"
            code = entry.read_text(encoding="utf-8")
            self.assertEqual(code.count("pub mod package;"), 1)
            _ = entry.write_text(code.replace("pub mod package;", "mod package;\npub mod compatibility_package { pub use crate::package::*; }"), encoding="utf-8")
            broken = check(frozen)
            self.assertNotEqual(broken.returncode, 0)
            self.assertIn(b"E0603", broken.stderr)
            self.assertIn(b"package", broken.stderr)
            current = directory / "current"
            _ = shutil.copytree(frozen, current, ignore=shutil.ignore_patterns("target"))
            replaced = 0
            for path in current.rglob("*.rs"):
                text = path.read_text(encoding="utf-8")
                replaced += text.count("nepl3_engine::package")
                _ = path.write_text(text.replace("nepl3_engine::package", "nepl3_engine::compatibility_package"), encoding="utf-8")
            self.assertGreater(replaced, 0)
            adapted = check(current)
            self.assertEqual(adapted.returncode, 0, adapted.stderr.decode(errors="replace"))
            # Re-running the pinned input still fails after current adaptation.
            self.assertNotEqual(check(frozen).returncode, 0)


if __name__ == "__main__":
    _ = unittest.main()
