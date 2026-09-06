import hashlib
import unittest
from grammar import ROOT, SeedError, decode_text, source_input


class SeedAdapterTests(unittest.TestCase):
    def test_complete_source_retains_all_metadata_and_original_bytes(self):
        raw = (ROOT / "languages/grammar/syntax.neplg").read_bytes()
        result = source_input(raw, "grammar-seed", "memory:grammar-seed")
        self.assertEqual(result["source"]["text"].encode("utf-8"), raw)
        self.assertEqual(result["source"]["digest"], hashlib.sha256(raw).hexdigest())
        declarations = result["root"]["fields"]["declarations"]["list"]
        self.assertTrue({"Reader", "Mode", "Form", "Extension", "Category"}.issubset({d["kind"] for d in declarations}))
        form = next(d for d in declarations if d["kind"] == "Form")
        self.assertIn("bindings", form["fields"])
        self.assertIn("styles", form["fields"])
        for declaration in declarations:
            start, end = declaration["span"]
            hstart, hend = declaration["head"]
            self.assertEqual(start, hstart)
            self.assertLessEqual(hend, end)
            self.assertTrue(raw[start:end])
            self.assertTrue(raw[hstart:hend].decode("utf-8").isalpha())

    def test_japanese_comment_crlf_and_scalar_escape_positions_are_byte_offsets(self):
        raw = '# 日本語🙂\r\nlanguage G 1 Root cons reader r literal "x\\u{1F642}\\n" nil\r\n'.encode("utf-8")
        result = source_input(raw, "s", "memory:s")
        self.assertEqual(result["root"]["span"][0], raw.index(b"language"))
        literal = result["root"]["fields"]["declarations"]["list"][0]["fields"]["expression"]["fields"]["text"]
        start, end = literal["span"]
        self.assertEqual(raw[start:end].decode("utf-8"), literal["raw"])
        self.assertEqual(literal["value"], "x🙂\n")
        self.assertEqual(result["source"]["text"].encode("utf-8"), raw)

    def test_json_only_escape_and_invalid_scalars_are_rejected(self):
        for value in ['"\\u0041"', '"\\b"', '"\\u{D800}"', '"\\u{110000}"', '"\\u{}"']:
            with self.subTest(value=value), self.assertRaises(SeedError):
                decode_text(value)


if __name__ == "__main__":
    unittest.main()
