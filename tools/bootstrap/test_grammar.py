import hashlib
import unittest
from grammar import MAX_BYTES, MAX_DEPTH, ROOT, SeedError, SeedParser, decode_text, source_input
from tools.bootstrap.model import Constructor, LiteralNode, NodeList
from tools.catalog.forms import ListRead


class SeedAdapterTests(unittest.TestCase):
    def test_seed_wire_representation_has_exact_literal_and_list_spans(self) -> None:
        raw = b"language G 1 Root nil"
        result = source_input(raw, "s", "memory:s").representation()
        # Fixed byte ranges follow the source above, independently of the parser.
        self.assertEqual(result, {
            "schema": "nepl3.grammar-seed-input/1",
            "source": {"sourceId": "s", "revision": 0, "uri": "memory:s",
                       "digest": hashlib.sha256(raw).hexdigest(), "text": raw.decode("utf-8")},
            "root": {"kind": "Language", "category": "Grammar/Root", "head": [0, 8], "span": [0, 21],
                     "fields": {
                         "name": {"literal": "Name", "value": "G", "raw": "G", "span": [9, 10]},
                         "revision": {"literal": "Nat", "value": "1", "raw": "1", "span": [11, 12]},
                         "root": {"literal": "Name", "value": "Root", "raw": "Root", "span": [13, 17]},
                         "declarations": {"list": [], "span": [18, 21], "heads": [[18, 21]]},
                     }},
        })

    def test_source_and_constructor_limits_remain_enforced(self) -> None:
        for raw in (b'"unclosed', b'"line\nbreak"', b" " * (MAX_BYTES + 1)):
            with self.subTest(raw=raw[:30]), self.assertRaises(SeedError):
                _ = SeedParser(raw, {})
        with self.assertRaises(SeedError):
            _ = SeedParser(b"cons 1 " * (MAX_DEPTH + 1) + b"nil", {}).parse(ListRead("@Nat"))
        for raw in (b"language G 1 Root", b"language G 1 Root nil extra", b"unknown"):
            with self.subTest(raw=raw), self.assertRaises(SeedError):
                _ = source_input(raw, "s", "memory:s")

    def test_complete_source_retains_all_metadata_and_original_bytes(self) -> None:
        raw = (ROOT / "languages/grammar/syntax.neplg").read_bytes()
        result = source_input(raw, "grammar-seed", "memory:grammar-seed")
        self.assertEqual(result.source.text.encode("utf-8"), raw)
        self.assertEqual(result.source.digest, hashlib.sha256(raw).hexdigest())
        declaration_list = result.root.fields["declarations"]
        assert isinstance(declaration_list, NodeList)
        declarations: list[Constructor] = []
        for declaration in declaration_list.items:
            assert isinstance(declaration, Constructor)
            declarations.append(declaration)
        self.assertTrue({"Reader", "Mode", "Form", "Extension", "Category"}.issubset({d.kind for d in declarations}))
        form = next(d for d in declarations if d.kind == "Form")
        self.assertIn("bindings", form.fields)
        self.assertIn("styles", form.fields)
        for declaration in declarations:
            start, end = declaration.span.start, declaration.span.end
            hstart, hend = declaration.head.start, declaration.head.end
            self.assertEqual(start, hstart)
            self.assertLessEqual(hend, end)
            self.assertTrue(raw[start:end])
            self.assertTrue(raw[hstart:hend].decode("utf-8").isalpha())

    def test_japanese_comment_crlf_and_scalar_escape_positions_are_byte_offsets(self) -> None:
        raw = '# 日本語🙂\r\nlanguage G 1 Root cons reader r literal "x\\u{1F642}\\n" nil\r\n'.encode("utf-8")
        result = source_input(raw, "s", "memory:s")
        self.assertEqual(result.root.span.start, raw.index(b"language"))
        declarations = result.root.fields["declarations"]
        assert isinstance(declarations, NodeList)
        reader = declarations.items[0]
        assert isinstance(reader, Constructor)
        expression = reader.fields["expression"]
        assert isinstance(expression, Constructor)
        literal = expression.fields["text"]
        assert isinstance(literal, LiteralNode)
        start, end = literal.span.start, literal.span.end
        self.assertEqual(raw[start:end].decode("utf-8"), literal.raw)
        self.assertEqual(literal.value, "x🙂\n")
        self.assertEqual(result.source.text.encode("utf-8"), raw)

    def test_json_only_escape_and_invalid_scalars_are_rejected(self) -> None:
        for value in ['"\\u0041"', '"\\b"', '"\\u{D800}"', '"\\u{110000}"', '"\\u{}"']:
            with self.subTest(value=value), self.assertRaises(SeedError):
                _ = decode_text(value)


if __name__ == "__main__":
    _ = unittest.main()
