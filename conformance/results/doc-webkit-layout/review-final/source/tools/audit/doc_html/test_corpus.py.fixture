import unittest
from browser import CASES, extract_cases, valid_measurement


class Corpus(unittest.TestCase):
    def setUp(self):
        self.raw = (
            b'test html::browser_layout_corpus_from_real_doc_source ... DOC_HTML_CASE ruby 41\n'
            b'DOC_HTML_CASE anno 42\nDOC_HTML_CASE anno-ruby 43\n'
            b'DOC_HTML_CASE ruby-anno 44\nDOC_HTML_CASE ruby-ruby 45\n'
            b'DOC_HTML_CASE table-ruby 46\nDOC_HTML_CASE list-ruby 47\nok\n'
        )

        old = {'ruby', 'anno', 'anno-ruby', 'ruby-anno', 'ruby-ruby', 'table-ruby', 'list-ruby'}
        self.extra = {name: name for name in CASES - old}
        self.raw += ''.join(f'DOC_HTML_CASE {name} {name.encode().hex()}\n' for name in sorted(self.extra)).encode()

    def test_raw_cargo_and_plain_logs_agree(self):
        plain = self.raw.split(b' ... ', 1)[1]
        expected = {'ruby': 'A', 'anno': 'B', 'anno-ruby': 'C', 'ruby-anno': 'D',
                    'ruby-ruby': 'E', 'table-ruby': 'F', 'list-ruby': 'G'}
        expected.update(self.extra)
        self.assertEqual(extract_cases(self.raw), expected)
        self.assertEqual(extract_cases(plain), expected)

    def test_missing_duplicate_invalid_or_arbitrary_prefix_fails(self):
        for raw in [self.raw.replace(b'DOC_HTML_CASE ruby 41', b'ignored'),
                    self.raw + b'DOC_HTML_CASE ruby 41\n',
                    self.raw.replace(b'list-ruby 47', b'list-ruby not-hex'),
                    self.raw.replace(b'test html::browser_layout_corpus_from_real_doc_source ... ',
                                     b'untrusted log prefix '),
                    self.raw.replace(b'list-ruby 47', b'list-ruby ff')]:
            with self.subTest(raw=raw), self.assertRaises((ValueError, UnicodeError)):
                extract_cases(raw)


    def test_layout_checks_behavior_instead_of_property_support(self):
        row = {'scripts': 0, 'difference': 0, 'baseline_source_supported': False,
               'annotation_gaps': [0, 1], 'line_gaps': [0, 1]}
        self.assertTrue(valid_measurement(row))
        for change in [{'difference': 10}, {'scripts': 1}, {'annotation_gaps': []},
                       {'annotation_gaps': [-1]}, {'line_gaps': [-1]}]:
            with self.subTest(change=change):
                self.assertFalse(valid_measurement(row | change))


if __name__ == '__main__':
    unittest.main()
