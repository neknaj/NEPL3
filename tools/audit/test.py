"""Negative checks for the independent, deliberately restricted design audit."""

import json
import unittest

from structure import AuditError, MAX_DEPTH, Parser, load_forms, signatures, unique_object


class StructureTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.categories = load_forms()

    def test_unclosed_quotes_and_raw_newlines_are_rejected(self):
        for text in ['"unclosed', '"line\nbreak"', '"ok" "unclosed']:
            with self.subTest(text=text), self.assertRaises(AuditError):
                Parser(text, self.categories)

    def test_text_values_follow_nepl3_scalar_escapes(self):
        # Fixed semantic expectations, not a second decoder used as an oracle.
        cases = [
            ('"plain"', 'plain'),
            ('"世界𠮷"', '世界𠮷'),
            (r'"\"\\\n\r\t"', '"\\\n\r\t'),
            (r'"\u{0}\u{41}\u{20bb7}\u{10FFFF}"', '\0A𠮷\U0010ffff'),
            (r'"he\u{6c}lo"', 'hello'),
        ]
        categories = {"Test/Root": {"leaf": None, "forms": {
            "root": {"kind": "Root", "fields": [{"name": "title", "read": "@Text"}]}
        }}}
        for source, expected in cases:
            with self.subTest(source=source):
                self.assertEqual(Parser(source, {}).complete("@Text"), expected)
                self.assertEqual(
                    Parser("root " + source, categories).complete("Test/Root"),
                    {"kind": "Root", "fields": {"title": expected}},
                )

    def test_json_only_and_invalid_scalar_escapes_are_rejected(self):
        for source in [r'"\u0041"', r'"\b"', r'"\f"', r'"\/"', r'"\uD800"',
                       r'"\u{}"', r'"\u{D800}"', r'"\u{DFFF}"',
                       r'"\u{110000}"', r'"\u{0000041}"', r'"\u{G}"', r'"\u{41"']:
            with self.subTest(source=source), self.assertRaises(AuditError):
                Parser(source, {}).complete("@Text")

    def test_arity_trailing_tokens_and_unknown_forms_are_rejected(self):
        for text in ["frac 1", "frac 1 2 3", "unknown 1 2"]:
            with self.subTest(text=text), self.assertRaises(AuditError):
                Parser(text, self.categories).complete("Math/Expr")

    def test_nested_input_is_bounded(self):
        with self.assertRaises(AuditError):
            Parser("cons 1 " * (MAX_DEPTH + 1) + "nil", self.categories).complete({"list": "Math/Expr"})

    def test_duplicate_form_is_rejected(self):
        form = {"kind": "Form", "fields": {"category": "Expr", "spelling": "test", "kind": "Test", "fields": []}}
        with self.assertRaises(AuditError):
            signatures({"fields": {"declarations": [form, form]}}, "Math")

    def test_duplicate_json_key_is_rejected(self):
        with self.assertRaises(AuditError):
            json.loads('{"x":1,"x":2}', object_pairs_hook=unique_object)

    def test_sentence_internals_are_not_evaluated(self):
        # A malformed Ruby inside a closed quote remains one opaque token here.
        # Sentence semantic acceptance belongs to the real Doc reader, not this audit.
        tree = Parser('"[broken/"', self.categories).complete("Doc/Sentence")
        self.assertEqual(tree["kind"], "leaf")


if __name__ == "__main__":
    unittest.main()
