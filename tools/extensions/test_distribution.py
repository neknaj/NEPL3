"""Distribution boundary and lock preservation tests."""
import tomllib
import unittest

from tools.extensions.distribution import MEMBERS, check_lock, workspace_manifest


class DistributionTests(unittest.TestCase):
    def test_only_used_dependencies_and_inherited_settings_are_retained(self):
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
        result = tomllib.loads(workspace_manifest(source, [crate]))["workspace"]
        self.assertEqual(result["members"], list(MEMBERS))
        self.assertEqual(set(result["dependencies"]), {"nepl3-core", "unicode-ident"})
        self.assertEqual(result["package"], {"edition": "2024", "publish": False})
        self.assertEqual(result["lints"]["rust"]["unsafe_code"], "forbid")

    def test_dependency_outside_foundation_is_rejected(self):
        for path in ["tools", "../other", "crates/languages/doc/core"]:
            source = f'[workspace.dependencies]\nother = {{ path = "{path}" }}\n'
            with self.subTest(path=path), self.assertRaises(ValueError):
                workspace_manifest(source, ['[dependencies]\nother.workspace = true\n'])
        with self.assertRaises(ValueError):
            workspace_manifest('[workspace.dependencies]\n',
                               ['[dependencies]\nother = { path = "../other" }\n'])

    def test_lock_allows_pruning_but_rejects_version_checksum_and_dependency_changes(self):
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
    unittest.main()
