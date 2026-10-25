"""Boundary tests for native package corpus transport, without launching browsers."""
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from tools.audit.doc_html.pages_svg import materialize
from tools.serialization.json import JsonValue


def corpus() -> bytes:
    lines: list[str] = []
    for mode in ("Rows", "Columns", "Single"):
        files: dict[str, JsonValue] = {
            "deep/a.html": "body-A", "deep/b.html": "body-B",
            "deep/assets/doc-" + "0" * 64 + ".css": "css-A",
            "deep/assets/doc-" + "1" * 64 + ".css": "css-B",
        }
        records: list[JsonValue] = []
        for path, value in files.items():
            if not isinstance(value, str):
                raise AssertionError("fixture string")
            records.append({"path": path, "bytes": len(value), "sha256": hashlib.sha256(value.encode()).hexdigest()})
        case: dict[str, JsonValue] = {"mode": mode, "files": files, "manifest": json.dumps({"files": records, "format": "nepl3.local-doc-pages.root-svg-package/1", "display": {"mode": mode}})}
        lines.append("NEPL3_PAGES_SVG_CORPUS " + json.dumps(case))
    return ("test fixture::packages ... " + "\n".join(lines) + "\n").encode()


class CorpusTests(unittest.TestCase):
    def test_exact_payload_is_materialized(self) -> None:
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary) / "packages"
            packages = materialize(corpus(), root)
            self.assertEqual([mode for mode, _ in packages], ["Rows", "Columns", "Single"])
            for _, path in packages:
                self.assertEqual((path / "deep/a.html").read_bytes(), b"body-A")
                self.assertTrue((path / "manifest.json").is_file())

    def test_missing_duplicate_unsafe_and_changed_payloads_are_rejected(self) -> None:
        original = corpus()
        for bad in (
            original.splitlines()[0],
            original.replace(b'"Single"', b'"Rows"'),
            original.replace(b"deep/a.html", b"../a.html"),
            original.replace(b"body-A", b"changed"),
            original.replace(b"root-svg-package/1", b"root-svg-package/2"),
            original.replace(b'"mode": "Rows"', b'"mode": "Rows", "mode": "Rows"'),
        ):
            with self.subTest(bad=bad[:90]), tempfile.TemporaryDirectory() as temporary:
                with self.assertRaises(ValueError):
                    materialize(bad, Path(temporary) / "packages")


if __name__ == "__main__":
    unittest.main()
