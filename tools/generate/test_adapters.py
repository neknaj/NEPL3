"""Native adapter layout and configuration boundaries."""

import unittest

from tools.generate.adapters import Codec, Options, Record, Variant, render, shapes
from tools.serialization.json import decode


def tuple_case(name: str, _case: str) -> bool:
    return name == "Root"


class AdapterTests(unittest.TestCase):
    def test_projection_preserves_order_and_excludes_owned_manual_types(self) -> None:
        source = decode('''{"types": {
          "Record": {"record": [["second", "Text"], ["first", "U64"]], "constraints": []},
          "Choice": {"variant": {"Empty": [], "Some": [["value", "Text"]]}, "constraints": []},
          "Manual": {}, "View:Hidden": {}
        }}''')
        result = shapes(source, frozenset(("Manual",)))
        self.assertEqual(tuple(result), ("Record", "Choice"))
        self.assertEqual(result["Record"], Record(("second", "first")))
        self.assertEqual(result["Choice"], Variant({"Empty": (), "Some": ("value",)}))
        # The JSON input may be released or modified without changing the projection.
        assert isinstance(source, dict)
        source.clear()
        self.assertEqual(result["Record"], Record(("second", "first")))

    def test_invalid_layout_and_field_pairs_are_rejected(self) -> None:
        for source in (
            '{"types": {"T": {}}}',
            '{"types": {"T": {"record": [], "variant": {}}}}',
            '{"types": {"T": {"record": [["name"]]}}}',
            '{"types": {"T": {"record": [[true, "Text"]]}}}',
            '{"types": {"T": {"variant": {"Case": false}}}}',
        ):
            with self.subTest(source=source), self.assertRaises(ValueError):
                _ = shapes(decode(source), frozenset())

    def test_tuple_cases_require_exactly_one_field(self) -> None:
        options = Options(frozenset(), {}, tuple_case)
        # Positive control reaches generation with the same configured tuple case.
        output = render({"Root": Variant({"Some": ("value",)})}, "// header", options)
        self.assertIn('Self::Some(value) => variant(s,"Root","Some",[value.put(s,c,b)?],b)', output)
        for members in ((), ("first", "second")):
            with self.subTest(members=members), self.assertRaisesRegex(ValueError, "requires one field"):
                _ = render({"Root": Variant({"Some": members})}, "// header", options)

    def test_registry_and_field_rename_are_generation_inputs(self) -> None:
        options = Options(frozenset(), {"sourceMaps": "source_maps"}, tuple_case, Codec.REGISTRY)
        output = render({"Data": Record(("sourceMaps",))}, "// header", options)
        self.assertIn('record(s,"Data",[self.source_maps.put(s,r,c,b)?],b)', output)
        self.assertIn('Ok(Self {source_maps:Value::read(&f[0],s,r,c,b)?})', output)
        self.assertIn('r:&SchemaRegistry', output)


if __name__ == "__main__":
    _ = unittest.main()
