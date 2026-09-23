"""Decode TOML values and retain syntax owned by the format-preserving editor."""

from datetime import date, datetime, time, timezone
import unittest

from tools.serialization.toml import array, decode, parse, string, table


class TomlTests(unittest.TestCase):
    def test_scalar_array_and_table_values(self) -> None:
        value = decode('''day = 2026-09-23
at = 12:34:56
time = 2026-09-23T12:34:56Z
items = [true, 42, 1.5, "世界𠮷"]
[nested]
path = "core"
''')
        self.assertEqual(value["day"], date(2026, 9, 23))
        self.assertEqual(value["at"], time(12, 34, 56))
        self.assertEqual(value["time"], datetime(2026, 9, 23, 12, 34, 56, tzinfo=timezone.utc))
        self.assertEqual(array(value["items"]), [True, 42, 1.5, "世界𠮷"])
        self.assertEqual(string(table(value["nested"])["path"]), "core")

    def test_wrong_shapes_fail_narrowing(self) -> None:
        with self.assertRaises(ValueError):
            _ = table("table")
        with self.assertRaises(ValueError):
            _ = array(True)
        with self.assertRaises(ValueError):
            _ = string(42)

    def test_edit_preserves_unrelated_comments_and_out_of_order_tables(self) -> None:
        source = '''# keep this comment
[dependencies.a]
path = 'old-a' # retain annotation
[package]
name = "keep"
[dependencies.b]
path = "old-b"
'''
        document = parse(source)
        dependencies = document.child("dependencies")
        self.assertEqual(dependencies.keys(), ("a", "b"))
        dependencies.child("a").set("path", '/𠮷田/"a')
        dependencies.child("b").set("path", "/b")
        output = document.render()
        self.assertIn("# keep this comment", output)
        self.assertIn("# retain annotation", output)
        self.assertIn('[package]\nname = "keep"', output)
        decoded = table(decode(output)["dependencies"])
        self.assertEqual(table(decoded["a"])["path"], '/𠮷田/"a')
        self.assertEqual(table(decoded["b"])["path"], "/b")


if __name__ == "__main__":
    _ = unittest.main()
