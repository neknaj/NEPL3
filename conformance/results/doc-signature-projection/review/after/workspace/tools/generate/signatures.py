"""Project normative form tables into Doc; do not rewrite authored prose."""
import argparse
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
LANGUAGES = ("Grammar", "Doc", "Math", "Circuit")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"Duplicate form-table JSON key: {key}")
        result[key] = value
    return result


def load_categories(path):
    return json.loads(path.read_text(encoding='utf-8'),
                      object_pairs_hook=unique_object)['categories']


def quoted(text):
    """NEPL Text escapes, independent of JSON's Unicode escape syntax."""
    if any(ord(c) < 32 and c not in "\n\r\t" for c in text):
        raise ValueError("Unsupported control character in form table")
    return '"' + text.replace('\\', '\\\\').replace('"', '\\"').replace(
        '\n', '\\n').replace('\r', '\\r').replace('\t', '\\t') + '"'


def code(text):
    return 'sentence cons code ' + quoted(text) + ' nil'


def read_type(value):
    if isinstance(value, str) and value:
        return value
    if isinstance(value, dict) and set(value) == {"list"}:
        return "List<" + read_type(value["list"]) + ">"
    raise ValueError(f"Invalid reader category: {value!r}")


def generate(categories, language):
    if language not in LANGUAGES:
        raise ValueError("Unknown language")
    out = [
        '# Generated from design/forms.json by tools/generate/signatures.py; do not edit.',
        f'article ja "{language}：[構文/こうぶん]signatureの[全表/ぜんぴょう]"',
        'body',
        '  cons paragraph',
        '    cons sentence cons text "この" cons ruby text "表" text "ひょう" cons text "は" cons code "design/forms.json" cons text "から" cons ruby text "生成" text "せいせい" cons text "した。" nil',
        '    cons sentence cons code "List<T>" cons text "は" cons code "cons T List<T>" cons text " / " cons code "nil" cons text "、" cons code "@" cons text "は" cons ruby text "基礎" text "きそ" cons text "readerの" cons ruby text "識別" text "しきべつ" cons text "である。" nil',
        '    nil',
    ]
    found = False
    for category, definition in categories.items():
        if not category.startswith(language + "/"):
            continue
        found = True
        # Hex of UTF-8 is injective, deterministic and a valid Name.
        anchor = 'category_' + category.encode('utf-8').hex()
        out.extend([
            f'  cons section {anchor} {code(category)}',
            '    body',
            '      cons table cons left cons left cons left cons right nil',
            '        some row cons "[綴/つづ]り" cons "kind" cons "[子/こ]（[順序固定/じゅんじょこてい]）" cons "arity" nil',
        ])
        for spelling, form in definition['forms'].items():
            fields = ', '.join(field['name'] + ': ' + read_type(field['read'])
                               for field in form['fields'])
            cells = [code(spelling), code(language + '.' + form['kind']),
                     code(fields) if fields else '"なし"', quoted(str(len(form['fields'])))]
            out.append('        cons row ' + ' '.join('cons ' + cell for cell in cells) + ' nil')
        out.append('        nil')
        if definition.get('leaf'):
            out.extend([
                '      cons paragraph',
                '        cons sentence cons ruby text "葉" text "は" cons text "の" cons ruby text "認識規則" text "にんしききそく" cons text "：" cons code ' + quoted(definition['leaf']) + ' cons text "。" nil',
                '        nil',
            ])
        out.append('      nil')
    if not found:
        raise ValueError(f"No categories for {language}")
    return '\n'.join(out + ['  nil', ''])


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--write', action='store_true')
    args = parser.parse_args()
    categories = load_categories(ROOT/'design/forms.json')
    for language in LANGUAGES:
        output = ROOT/'doc/migration/generated'/f'{language.lower()}-signatures.nepld'
        data = generate(categories, language).encode('utf-8')
        if args.write:
            output.parent.mkdir(parents=True, exist_ok=True)
            output.write_bytes(data)
        elif not output.exists() or output.read_bytes() != data:
            raise SystemExit(f'{output.relative_to(ROOT)} is stale; run python tools/generate/signatures.py --write')


if __name__ == '__main__':
    main()
