"""Cargo artifact messages are narrowed before selecting the measured core."""

from pathlib import Path
import unittest

from tools.audit.allocation.run import Artifact, artifact


class ArtifactTests(unittest.TestCase):
    def test_artifact_fields_are_retained(self) -> None:
        source = '''{"reason":"compiler-artifact","target":{
          "name":"nepl3_core","kind":["lib"],"src_path":"crate/src/lib.rs"},
          "filenames":["target/core.rlib","target/core.rmeta"]}'''
        self.assertEqual(artifact(source), Artifact("nepl3_core", ("lib",), Path("crate/src/lib.rs"),
                                                   ("target/core.rlib", "target/core.rmeta")))

    def test_non_artifact_is_absent_and_malformed_artifact_is_rejected(self) -> None:
        self.assertIsNone(artifact('{"reason":"build-finished","success":true}'))
        for source in ('{"reason":"compiler-artifact","target":false,"filenames":[]}',
                       '{"reason":"compiler-artifact","target":{"name":"x","kind":[1],"src_path":"x"},"filenames":[]}',
                       '{"reason":"compiler-artifact","target":{"name":"x","kind":[],"src_path":"x"},"filenames":[false]}'):
            with self.subTest(source=source), self.assertRaises(ValueError):
                _ = artifact(source)


if __name__ == "__main__":
    _ = unittest.main()
