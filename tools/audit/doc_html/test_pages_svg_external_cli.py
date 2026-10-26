"""Receipt/file integrity tests; actual markup is checked by the hosted browser."""
import hashlib
from html import escape
import json
from pathlib import Path
import tempfile
import unittest
from tools.serialization.json import JsonValue, array, object_value
from tools.audit.doc_html.pages_svg_external_cli import GOOGLE_FONTS, stylesheet_path, verify_package, verify_stylesheets


def fixture(root: Path, renderer: str) -> dict[str, JsonValue]:
    files = {
        "one/index.html": f'<link rel="stylesheet" href="{escape(GOOGLE_FONTS, quote=True)}"><link rel="stylesheet" href="{stylesheet_path("one")}">one',
        "two/index.html": f'<link rel="stylesheet" href="{escape(GOOGLE_FONTS, quote=True)}"><link rel="stylesheet" href="{stylesheet_path("two")}">two',
        "one/" + stylesheet_path("one"): "css-one",
        "two/" + stylesheet_path("two"): "css-two",
        "one/assets/" + hashlib.sha256(b"svg-one").hexdigest() + ".svg": "svg-one",
        "two/assets/" + hashlib.sha256(b"svg-two").hexdigest() + ".svg": "svg-two",
    }
    records: list[JsonValue] = []
    for name, content in files.items():
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        data = content.encode()
        _ = path.write_bytes(data)
        records.append({"path": name, "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    preferred = renderer == "katex-preferred"
    source: dict[str, JsonValue] = {"math_renderer": renderer if preferred else "mathml-only-explicit"}
    generation: JsonValue = False
    if preferred:
        generation = {"successful_visual_pages": 1, "successful_visuals": 2, "host_invocation_recorded": False}
        source["native_generation"] = {
            "contract": "nepl3.local-doc-pages.svg-katex-context/1", "requested_renderer": renderer,
            "pinned_version": "0.18.7", "host_invocation_recorded": False,
            "host_internal_work": "unobserved", "host_internal_allocation": "unobserved",
            "pages": [
                {"page": "one", "request_context": "1" * 64,
                 "visuals": [{"root": n, "scope": "nepl-math-" + "2" * 64 + f"-k{n}"} for n in range(2)],
                 "formulas": [{"root": n, "owner_path": [{"kind": "doc", "placement": n, "embed": n}], "fallback_detail": None, "unsupported": None} for n in range(2)],
                 "files": [{"path": "katex.min.css", "mime": "text/css", "bytes": 1, "sha256": "3" * 64}]},
                {"page": "two", "request_context": "4" * 64, "visuals": [], "formulas": [], "files": []},
            ],
        }
    receipt: dict[str, JsonValue] = {
        "format": "nepl3.local-doc-pages.root-svg-external-package/" + ("2" if preferred else "1"),
        "svg": "external", "display": {"mode": "Rows"}, "source_input": source,
        "katex_generation": generation, "files": records,
    }
    _ = (root / "manifest.json").write_text(json.dumps(receipt), encoding="utf-8")
    return receipt


class ExternalCliPackageTests(unittest.TestCase):
    def test_both_math_modes(self) -> None:
        for renderer in ("katex-preferred", "mathml-only"):
            with self.subTest(renderer=renderer), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                _ = fixture(root, renderer)
                verify_package(root, renderer, "Rows", 2)

    def test_only_exact_legacy_font_and_recorded_page_css_are_allowed(self) -> None:
        local = stylesheet_path("one")
        paths = {"one/" + local, "two/" + stylesheet_path("two")}
        good = f'<link rel="stylesheet" href="{escape(GOOGLE_FONTS, quote=True)}"><link rel="stylesheet" href="{local}">'
        verify_stylesheets(good, "one", paths)
        for bad in (
            good.replace(escape(GOOGLE_FONTS, quote=True), "https://evil.example/style.css"),
            good.replace(escape(GOOGLE_FONTS, quote=True), escape(GOOGLE_FONTS.replace("Klee+One", "Other"), quote=True)),
            good.replace(local, "../two/" + stylesheet_path("two")),
            good + f'<link rel="stylesheet" href="{local}">',
            good.replace('rel="stylesheet"', 'rel="stylesheet" rel="alternate"', 1),
        ):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                verify_stylesheets(bad, "one", paths)
        with self.assertRaisesRegex(ValueError, "not recorded"):
            verify_stylesheets(good, "one", set())

    def test_wrong_receipt_and_payload_are_rejected(self) -> None:
        for attack in ("mode", "version", "renderer", "visuals", "extra", "changed", "address", "context", "scope", "formula", "native-file", "native-accounting"):
            with self.subTest(attack=attack), tempfile.TemporaryDirectory() as temporary:
                root = Path(temporary)
                receipt = fixture(root, "katex-preferred")
                if attack == "mode":
                    receipt["svg"] = "embedded"
                elif attack == "version":
                    receipt["format"] = "nepl3.local-doc-pages.root-svg-external-package/1"
                elif attack == "renderer":
                    receipt["source_input"] = {"math_renderer": "mathml-only-explicit"}
                elif attack == "visuals":
                    receipt["katex_generation"] = {"successful_visual_pages": 1, "successful_visuals": 0, "host_invocation_recorded": False}
                elif attack == "extra":
                    _ = (root / "unrecorded").write_text("extra", encoding="utf-8")
                elif attack == "changed":
                    _ = (root / "one/index.html").write_text("changed", encoding="utf-8")
                elif attack in ("context", "scope", "formula", "native-file", "native-accounting"):
                    source = dict(object_value(receipt["source_input"]))
                    native = dict(object_value(source["native_generation"]))
                    if attack == "native-accounting":
                        native["host_invocation_recorded"] = True
                    else:
                        pages: list[JsonValue] = [dict(object_value(v)) for v in array(native["pages"])]
                        first = dict(object_value(pages[0]))
                        if attack == "context":
                            first["request_context"] = "bad"
                        elif attack == "scope":
                            first["visuals"] = [{"root": 0, "scope": "bad"}, {"root": 1, "scope": "nepl-math-" + "2" * 64 + "-k1"}]
                        elif attack == "formula":
                            first["formulas"] = []
                        else:
                            first["files"] = [{"path": "katex.min.css", "mime": "text/css", "bytes": 0, "sha256": "3" * 64}]
                        pages[0] = first
                        native["pages"] = pages
                    source["native_generation"] = native
                    receipt["source_input"] = source
                else:
                    digest = hashlib.sha256(b"svg-one").hexdigest()
                    records: list[JsonValue] = []
                    for raw in array(receipt["files"]):
                        record = dict(object_value(raw))
                        if record["path"] == f"one/assets/{digest}.svg":
                            record["path"] = "one/assets/" + "f" * 64 + ".svg"
                        records.append(record)
                    receipt["files"] = records
                    _ = (root / "manifest.json").write_text(json.dumps(receipt), encoding="utf-8")
                    _ = (root / f"one/assets/{digest}.svg").rename(root / ("one/assets/" + "f" * 64 + ".svg"))
                    with self.assertRaisesRegex(ValueError, "SVG content address"):
                        verify_package(root, "katex-preferred", "Rows", 2)
                    continue
                _ = (root / "manifest.json").write_text(json.dumps(receipt), encoding="utf-8")
                with self.assertRaises(ValueError):
                    verify_package(root, "katex-preferred", "Rows", 2)


if __name__ == "__main__":
    _ = unittest.main()
