import json
from pathlib import Path
import subprocess
import tempfile
import unittest

from tools.evidence.runner import run,verify,specification


class RunnerTests(unittest.TestCase):
    def fixture(self,parent,code='print("evidence")'):
        root=parent/'repo'; root.mkdir()
        def git(*args): subprocess.run(['git','-C',str(root),*args],check=True,capture_output=True)
        git('init'); git('config','user.email','test@example.invalid'); git('config','user.name','test')
        spec=root/'spec.json'
        spec.write_text(json.dumps(dict(version=1,scope='test command only',commands=[dict(id='probe',argv=['python','-c',code],cwd='.',timeout_seconds=5)])),encoding='utf-8')
        git('add','spec.json'); git('commit','-m','fixture')
        return root,spec,parent/'evidence'

    def test_records_command_logs_without_source_copy_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve())
            self.assertTrue(run(root,spec,out))
            report=verify(out)
            self.assertEqual((out/'probe.stdout').read_bytes().strip(),b'evidence')
            self.assertFalse(report['acceptance_decision'])
            self.assertEqual(report['commands'][0]['outcome'],'passed')
            self.assertEqual({p.name for p in out.iterdir()},{'spec.json','manifest.json','probe.stdout','probe.stderr'})
            with self.assertRaises(FileExistsError): run(root,spec,out)

    def test_failed_command_is_sealed_but_not_successful(self):
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve(),'import sys; print("failed"); sys.exit(3)')
            self.assertFalse(run(root,spec,out))
            self.assertEqual(verify(out)['commands'][0]['exit_code'],3)

    def test_log_command_scope_and_file_set_tampering_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve()); run(root,spec,out)
            raw=(out/'manifest.json').read_bytes()
            for field,value in [('argv',['other']),('exit_code',1)]:
                report=json.loads(raw); report['commands'][0][field]=value
                (out/'manifest.json').write_text(json.dumps(report),encoding='utf-8')
                with self.assertRaises(ValueError): verify(out)
            (out/'manifest.json').write_bytes(raw)
            (out/'probe.stdout').write_bytes(b'changed')
            with self.assertRaises(ValueError): verify(out)

    def test_dirty_source_and_untracked_input_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve())
            (root/'new.py').write_text('pass',encoding='utf-8')
            with self.assertRaises(ValueError): run(root,spec,out)
            self.assertFalse(out.exists())

    def test_historical_working_directory_is_not_executable(self):
        value=dict(version=1,scope='boundary',commands=[dict(id='probe',argv=['python','seal.py.fixture'],cwd='conformance/results/old',timeout_seconds=5)])
        for cwd in ['conformance/results/old','./conformance/results/old','CONFORMANCE/RESULTS/old']:
            value['commands'][0]['cwd']=cwd
            with self.subTest(cwd=cwd),self.assertRaises(ValueError): specification(value)

    def test_timeout_is_unknown_and_does_not_run_next_command(self):
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve(),'import time; time.sleep(10)')
            value=json.loads(spec.read_text(encoding='utf-8'))
            value['commands'][0]['timeout_seconds']=1
            value['commands'].append(dict(id='later',argv=['python','-c','print("not reached")'],cwd='.',timeout_seconds=5))
            spec.write_text(json.dumps(value),encoding='utf-8')
            subprocess.run(['git','-C',str(root),'commit','-am','timeout fixture'],check=True,capture_output=True)
            self.assertFalse(run(root,spec,out))
            report=verify(out)
            self.assertEqual(len(report['commands']),1)
            self.assertEqual(report['commands'][0]['outcome'],'unknown')

    def test_tracked_source_mutation_is_not_success(self):
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve(),'from pathlib import Path; Path("spec.json").write_text("changed")')
            self.assertFalse(run(root,spec,out))
            self.assertTrue(verify(out)['source_changed'])


if __name__=='__main__': unittest.main()
