"""Collect command evidence; never decide acceptance or write a review."""
import argparse
import hashlib
import json
from pathlib import Path
import platform
import re
import subprocess
import sys


def digest(data): return hashlib.sha256(data).hexdigest()


def unique(pairs):
    result={}
    for key,value in pairs:
        if key in result: raise ValueError('duplicate JSON key')
        result[key]=value
    return result


def read_json(path):
    return json.loads(path.read_bytes().decode('utf-8'),object_pairs_hook=unique,
                      parse_constant=lambda value: (_ for _ in ()).throw(ValueError('nonfinite JSON')))


def specification(value):
    if not isinstance(value,dict) or set(value)!={'version','scope','commands'} or type(value['version']) is not int or value['version']!=1:
        raise ValueError('invalid specification')
    if not isinstance(value['scope'],str) or not value['scope'].strip(): raise ValueError('scope required')
    if not isinstance(value['commands'],list) or not value['commands']: raise ValueError('commands required')
    names=set()
    for command in value['commands']:
        if not isinstance(command,dict) or set(command)!={'id','argv','cwd','timeout_seconds'}: raise ValueError('invalid command')
        name=command['id']
        if not isinstance(name,str) or not re.fullmatch('[a-z0-9-]{1,80}',name) or name in names: raise ValueError('invalid command ID')
        names.add(name)
        argv=command['argv']
        if not isinstance(argv,list) or not argv or any(not isinstance(arg,str) or not arg or '\0' in arg for arg in argv): raise ValueError('invalid argv')
        if any('conformance/results/' in arg.replace('\\','/') for arg in argv): raise ValueError('do not execute historical evidence')
        cwd=command['cwd']
        if not isinstance(cwd,str) or Path(cwd).is_absolute() or '..' in Path(cwd).parts: raise ValueError('invalid cwd')
        normalized='/'.join(Path(cwd).parts).replace('\\','/').lower()
        if normalized=='conformance/results' or normalized.startswith('conformance/results/'): raise ValueError('historical evidence is not an execution directory')
        timeout=command['timeout_seconds']
        if type(timeout) is not int or not 0 < timeout <= 3600: raise ValueError('invalid timeout')
    return value


def git(root,*args):
    return subprocess.run(['git','-C',str(root),*args],check=True,capture_output=True).stdout


def run(root,spec_path,output):
    root=root.resolve(strict=True)
    spec_raw=spec_path.read_bytes(); spec=specification(read_json(spec_path))
    revision=git(root,'rev-parse','HEAD').decode().strip()
    # A reviewed source revision must exist. No full source snapshot is copied.
    if git(root,'diff','HEAD','--binary'): raise ValueError('commit tracked changes before recording evidence')
    if git(root,'ls-files','--others','--exclude-standard','-z'): raise ValueError('untracked files require review before recording evidence')
    for command in spec['commands']:
        cwd=(root/command['cwd']).resolve(strict=True)
        if not cwd.is_relative_to(root) or not cwd.is_dir(): raise ValueError('cwd escapes repository')
        if cwd.is_relative_to((root/'conformance/results').resolve()): raise ValueError('historical evidence is not an execution directory')
    output.mkdir(parents=True,exist_ok=False)
    (output/'spec.json').write_bytes(spec_raw)
    rows=[]
    for command in spec['commands']:
        argv=list(command['argv'])
        if argv[0]=='python': argv[0]=sys.executable
        record=dict(id=command['id'],argv=argv,cwd=command['cwd'],timeout_seconds=command['timeout_seconds'])
        with (output/(command['id']+'.stdout')).open('xb') as stdout, (output/(command['id']+'.stderr')).open('xb') as stderr:
            try:
                completed=subprocess.run(argv,cwd=root/command['cwd'],stdout=stdout,stderr=stderr,timeout=command['timeout_seconds'])
                record.update(outcome='passed' if completed.returncode==0 else 'failed',exit_code=completed.returncode)
            except subprocess.TimeoutExpired:
                record.update(outcome='unknown',exit_code=None,reason='timeout; descendant completion and final log bytes not established')
            except OSError:
                record.update(outcome='not-run',exit_code=None,reason='process could not start')
        rows.append(record)
        if record['outcome'] in ('unknown','not-run'): break
    files=[]
    for path in sorted(output.iterdir()):
        data=path.read_bytes(); files.append(dict(path=path.name,bytes=len(data),sha256=digest(data)))
    changed=bool(git(root,'diff','HEAD','--binary')) or git(root,'rev-parse','HEAD').decode().strip()!=revision
    report=dict(version=1,kind='command-evidence',scope=spec['scope'],source_revision=revision,
                runner_sha256=digest(Path(__file__).read_bytes()),source_changed=changed,
                source_check='tracked HEAD and diff before/after; ignored inputs and transient changes are not covered',
                environment=dict(python=sys.version,python_executable=sys.executable,platform=platform.platform(),machine=platform.machine()),
                commands=rows,files=files,acceptance_decision=False)
    (output/'manifest.json').write_text(json.dumps(report,indent=2,ensure_ascii=False)+'\n',encoding='utf-8',newline='\n')
    verify(output)
    return not changed and len(rows)==len(spec['commands']) and all(r['outcome']=='passed' for r in rows)


def verify(output):
    value=read_json(output/'manifest.json')
    if type(value.get('version')) is not int or value['version']!=1 or value.get('kind')!='command-evidence' or value.get('acceptance_decision') is not False: raise ValueError('wrong evidence kind')
    if type(value.get('source_changed')) is not bool: raise ValueError('source change state missing')
    if not re.fullmatch('[0-9a-f]{40}',value.get('source_revision','')): raise ValueError('source revision missing')
    names=set()
    for entry in value['files']:
        name=entry['path']
        if not isinstance(name,str) or not re.fullmatch('[a-z0-9.-]+',name) or name in ('.','..','manifest.json') or name in names: raise ValueError('invalid evidence path')
        names.add(name); path=output/name
        if path.is_symlink() or not path.is_file(): raise ValueError('evidence must be regular data')
        data=path.read_bytes()
        if type(entry['bytes']) is not int or len(data)!=entry['bytes'] or digest(data)!=entry['sha256']: raise ValueError('evidence hash mismatch')
    if {p.name for p in output.iterdir()}!=names|{'manifest.json'}: raise ValueError('evidence file set changed')
    spec=specification(read_json(output/'spec.json'))
    if value.get('scope')!=spec['scope']: raise ValueError('scope mismatch')
    if not 0<len(value['commands'])<=len(spec['commands']): raise ValueError('missing or extra command results')
    if len(value['commands'])<len(spec['commands']) and value['commands'][-1]['outcome'] not in ('unknown','not-run'): raise ValueError('unexplained missing results')
    for row,command in zip(value['commands'],spec['commands']):
        if row['id']!=command['id'] or row['cwd']!=command['cwd'] or row['timeout_seconds']!=command['timeout_seconds']: raise ValueError('command identity mismatch')
        expected=list(command['argv'])
        if expected[0]=='python': expected[0]=value['environment']['python_executable']
        if row['argv']!=expected: raise ValueError('command argv mismatch')
        if row['outcome'] not in ('passed','failed','unknown','not-run'): raise ValueError('invalid outcome')
        if row['outcome'] in ('passed','failed'):
            if type(row['exit_code']) is not int or (row['exit_code']==0)!=(row['outcome']=='passed'): raise ValueError('contradictory outcome')
        elif row['exit_code'] is not None: raise ValueError('unexecuted exit code')
        if not {row['id']+'.stdout',row['id']+'.stderr'}<=names: raise ValueError('missing raw logs')
    return value


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    sub=parser.add_subparsers(dest='operation',required=True)
    collect=sub.add_parser('run'); collect.add_argument('spec',type=Path); collect.add_argument('output',type=Path)
    check=sub.add_parser('verify'); check.add_argument('output',type=Path)
    args=parser.parse_args()
    try:
        if args.operation=='verify': verify(args.output); return 0
        return 0 if run(Path.cwd(),args.spec,args.output) else 1
    except (ValueError,OSError,KeyError,TypeError,subprocess.SubprocessError) as error:
        print(type(error).__name__+': evidence operation failed',file=sys.stderr); return 1


if __name__=='__main__': sys.exit(main())
