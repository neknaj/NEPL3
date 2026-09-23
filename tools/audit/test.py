"""Negative checks for the independent, deliberately restricted design audit."""

from pathlib import Path
import tempfile
import unittest
from typing import ClassVar

from structure import AuditError, Constructor, MAX_DEPTH, OpaqueLeaf, Parser, load_forms, signatures
from tools.catalog.forms import Categories, Category, Field, Form, ListRead


class StructureTests(unittest.TestCase):
    categories: ClassVar[Categories] = load_forms()

    def test_unclosed_quotes_and_raw_newlines_are_rejected(self) -> None:
        for text in ['"unclosed', '"line\nbreak"', '"ok" "unclosed']:
            with self.subTest(text=text), self.assertRaises(AuditError):
                _ = Parser(text, self.categories)

    def test_text_values_follow_nepl3_scalar_escapes(self) -> None:
        # Fixed semantic expectations, not a second decoder used as an oracle.
        cases = [
            ('"plain"', 'plain'),
            ('"世界𠮷"', '世界𠮷'),
            (r'"\"\\\n\r\t"', '"\\\n\r\t'),
            (r'"\u{0}\u{41}\u{20bb7}\u{10FFFF}"', '\0A𠮷\U0010ffff'),
            (r'"he\u{6c}lo"', 'hello'),
        ]
        categories = {"Test/Root": Category({
            "root": Form("Root", (Field("title", "@Text"),))
        }, None)}
        for source, expected in cases:
            with self.subTest(source=source):
                self.assertEqual(Parser(source, {}).complete("@Text"), expected)
                self.assertEqual(
                    Parser("root " + source, categories).complete("Test/Root"),
                    Constructor("Root", {"title": expected}),
                )

    def test_json_only_and_invalid_scalar_escapes_are_rejected(self) -> None:
        for source in [r'"\u0041"', r'"\b"', r'"\f"', r'"\/"', r'"\uD800"',
                       r'"\u{}"', r'"\u{D800}"', r'"\u{DFFF}"',
                       r'"\u{110000}"', r'"\u{0000041}"', r'"\u{G}"', r'"\u{41"']:
            with self.subTest(source=source), self.assertRaises(AuditError):
                _ = Parser(source, {}).complete("@Text")

    def test_arity_trailing_tokens_and_unknown_forms_are_rejected(self) -> None:
        for text in ["frac 1", "frac 1 2 3", "unknown 1 2"]:
            with self.subTest(text=text), self.assertRaises(AuditError):
                _ = Parser(text, self.categories).complete("Math/Expr")

    def test_nested_input_is_bounded(self) -> None:
        with self.assertRaises(AuditError):
            _ = Parser("cons 1 " * (MAX_DEPTH + 1) + "nil", self.categories).complete(ListRead("Math/Expr"))

    def test_duplicate_form_is_rejected(self) -> None:
        form = Constructor("Form", {"category": "Expr", "spelling": "test", "kind": "Test", "fields": ()})
        with self.assertRaises(AuditError):
            _ = signatures(Constructor("Language", {"declarations": (form, form)}), "Math")

    def test_duplicate_json_key_is_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "design").mkdir()
            path = root / "design/forms.json"
            # A valid catalog is the positive control; only the duplicate differs.
            _ = path.write_text('{"categories": {}}', encoding="utf-8")
            self.assertEqual(load_forms(root), {})
            _ = path.write_text('{"categories": {}, "categories": {}}', encoding="utf-8")
            with self.assertRaisesRegex(AuditError, "duplicate JSON key"):
                _ = load_forms(root)

    def test_sentence_internals_are_not_evaluated(self) -> None:
        # A malformed Ruby inside a closed quote remains one opaque token here.
        # Sentence semantic acceptance belongs to the real Doc reader, not this audit.
        tree = Parser('"[broken/"', self.categories).complete("Doc/Sentence")
        self.assertEqual(tree, OpaqueLeaf('"[broken/"'))


if __name__ == "__main__":
    _ = unittest.main()
