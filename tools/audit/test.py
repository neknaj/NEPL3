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
