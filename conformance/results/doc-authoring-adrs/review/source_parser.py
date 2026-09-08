import re,json
string=r'"(?:\\.|[^"\\])*"'
escaped={chr(0xe000+i):c for i,c in enumerate('[]{}/')}
def docdecode(t):
    out=[]; i=0
    while i<len(t):
        c=t[i]; i+=1
        if c=='\\':
            d=t[i]; i+=1
            if d in '[]{}/': out.append(chr(0xe000+'[]{}/'.index(d)))
            else: out.extend([c,d])
        else: out.append(c)
    return json.loads(''.join(out))
def merge(items):
    out=[]
    for item in items:
        if item[0]=='Text' and not item[1]: continue
        if out and item[0]=='Text' and out[-1][0]=='Text': out[-1][1]+=item[1]
        else: out.append(list(item))
    return out
def literal(s,readings):
    i=0
    def seq(ends):
        nonlocal i
        out=[]
        while i<len(s) and s[i] not in ends:
            c=s[i]; i+=1
            if c=='[':
                a=seq('/'); assert i<len(s) and s[i]=='/'; i+=1
                b=seq(']'); assert i<len(s) and s[i]==']'; i+=1
                readings.append([a,b]); out+=a
            elif c=='{':
                a=seq('/'); notes=[]
                while i<len(s) and s[i]=='/':
                    i+=1; notes.append(seq('/}'))
                assert notes and i<len(s) and s[i]=='}'; i+=1
                out.append(['Anno',a,notes])
            else:
                assert c not in ']}', c
                out.append(['Text',escaped.get(c,c)])
        return merge(out)
    result=seq(''); assert i==len(s)
    return result
class Parser:
    def __init__(self,s):
        self.tokens=re.findall(string+r'|[^\s"]+',s)
        self.i=0; self.readings=[]
    def take(self):
        v=self.tokens[self.i]; self.i+=1; return v
    def read(self,cat):
        if isinstance(cat,dict):
            assert list(cat)==['list'],cat
            out=[]
            while True:
                t=self.take()
                if t=='nil':return out
                assert t=='cons',t
                out.append(self.read(cat['list']))
        t=self.take()
        if cat.startswith('@'): return json.loads(t) if t.startswith('"') else t
        if t.startswith('"'):
            assert cat in ['Doc/Flow','Doc/Sentence']
            return ['Sentence',literal(docdecode(t),self.readings)]
        form=forms[cat]['forms'][t]
        fields=[self.read(f['read']) for f in form['fields']]
        kind=form['kind']
        if cat=='Doc/Inline':
            if kind=='Text':return [['Text',fields[0]]]
            if kind=='Ruby':self.readings.append(fields);return fields[0]
            if kind=='Concat':return merge([v for child in fields[0] for v in child])
            return [[kind]+fields]
        if kind=='Sentence':return ['Sentence',merge([v for child in fields[0] for v in child])]
        return [kind]+fields
    def run(self):
        result=self.read('Doc/Article');assert self.i==len(self.tokens)
        return result
