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

    def test_explicit_tuple_record_preserves_wire_name(self) -> None:
        options = Options(frozenset(), {}, tuple_case, tuple_records=frozenset(("NodeId",)))
        output = render({"NodeId": Record(("index",))}, "// header", options)
        self.assertIn('record(s,"NodeId",[self.0.put(s,c,b)?],b)', output)
        self.assertIn('Ok(Self(Value::read(&f[0],s,c,b)?))', output)
        for members in ((), ("first", "second")):
            with self.subTest(members=members), self.assertRaisesRegex(ValueError, "tuple record requires one field"):
                _ = render({"NodeId": Record(members)}, "// header", options)

    def test_native_case_rename_preserves_wire_tag_and_scope(self) -> None:
        options = Options(frozenset(), {}, tuple_case, case_names={"NodeKind.Self": "SelfSelector"})
        output = render({"NodeKind": Variant({"Self": (), "Field": ("name",)}),
                         "Other": Variant({"Self": ()})}, "// header", options)
        self.assertIn('Self::SelfSelector => variant(s,"NodeKind","Self",[],b)', output)
        self.assertIn('("Self",0)=>Ok(Self::SelfSelector)', output)
        self.assertIn('Self::Self => variant(s,"Other","Self",[],b)', output)
        self.assertIn('Self::Field {name} => variant(s,"NodeKind","Field",[name.put(s,c,b)?],b)', output)

    def test_renamed_payload_cases_and_tuple_records_with_registry(self) -> None:
        for codec in Codec:
            with self.subTest(codec=codec):
                options = Options(frozenset(), {}, tuple_case, codec,
                                  frozenset(("NodeId",)), {"Root.Some": "Payload", "Data.Named": "Entry"})
                output = render({"NodeId": Record(("index",)),
                                 "Root": Variant({"Some": ("value",)}),
                                 "Data": Variant({"Named": ("first", "second")})}, "// header", options)
                args = "s,r,c,b" if codec is Codec.REGISTRY else "s,c,b"
                self.assertIn(f'Self::Payload(value) => variant(s,"Root","Some",[value.put({args})?],b)', output)
                self.assertIn(f'("Some",1)=>Ok(Self::Payload(Value::read(&f[0],{args})?))', output)
                self.assertIn('Self::Entry {first,second} => variant(s,"Data","Named",', output)
                self.assertIn(f'("Named",2)=>Ok(Self::Entry {{first:Value::read(&f[0],{args})?,second:Value::read(&f[1],{args})?}})', output)
                self.assertIn(f'record(s,"NodeId",[self.0.put({args})?],b)', output)

    def test_legacy_tuple_record_rejects_invalid_arity(self) -> None:
        options = Options(frozenset(), {}, tuple_case)
        for members in ((), ("first", "second")):
            with self.subTest(members=members), self.assertRaisesRegex(ValueError, "tuple record requires one field"):
                _ = render({"ExprRef": Record(members)}, "// header", options)
        output = render({"AssetRef": Record(("first", "second"))}, "// header", options)
        self.assertIn('self.first.put(s,c,b)?', output)

    def test_nominal_field_hooks_follow_wire_identity_before_native_rename(self) -> None:
        for codec in Codec:
            with self.subTest(codec=codec):
                options = Options(frozenset(), {"name": "native_name"}, tuple_case, codec,
                                  frozenset(("NodeId",)), {"NodeKind.Self": "SelfSelector"},
                                  {"NodeKind.Self.name": ("put_name", "read_name"),
                                   "Root.Some.value": ("put_text", "read_text"),
                                   "NodeId.index": ("put_index", "read_index"),
                                   "Node.name": ("put_name", "read_name")})
                output = render({"NodeKind": Variant({"Self": ("name",), "Other": ("name",)}),
                                 "Root": Variant({"Some": ("value",)}),
                                 "NodeId": Record(("index",)), "Node": Record(("name",))}, "// header", options)
                args = "s,r,c,b" if codec is Codec.REGISTRY else "s,c,b"
                self.assertIn(f'Self::SelfSelector {{native_name}} => variant(s,"NodeKind","Self",[put_name(native_name,{args})?],b)', output)
                self.assertIn(f'("Self",1)=>Ok(Self::SelfSelector {{native_name:read_name(&f[0],{args})?}})', output)
                self.assertIn(f'Self::Other {{native_name}} => variant(s,"NodeKind","Other",[native_name.put({args})?],b)', output)
                self.assertIn(f'put_text(value,{args})?', output)
                self.assertIn(f'Self::Some(read_text(&f[0],{args})?)', output)
                self.assertIn(f'put_index(&self.0,{args})?', output)
                self.assertIn(f'Ok(Self(read_index(&f[0],{args})?))', output)
                self.assertIn(f'put_name(&self.native_name,{args})?', output)
                self.assertIn(f'Ok(Self {{native_name:read_name(&f[0],{args})?}})', output)

    def test_registry_and_field_rename_are_generation_inputs(self) -> None:
        options = Options(frozenset(), {"sourceMaps": "source_maps"}, tuple_case, Codec.REGISTRY)
        output = render({"Data": Record(("sourceMaps",))}, "// header", options)
        self.assertIn('record(s,"Data",[self.source_maps.put(s,r,c,b)?],b)', output)
        self.assertIn('Ok(Self {source_maps:Value::read(&f[0],s,r,c,b)?})', output)
        self.assertIn('r:&SchemaRegistry', output)


if __name__ == "__main__":
    _ = unittest.main()
