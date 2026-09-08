import importlib.util, pathlib, sys
sys.stdout.reconfigure(encoding='utf-8')
p=pathlib.Path(__file__).resolve().parent
spec=importlib.util.spec_from_file_location('structure',p/'snapshot/tools/audit/structure.py')
audit=importlib.util.module_from_spec(spec);spec.loader.exec_module(audit)
categories=audit.load_forms(p/'snapshot')
raw=(p/'snapshot/doc/migration/authored/guide/review.nepld').read_text(encoding='utf-8')
audit.Parser(raw,categories).complete('Doc/Article')
print('PASS outer design-source parser; no production parsing/lowering/rendering claim')
