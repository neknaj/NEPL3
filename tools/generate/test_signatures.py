"""Check Doc structure and independently specified table cells."""
import json
from pathlib import Path
import sys
import tempfile
import unittest

from tools.generate import signatures

sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'audit'))
from structure import Parser, load_forms


def nodes(value, kind):
    if isinstance(value, dict):
        if value.get('kind') == kind:
            yield value
        for child in value.values():
            yield from nodes(child, kind)
    elif isinstance(value, list):
        for child in value:
            yield from nodes(child, kind)


def cell_text(cell):
    if cell['kind'] == 'leaf':
        return json.loads(cell['token'])
    inline, = cell['fields']['inlines']
    return inline['fields']['text']


class SignatureTests(unittest.TestCase):
    def test_duplicate_keys_are_rejected_at_every_object_depth(self):
        samples = [
            '{"categories":{},"categories":{}}',
            '{"categories":{"Math/Expr":{},"Math/Expr":{}}}',
            '{"categories":{"Math/Expr":{"forms":{"add":{},"add":{}}}}}',
            '{"categories":{"Math/Expr":{"forms":{"add":{"fields":[{"name":"x","name":"y"}]}}}}}',
        ]
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)/'forms.json'
            for text in samples:
                with self.subTest(text=text):
                    path.write_text(text, encoding='utf-8')
                    with self.assertRaisesRegex(ValueError, 'Duplicate'):
                        signatures.load_categories(path)

    def test_known_fields_order_nested_list_and_escapes(self):
        categories = {'Doc/Example': {'leaf': 'sentence', 'forms': {
            'q"\\\n[]': {'kind': 'Quoted', 'fields': [
                {'name': 'z', 'read': {'list': {'list': '@Text'}}},
                {'name': 'a', 'read': 'Doc/Inline'}]},
            'empty': {'kind': 'Empty', 'fields': []}}}}
        tree = Parser(signatures.generate(categories, 'Doc'), load_forms()).complete('Doc/Article')
        table, = nodes(tree, 'Table')
        rows = table['fields']['rows']
        self.assertEqual([[cell_text(c) for c in row['fields']['cells']] for row in rows], [
            ['q"\\\n[]', 'Doc.Quoted', 'z: List<List<@Text>>, a: Doc/Inline', '2'],
            ['empty', 'Doc.Empty', 'なし', '0']])
        self.assertEqual([c['kind'] for c in table['fields']['columns']],
                         ['AlignmentLeft', 'AlignmentLeft', 'AlignmentLeft', 'AlignmentRight'])
        self.assertIn('sentence', [n['fields']['text'] for n in nodes(tree, 'InlineCode')])

    def test_all_four_generated_articles_cover_categories_and_forms(self):
        categories = load_forms()
        for language in signatures.LANGUAGES:
            with self.subTest(language=language):
                selected = {k:v for k,v in categories.items() if k.startswith(language+'/')}
                tree = Parser(signatures.generate(categories, language), categories).complete('Doc/Article')
                sections = list(nodes(tree, 'Section'))
                self.assertEqual([cell_text(s['fields']['title']) for s in sections], list(selected))
                self.assertEqual(len({s['fields']['id'] for s in sections}), len(selected))
                tables = list(nodes(tree, 'Table'))
                for table, definition in zip(tables, selected.values(), strict=True):
                    self.assertEqual([cell_text(row['fields']['cells'][0]) for row in table['fields']['rows']], list(definition['forms']))

    def test_rejects_unknown_or_missing_language_and_invalid_reader(self):
        for language in ['Unknown', 'Doc']:
            with self.assertRaises(ValueError):
                signatures.generate({}, language)
        for value in ['', {}, {'list':'@Text','extra':True}, 1]:
            with self.assertRaises(ValueError):
                signatures.read_type(value)
        with self.assertRaises(ValueError):
            signatures.quoted('bad\0')


if __name__ == '__main__':
    unittest.main()
