import collections,hashlib,json,pathlib,re,source_parser
p=pathlib.Path(__file__).resolve().parent
md=(p/'snapshot/doc/spec/18-html-delivery.md').read_text(encoding='utf-8')
doc=(p/'snapshot/doc/migration/authored/18-html-delivery.nepld').read_text(encoding='utf-8')
diff=''
added='\n'.join(x[1:] for x in diff.splitlines() if x.startswith('+') and not x.startswith('+++'))
source_parser.forms=json.loads((p/'snapshot/design/forms.json').read_text(encoding='utf-8'))['categories']
parser=source_parser.Parser(doc)
article=parser.run()
prose=re.sub(r'^```[^\n]*\n.*?^```','',md,flags=re.M|re.S)+'\n'+added
codes=re.findall(r'(?<!`)`([^`\n]+)`(?!`)',prose)
new=[json.loads(m.group(1)) for m in re.finditer(r'\bcode\s+('+source_parser.string+r')',doc)]
missing=list((collections.Counter(codes)-collections.Counter(new)).elements())
assert not missing,missing
fences=re.findall(r'^```([^\n]*)\n(.*?)^```',md,flags=re.S|re.M)
raw=[(json.loads(m.group(1)),json.loads(m.group(2))) for m in re.finditer(r'\brawcode\s+some\s+('+source_parser.string+r')\s+('+source_parser.string+r')',doc)]
assert fences==raw
prefix=[];literals=[]
for n,line in enumerate(doc.splitlines(),1):
    if 'rawcode ' in line:continue
    for m in re.finditer(r'\btext\s+('+source_parser.string+r')',line):
        text=json.loads(m.group(1))
        if re.search('[\u3400-\u9fff]',text) and not re.search(r'\bruby\s*$',line[:m.start()]):prefix.append({'line':n,'text':text})
    for m in re.finditer(r'cons\s+('+source_parser.string+r')',line):
        text=source_parser.docdecode(m.group(1));text=re.sub(r'\[[^\[\]]+\]','',text)
        if re.search('[\u3400-\u9fff]',text):literals.append({'line':n,'text':text})
readings=[]
for a,b in parser.readings:
    assert all(x[0]=='Text' for x in a+b)
    base=''.join(x[1] for x in a);reading=''.join(x[1] for x in b)
    assert re.fullmatch('[\u3400-\u9fff]+',base),base
    assert re.fullmatch('[\u3040-\u309f\u30fc]+',reading),reading
    readings.append([base,reading])
out={'original_inline_code_count':len(codes),'authored_inline_codes':len(new),'missing_inline_codes':missing,'raw_blocks':[{'hint':a,'bytes':len(b.encode()),'sha256':hashlib.sha256(b.encode()).hexdigest()} for a,b in raw],'raw_contents_and_hints_equal':True,'original_markdown_links':re.findall(r'(?<!!)\[([^\]]+)\]\(([^)]+)\)',md),'unannotated_prefix_candidates':prefix,'unannotated_literal_candidates':literals,'ruby_count':len(readings),'ruby_bases_kanji_only':True,'readings':readings,'full_source_subset_parsed':True,'production_runtime_executed':False}
(p/'checks.json').write_text(json.dumps(out,ensure_ascii=False,indent=2)+'\n',encoding='utf-8',newline='\n')
print(json.dumps({k:v for k,v in out.items() if k!='readings'},ensure_ascii=True,indent=2))
