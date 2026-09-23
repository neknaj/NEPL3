"""The external decoder preserves values and caller-selected rejection rules."""

import math
import unittest

from tools.serialization.json import array, decode, integer, object_value, string


class JsonBoundaryTests(unittest.TestCase):
    def test_nested_values_and_unicode_are_preserved(self) -> None:
        value = object_value(decode('{"items":["世界🙂",123456789012345678901234567890,null,true]}'))
        items = array(value["items"])
        self.assertEqual(string(items[0]), "世界🙂")
        self.assertEqual(integer(items[1]), 123456789012345678901234567890)
        self.assertIsNone(items[2])
        self.assertIs(items[3], True)

    def test_duplicate_policy_is_explicit(self) -> None:
        self.assertEqual(object_value(decode('{"x":1,"x":2}'))["x"], 2)
        with self.assertRaisesRegex(ValueError, "duplicate JSON key: x"):
            _ = decode('{"x":1,"x":2}', reject_duplicates=True)

    def test_nonfinite_policy_is_explicit(self) -> None:
        value = decode('NaN')
        self.assertIsInstance(value, float)
        if not isinstance(value, float):
            self.fail("NaN must decode as float")
        self.assertTrue(math.isnan(value))
        with self.assertRaisesRegex(ValueError, "nonfinite JSON number"):
            _ = decode('NaN', reject_nonfinite=True)

    def test_float_overflow_is_rejected_at_any_depth(self) -> None:
        for source in ('1e999', '-1e999', '{"items":[1e999]}'):
            with self.subTest(source=source), self.assertRaisesRegex(ValueError, "nonfinite JSON number"):
                _ = decode(source, reject_nonfinite=True)
        self.assertEqual(decode('1e999'), math.inf)
        self.assertEqual(decode('1.25', reject_nonfinite=True), 1.25)

    def test_field_narrowing_rejects_wrong_types(self) -> None:
        with self.assertRaisesRegex(ValueError, "expected JSON integer"):
            _ = integer(True)
        with self.assertRaisesRegex(ValueError, "expected JSON object"):
            _ = object_value([])
        with self.assertRaisesRegex(ValueError, "expected JSON array"):
            _ = array({})
        with self.assertRaisesRegex(ValueError, "expected JSON string"):
            _ = string(1)


if __name__ == "__main__":
    _ = unittest.main()
