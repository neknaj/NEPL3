def walk(x):
 if isinstance(x,list):
  for y in x:yield from walk(y)
 elif isinstance(x,dict):
  yield x
  for y in x.get('fields',{}).values():yield from walk(y)
han=re.compile('[\u3400-\u4dbf\u4e00-\u9fff\uf900-\ufaff\u3005]+');ruby=re.compile(r'\[([^\[\]/]+?)/([^\[\]/]+?)\]');readings=[]
def pair(b,r):
 assert han.fullmatch(b) and re.fullmatch('[ぁ-ゖー]+',r),(b,r)
 readings.append([b,r]);return b
def plain(x,protected=False):
 if isinstance(x,list):return ''.join(plain(y,protected) for y in x)
 if not isinstance(x,dict):return ''
 k=x['kind'];f=x.get('fields',{})
 if k=='leaf':
  s=json.loads(x['token']);rest=ruby.sub('',s)
  assert not han.search(rest) and not any(c in rest for c in '[]{}\\')
  return ruby.sub(lambda t:pair(t[1],t[2]),s)
 if k=='Ruby':return pair(plain(f['base'],True),plain(f['reading'],True))
 if k in ['Text','InlineCode']:
  if k=='Text' and not protected:assert not han.search(f['text'])
  return f['text']
 return ''.join(plain(y,protected) for y in f.values())
