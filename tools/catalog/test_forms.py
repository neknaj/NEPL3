"""Catalog boundary checks with fixed, independently stated expectations."""

from pathlib import Path
import unittest

from tools.catalog.forms import Field, Form, ListRead, categories, load
from tools.serialization.json import decode


class CatalogTests(unittest.TestCase):
    def test_complete_catalog_and_optional_leaf(self) -> None:
        catalog = load(Path(__file__).resolve().parents[2])
        self.assertEqual(catalog["Doc/Row"].leaf, None)
        self.assertEqual(catalog["Doc/Sentence"].leaf, "sentence")
        self.assertEqual(catalog["Grammar/Root"].forms["language"].fields[0], Field("name", "@Name"))

    def test_nested_read_and_field_order(self) -> None:
        value = decode('''{"X/Root": {"forms": {"root": {"kind": "Root", "fields": [
          {"name": "first", "read": {"list": {"list": "@Text"}}},
          {"name": "second", "read": "X/Root"}]}}}}''')
        result = categories(value)
        self.assertEqual(result["X/Root"].forms["root"], Form("Root", (
            Field("first", ListRead(ListRead("@Text"))), Field("second", "X/Root"))))
        # No mutable JSON aliases survive decoding into the domain records.
        self.assertIsInstance(value, dict)
        if isinstance(value, dict):
            value.clear()
        self.assertIn("X/Root", result)

    def test_invalid_shape_is_rejected_before_use(self) -> None:
        invalid = [
            '[]', '{"C": {"forms": [], "leaf": null}}',
            '{"C": {"forms": {}, "leaf": "other"}}',
            '{"C": {"forms": {}, "extra": true}}',
            '{"C": {"forms": {"x": {"kind": false, "fields": []}}}}',
            '{"C": {"forms": {"x": {"kind": "X", "fields": [], "extra": 1}}}}',
            '{"C": {"forms": {"x": {"kind": "X", "fields": [{"name": "x", "read": {"list": "@Text", "extra": 1}}]}}}}',
        ]
        for source in invalid:
            with self.subTest(source=source), self.assertRaises(ValueError):
                _ = categories(decode(source))


if __name__ == "__main__":
    _ = unittest.main()
