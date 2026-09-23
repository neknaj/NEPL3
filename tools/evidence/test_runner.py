import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from tools.evidence.runner import run,verify,specification
from tools.evidence.records import Exited, Incomplete, Interrupted
from tools.serialization.json import JsonValue, array, decode, object_value


class RunnerTests(unittest.TestCase):
    def fixture(self,parent: Path,code: str='print("evidence")') -> tuple[Path, Path, Path]:
        root=parent/'repo'; root.mkdir()
        def git(*args: str) -> None:
            _ = subprocess.run(['git','-C',str(root),*args],check=True,capture_output=True)
        git('init'); git('config','user.email','test@example.invalid'); git('config','user.name','test')
        spec=root/'spec.json'
        _ = spec.write_text(json.dumps(dict(version=1,scope='test command only',commands=[dict(id='probe',argv=['python','-c',code],cwd='.',timeout_seconds=5)])),encoding='utf-8')
        git('add','spec.json'); git('commit','-m','fixture')
        return root,spec,parent/'evidence'

    def test_records_command_logs_without_source_copy_and_refuses_overwrite(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve())
            self.assertTrue(run(root,spec,out))
            report=verify(out)
            self.assertEqual((out/'probe.stdout').read_bytes().strip(),b'evidence')
            self.assertFalse(report.representation()['acceptance_decision'])
            self.assertEqual(report.commands[0].outcome, Exited(0))
            self.assertEqual({p.name for p in out.iterdir()},{'spec.json','manifest.json','probe.stdout','probe.stderr'})
            with self.assertRaises(FileExistsError): _ = run(root,spec,out)

    def test_failed_command_is_sealed_but_not_successful(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve(),'import sys; print("failed"); sys.exit(3)')
            self.assertFalse(run(root,spec,out))
            self.assertEqual(verify(out).commands[0].outcome, Exited(3))

    def test_log_command_scope_and_file_set_tampering_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve()); _ = run(root,spec,out)
            raw=(out/'manifest.json').read_bytes()
            mutations: tuple[tuple[str, JsonValue], ...] = (('argv',['other']),('exit_code',1))
            for field,value in mutations:
                report=dict(object_value(decode(raw)))
                command=dict(object_value(array(report['commands'])[0]))
                command[field]=value
                report['commands']=[command]
                _ = (out/'manifest.json').write_text(json.dumps(report),encoding='utf-8')
                with self.assertRaises(ValueError): _ = verify(out)
            _ = (out/'manifest.json').write_bytes(raw)
            _ = (out/'probe.stdout').write_bytes(b'changed')
            with self.assertRaises(ValueError): _ = verify(out)

    def test_dirty_source_and_untracked_input_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve())
            _ = (root/'new.py').write_text('pass',encoding='utf-8')
            with self.assertRaises(ValueError): _ = run(root,spec,out)
            self.assertFalse(out.exists())

    def test_historical_working_directory_is_not_executable(self) -> None:
        for cwd in ['conformance/results/old','./conformance/results/old','CONFORMANCE/RESULTS/old']:
            value: JsonValue = dict(version=1,scope='boundary',commands=[dict(id='probe',argv=['python','seal.py.fixture'],cwd=cwd,timeout_seconds=5)])
            with self.subTest(cwd=cwd),self.assertRaises(ValueError): _ = specification(value)

    def test_timeout_is_unknown_and_does_not_run_next_command(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve(),'import time; time.sleep(10)')
            value=dict(object_value(decode(spec.read_bytes())))
            first=dict(object_value(array(value['commands'])[0]))
            first['timeout_seconds']=1
            value['commands']=[first,dict(id='later',argv=['python','-c','print("not reached")'],cwd='.',timeout_seconds=5)]
            _ = spec.write_text(json.dumps(value),encoding='utf-8')
            _ = subprocess.run(['git','-C',str(root),'commit','-am','timeout fixture'],check=True,capture_output=True)
            self.assertFalse(run(root,spec,out))
            report=verify(out)
            self.assertEqual(len(report.commands),1)
            outcome=report.commands[0].outcome
            self.assertIsInstance(outcome, Interrupted)
            if isinstance(outcome, Interrupted):
                self.assertEqual(outcome.state, Incomplete.UNKNOWN)

    def test_tracked_source_mutation_is_not_success(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve(),'from pathlib import Path; Path("spec.json").write_text("changed")')
            self.assertFalse(run(root,spec,out))
            self.assertTrue(verify(out).source_changed)

    def test_start_failure_retains_logs_and_stops_sequence(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve())
            value=dict(object_value(decode(spec.read_bytes())))
            value['commands']=[*array(value['commands']),dict(id='later',argv=['python','-c','print("not reached")'],cwd='.',timeout_seconds=5)]
            _ = spec.write_text(json.dumps(value),encoding='utf-8')
            _ = subprocess.run(['git','-C',str(root),'commit','-am','launch fixture'],check=True,capture_output=True)
            invoke = subprocess.run
            launches: list[tuple[str, ...]] = []

            def execute(argv: list[str], **kwargs: object) -> subprocess.CompletedProcess[bytes]:
                if argv[0] != 'git':
                    launches.append(tuple(argv))
                    raise OSError('test launch failure')
                # Git calls have the same fixed options throughout the runner.
                self.assertEqual(kwargs, {'check': True, 'capture_output': True})
                return invoke(argv, check=True, capture_output=True)

            with patch('tools.evidence.runner.subprocess.run', side_effect=execute):
                self.assertFalse(run(root,spec,out))
            result=verify(out).commands[0].outcome
            self.assertEqual(result, Interrupted(Incomplete.NOT_RUN, 'process could not start'))
            self.assertEqual((out/'probe.stdout').read_bytes(), b'')
            self.assertEqual((out/'probe.stderr').read_bytes(), b'')
            self.assertEqual(len(launches), 1)
            self.assertFalse((out/'later.stdout').exists())
            self.assertEqual(len(verify(out).commands), 1)

    def test_external_record_types_and_contradictory_states_are_rejected(self) -> None:
        with tempfile.TemporaryDirectory() as directory:
            root,spec,out=self.fixture(Path(directory).resolve())
            self.assertTrue(run(root,spec,out))
            raw=(out/'manifest.json').read_bytes()
            mutations: tuple[tuple[str, JsonValue], ...] = (
                ('exit_code', True), ('outcome', 'unknown'), ('outcome', 'unrecognized'),
                ('timeout_seconds', False), ('argv', [7]),
            )
            for field,value in mutations:
                manifest=dict(object_value(decode(raw)))
                command=dict(object_value(array(manifest['commands'])[0]))
                command[field]=value
                if field == 'outcome' and value == 'unrecognized':
                    command['exit_code']=None
                    command['reason']='test unknown state'
                manifest['commands']=[command]
                _ = (out/'manifest.json').write_text(json.dumps(manifest),encoding='utf-8')
                with self.subTest(field=field,value=value),self.assertRaises(ValueError):
                    _ = verify(out)


if __name__=='__main__': _ = unittest.main()
