"""Inventory historical Python evidence without executing it.

Classification is triage based on content and path, not a claim that every
historical probe was independently revalidated. Uncertain entries stay visible.
"""
import argparse
import ast
from collections import Counter
import hashlib
import json
import re
import subprocess
from pathlib import Path


def git(*args,input=None):
    return subprocess.run(['git',*args],input=input,check=True,capture_output=True).stdout


def inventory(revision):
    if not re.fullmatch('[0-9a-f]{40}',revision): raise ValueError('full commit required')
    paths=git('ls-tree','-r','--name-only','-z',revision,'conformance/results').decode('utf-8').split('\0')[:-1]
    paths=[p for p in paths if p.endswith(('.py','.py.fixture'))]
    if any('\n' in p or '\r' in p for p in paths): raise ValueError('unsupported batch path')
    raw=git('cat-file','--batch',input=''.join(revision+':'+p+'\n' for p in paths).encode('utf-8'))
    offset=0; rows=[]; seen={}
    for path in paths:
        end=raw.index(b'\n',offset); header=raw[offset:end].split()
        if len(header)!=3 or header[1]!=b'blob': raise ValueError('expected Git blob')
        size=int(header[2]); data=raw[end+1:end+1+size]; offset=end+2+size
        sha=hashlib.sha256(data).hexdigest()
        text=data.decode('utf-8-sig'); syntax=None
        try: syntax=ast.parse(text)
        except SyntaxError: pass
        functions=sorted({n.name for n in ast.walk(syntax) if isinstance(n,(ast.FunctionDef,ast.AsyncFunctionDef))}) if syntax else []
        parts=Path(path).parts
        name=Path(path).name.removesuffix('.fixture')
        source_copy=any(p in parts for p in ('source','sources','snapshot','context'))
        sealing=any(s in text for s in ('sha256(',"hashlib.sha256",'SHA256')) and any(s in text for s in ('manifest','files.json','sealed'))
        executes=any(s in text for s in ('subprocess.run','subprocess.Popen','check_call','check_output'))
        probes=any(s in text for s in ('unittest','assert ','assertEqual','mock.patch','with patch('))
        roles=[]
        if source_copy: roles.append('source-snapshot')
        if sealing: roles.append('evidence-sealing')
        if executes: roles.append('command-runner')
        if probes: roles.append('test-or-reproducer')
        if not roles: roles.append('needs-content-review')
        purpose=ast.get_docstring(syntax) if syntax else None
        rows.append(dict(path=path,git_blob=header[0].decode(),bytes=size,sha256=sha,
                         roles=roles,definitions=functions,purpose=purpose,
                         production=False,reuse_candidates=[r for r in roles if r in ('evidence-sealing','command-runner')],
                         one_off_harness='not established' if not source_copy else 'source copy',duplicate_of=seen.get(sha),
                         future_policy='reference Git source' if source_copy else
                         'shared runner/seal; move substantive probes to managed tests; justify exceptional reproducer',
                         ))
        seen.setdefault(sha,path)
    return dict(version=1,source_revision=revision,scope='tracked historical Python and Python fixtures',
                preservation='retain historical bytes; do not execute from results',
                assessment='content/path triage; mixed roles and reuse candidates are not proof of one-off use or correctness',
                counts=dict(files=len(rows),bytes=sum(r['bytes'] for r in rows),
                            duplicate_copies=sum(r['duplicate_of'] is not None for r in rows),
                            roles=dict(Counter(role for row in rows for role in row['roles']))),files=rows)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('revision'); parser.add_argument('output',type=Path)
    args=parser.parse_args()
    result=inventory(args.revision)
    with args.output.open('x',encoding='utf-8',newline='\n') as output:
        json.dump(result,output,ensure_ascii=False,indent=2); output.write('\n')
    print(json.dumps(result['counts']))


if __name__=='__main__': main()
