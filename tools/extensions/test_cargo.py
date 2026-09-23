"""Cargo input projections preserve lock equality and local-source boundaries."""

from pathlib import Path
import unittest

from tools.extensions.cargo import LocalPackage, local_packages, lock_packages
from tools.extensions.distribution import check_lock
from tools.serialization.json import decode


class CargoTests(unittest.TestCase):
    def test_metadata_projects_local_paths_and_checks_remote_source_type(self) -> None:
        value = decode('''{"packages": [
          {"source": "registry+https://example.invalid", "manifest_path": "ignored"},
          {"source": null, "manifest_path": "crate/Cargo.toml", "targets": [{"src_path": "crate/src/lib.rs"}]}
        ]}''')
        self.assertEqual(local_packages(value), (LocalPackage(Path("crate/Cargo.toml"), (Path("crate/src/lib.rs"),)),))
        for source in ('{"packages":[{"source":true}]}',
                       '{"packages":[{"source":null,"manifest_path":42,"targets":[]}]}',
                       '{"packages":[{"source":null,"manifest_path":"x","targets":[{"src_path":false}]}]}'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                _ = local_packages(decode(source))

    def test_lock_presence_and_dependency_order_are_part_of_equality(self) -> None:
        base = '[[package]]\nname="p"\nversion="1"\n'
        check_lock(base, base)
        for suffix in ('dependencies=[]\n', 'source="registry+x"\n', 'checksum="abc"\n', 'replace="q"\n'):
            with self.subTest(suffix=suffix), self.assertRaises(ValueError):
                check_lock(base, base + suffix)
        with self.assertRaises(ValueError):
            check_lock(base + 'dependencies=["a","b"]\n', base + 'dependencies=["b","a"]\n')

    def test_unknown_lock_fields_are_rejected_instead_of_discarded(self) -> None:
        # An unsupported record must not compare equal after dropping a field.
        base = '[[package]]\nname="p"\nversion="1"\n'
        self.assertEqual(len(lock_packages(base)), 1)
        with self.assertRaisesRegex(ValueError, "unsupported Cargo lock package fields"):
            _ = lock_packages(base + 'future_field="unhandled"\n')


if __name__ == "__main__":
    _ = unittest.main()
