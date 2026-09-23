"""Distribution boundary and lock preservation tests."""
import json
import re
import unittest
from pathlib import Path
import tempfile
from unittest.mock import call, patch
from collections.abc import Callable

from tools.serialization.json import JsonValue
from tools.serialization.toml import decode, table

from tools.extensions.distribution import (
    DOC_MEMBERS, MEMBERS, SUPPORT, check_foundation, check_lock, check_metadata,
    export, export_doc, workspace_manifest,
)


class DistributionTests(unittest.TestCase):
    @staticmethod
    def root_fixture(root: Path) -> bytes:
        """Valid input reaches Cargo metadata when every boundary is permitted."""
        _ = (root / "Cargo.toml").write_text(
            '[workspace]\nresolver = "3"\n[workspace.dependencies]\n', encoding="utf-8",
        )
        _ = (root / "Cargo.lock").write_text('version = 4\npackage = []\n', encoding="utf-8")
        _ = (root / "rust-toolchain.toml").write_text(
            '[toolchain]\nchannel = "1.97.0"\n', encoding="utf-8",
        )
        _ = (root / "LICENSE").write_text("fixture license\n", encoding="utf-8")
        tracked: list[str] = []
        for member in MEMBERS:
            directory = root / member
            (directory / "src").mkdir(parents=True)
            _ = (directory / "Cargo.toml").write_text(
                f'[package]\nname = "nepl3-{directory.name}"\nversion = "0.1.0"\n'
                + 'edition = "2024"\n', encoding="utf-8",
            )
            _ = (directory / "src/lib.rs").write_text("pub fn fixture() {}\n", encoding="utf-8")
            tracked.extend((f"{member}/Cargo.toml", f"{member}/src/lib.rs"))
        return ("\0".join(tracked) + "\0").encode("utf-8")

    def test_valid_root_inputs_reach_cargo_metadata(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            tracked = self.root_fixture(root)
            output = root / "output"
            metadata = {"packages": [{"source": None,
                "manifest_path": str(output / member / "Cargo.toml"),
                "targets": [{"src_path": str(output / member / "src/lib.rs")}]}
                for member in MEMBERS]}
            with patch("tools.extensions.distribution.subprocess.check_output",
                       side_effect=[tracked, json.dumps(metadata).encode("utf-8")]) as commands:
                self.assertEqual(export(root, output), output)
                self.assertEqual(commands.call_args_list, [
                    call(["git", "ls-files", "-z", "--", *MEMBERS], cwd=root),
                    call(["cargo", "+1.97.0", "metadata", "--offline", "--format-version", "1"],
                         cwd=output),
                ])
            manifest = decode((output / "Cargo.toml").read_text(encoding="utf-8"))
            self.assertEqual(table(manifest["workspace"])["members"], list(MEMBERS))
            for name in tracked.decode("utf-8").split("\0"):
                if name:
                    self.assertEqual((output / name).read_bytes(), (root / name).read_bytes())

    def test_metadata_checks_target_sources_and_manifests(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary).resolve()
            allowed = root / "allowed"
            package: dict[str, JsonValue] = {"source": None, "manifest_path": str(allowed / "Cargo.toml"),
                       "targets": [{"src_path": str(allowed / "src/lib.rs")}]}
            check_metadata({"packages": [package]}, [allowed])
            package["targets"] = [{"src_path": str(root / "original/lib.rs")}]
            with self.assertRaises(ValueError):
                check_metadata({"packages": [package]}, [allowed])
            package["targets"] = []
            package["manifest_path"] = str(root / "original/Cargo.toml")
            with self.assertRaises(ValueError):
                check_metadata({"packages": [package]}, [allowed])

    def test_foundation_requires_identical_files_and_refuses_symlinks(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root, external = base / "source", base / "external"
            relative = Path(MEMBERS[0]) / "src/lib.rs"
            for parent in (root, external):
                (parent / relative).parent.mkdir(parents=True)
                _ = (parent / relative).write_bytes(b"original")
            with patch("tools.extensions.distribution.subprocess.check_output",
                       return_value=(relative.as_posix() + "\0").encode()):
                check_foundation(root, external)
                _ = (external / relative).write_bytes(b"changed")
                with self.assertRaises(ValueError):
                    check_foundation(root, external)
                _ = (external / relative).write_bytes(b"original")
                is_symlink: Callable[[Path], bool] = lambda path: path == external / relative
                with patch.object(Path, "is_symlink", is_symlink):
                    with self.assertRaises(ValueError):
                        check_foundation(root, external)
                extra = external / MEMBERS[0] / "extra.rs"
                _ = extra.write_bytes(b"extra")
                with self.assertRaises(ValueError):
                    check_foundation(root, external)

    def test_doc_members_use_only_explicit_external_foundation(self) -> None:
        source = '''[workspace.dependencies]
nepl3-core = { path = "crates/foundation/core" }
nepl3-doc-core = { path = "crates/languages/doc/core" }
unused = { path = "tools" }
'''
        manifest = '''[dependencies]
nepl3-core.workspace = true
nepl3-doc-core.workspace = true
'''
        path = Path('/external/𠮷田/"foundation/core')
        output = table(decode(workspace_manifest(
            source, [manifest], members=DOC_MEMBERS,
            external={"crates/foundation/core": path},
        ))["workspace"])
        self.assertEqual(output["members"], list(DOC_MEMBERS))
        dependencies = table(output["dependencies"])
        self.assertEqual(table(dependencies["nepl3-core"])["path"], path.as_posix())
        self.assertEqual(table(dependencies["nepl3-doc-core"])["path"], DOC_MEMBERS[0])
        self.assertNotIn("unused", dependencies)
        with self.assertRaises(ValueError):
            _ = workspace_manifest(source, [manifest], members=DOC_MEMBERS)
        with self.assertRaises(ValueError):
            _ = workspace_manifest(source, [manifest])

    def test_doc_refuses_monorepo_as_foundation(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            with self.assertRaises(ValueError):
                _ = export_doc(root, root / "output", root)
            self.assertFalse((root / "output").exists())

    def test_doc_refuses_missing_or_wrong_foundation_packages(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            base = Path(temporary)
            root = base / "source"
            foundation = base / "foundation"
            output = base / "doc"
            with self.assertRaises(ValueError):
                _ = export_doc(root, output, foundation)
            directory = foundation / MEMBERS[0]
            directory.mkdir(parents=True)
            _ = (directory / "Cargo.toml").write_text(
                '[package]\nname = "wrong"\n', encoding="utf-8",
            )
            with self.assertRaises(ValueError):
                _ = export_doc(root, output, foundation)
            self.assertFalse(output.exists())

    def test_every_root_input_is_checked_before_output_is_created(self) -> None:
        inputs = (*SUPPORT, "Cargo.toml", *(f"{m}/Cargo.toml" for m in MEMBERS))
        for name in inputs:
            with self.subTest(name=name), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary).resolve()
                tracked = self.root_fixture(root)
                rejected = root / name
                output = root / "output"
                # Inject the filesystem predicate without requiring Windows
                # symlink privileges. A parser error cannot satisfy this reason.
                is_symlink: Callable[[Path], bool] = lambda path: path == rejected
                with patch("tools.extensions.distribution.subprocess.check_output",
                           side_effect=[tracked, AssertionError("Cargo metadata ran before rejection")]) as commands, \
                        patch.object(Path, "is_symlink", is_symlink):
                    reason = f"source path escapes distribution: {Path(name)}"
                    with self.assertRaisesRegex(ValueError, "^" + re.escape(reason) + "$"):
                        _ = export(root, output)
                    commands.assert_called_once_with(
                        ["git", "ls-files", "-z", "--", *MEMBERS], cwd=root,
                    )
                    self.assertFalse(output.exists())

    def test_only_used_dependencies_and_inherited_settings_are_retained(self) -> None:
        source = '''[workspace]
members = ["tools"]
resolver = "3"
[workspace.package]
edition = "2024"
publish = false
[workspace.dependencies]
nepl3-core = { path = "crates/foundation/core" }
unused = { path = "tools" }
unicode-ident = { version = "=1.0.18", default-features = false }
[workspace.lints.rust]
unsafe_code = "forbid"
'''
        crate = '''[dependencies]
nepl3-core.workspace = true
[target.'cfg(unix)'.dev-dependencies]
unicode-ident.workspace = true
'''
        result = table(decode(workspace_manifest(source, [crate]))["workspace"])
        self.assertEqual(result["members"], list(MEMBERS))
        self.assertEqual(set(table(result["dependencies"])), {"nepl3-core", "unicode-ident"})
        self.assertEqual(result["package"], {"edition": "2024", "publish": False})
        self.assertEqual(table(table(result["lints"])["rust"])["unsafe_code"], "forbid")

    def test_dependency_outside_foundation_is_rejected(self) -> None:
        for path in ["tools", "../other", "crates/languages/doc/core"]:
            source = f'[workspace.dependencies]\nother = {{ path = "{path}" }}\n'
            with self.subTest(path=path), self.assertRaises(ValueError):
                _ = workspace_manifest(source, ['[dependencies]\nother.workspace = true\n'])
        with self.assertRaises(ValueError):
            _ = workspace_manifest('[workspace.dependencies]\n',
                               ['[dependencies]\nother = { path = "../other" }\n'])

    def test_lock_allows_pruning_but_rejects_version_checksum_and_dependency_changes(self) -> None:
        original = '''version = 4
[[package]]
name = "kept"
version = "1.0.0"
checksum = "abc"
dependencies = ["leaf"]
[[package]]
name = "removed"
version = "1.0.0"
'''
        retained = '''version = 4
[[package]]
name = "kept"
version = "1.0.0"
checksum = "abc"
dependencies = ["leaf"]
'''
        check_lock(original, retained)
        for old, new in [('1.0.0', '1.0.1'), ('abc', 'changed'), ('leaf', 'other')]:
            with self.subTest(field=old), self.assertRaises(ValueError):
                check_lock(original, retained.replace(old, new))


if __name__ == "__main__":
    _ = unittest.main()
