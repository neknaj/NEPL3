import json
import unittest
from unittest.mock import patch

from deployment.acquire import acquire
from deployment.observations import Kind
import test_prepared_upload


class AcquireTests(unittest.TestCase):
    def fixture(self):
        args,pins,tar = test_prepared_upload.PreparedUploadTests().fixture()
        main=json.dumps(dict(ref='refs/heads/main',url='https://api.github.com/repos/neknaj/NEPL3/git/refs/heads/main',
                             object=dict(type='commit',sha=pins['current_main']))).encode()
        responses={Kind.MAIN:main,Kind.RUN:args[0],Kind.JOBS:args[1]}
        calls=[]
        def read(query,token,*,timeout):
            calls.append(query.kind)
            self.assertGreater(timeout,0); self.assertLessEqual(timeout,10)
            if query.kind==Kind.ARTIFACT:
                return args[2] if query.identity==pins['artifact_id'] else args[4]
            return responses[query.kind]
        def download(query,token,*,size,sha256,timeout):
            self.assertGreater(timeout,0); self.assertLessEqual(timeout,60)
            return args[3] if query.identity==pins['artifact_id'] else args[5]
        return args,pins,tar,responses,calls,read,download

    def test_actual_tar_checks_and_evidence_are_connected(self):
        args,pins,tar,responses,calls,read,download=self.fixture()
        with patch('deployment.acquire.read',read),patch('deployment.acquire.download',download):
            result=acquire('token',**pins)
        self.assertEqual(result.payload.data,tar)
        self.assertEqual(result.evidence.diagnostic_archive,args[3])
        self.assertEqual(result.evidence.upload_archive,args[5])
        self.assertEqual(result.candidate.source_commit,pins['current_main'])
        self.assertEqual(calls,[Kind.RUN,Kind.JOBS,Kind.MAIN,Kind.ARTIFACT,Kind.ARTIFACT,Kind.RUN,Kind.JOBS,Kind.MAIN])

    def test_failed_ci_or_wrong_main_prevents_download(self):
        for kind in (Kind.RUN,Kind.MAIN):
            args,pins,tar,responses,calls,read,download=self.fixture()
            value=json.loads(responses[kind])
            if kind==Kind.RUN: value['conclusion']='failure'
            else: value['ref']='refs/heads/other'
            responses[kind]=json.dumps(value).encode()
            with patch('deployment.acquire.read',read),patch('deployment.acquire.download') as fetch:
                with self.assertRaises(ValueError): acquire('token',**pins)
                fetch.assert_not_called()

    def test_changes_during_download_cannot_return_candidate(self):
        for kind in (Kind.RUN,Kind.JOBS,Kind.MAIN):
            args,pins,tar,responses,calls,read,download=self.fixture()
            def changed(query,token,**options):
                data=download(query,token,**options)
                value=json.loads(responses[kind])
                if kind==Kind.RUN: value['run_attempt']=2
                elif kind==Kind.JOBS: value['jobs'][0]['conclusion']='failure'
                else: value['object']['sha']='f'*40
                responses[kind]=json.dumps(value).encode()
                return data
            with patch('deployment.acquire.read',read),patch('deployment.acquire.download',changed):
                with self.assertRaises(ValueError): acquire('token',**pins)

    def test_corrupted_archive_cannot_be_hidden_by_good_ci(self):
        args,pins,tar,responses,calls,read,download=self.fixture()
        with patch('deployment.acquire.read',read),patch('deployment.acquire.download',return_value=b'bad'):
            with self.assertRaises(ValueError): acquire('token',**pins)
        self.assertEqual(calls.count(Kind.RUN),1)

    def test_shared_budget_expiration_and_invalid_input(self):
        args,pins,tar,responses,calls,read,download=self.fixture()
        def expired(query,token,**options):
            data=download(query,token,**options)
            clock[0]=301
            return data
        clock=[0]
        with patch('deployment.acquire.time.monotonic',side_effect=lambda:clock[0]),patch('deployment.acquire.read',read),patch('deployment.acquire.download',expired) as fetch:
            with self.assertRaisesRegex(ValueError,'deadline'): acquire('token',**pins)
        with patch('deployment.acquire.read') as reader:
            for value in (True,0,301,float('nan')):
                with self.assertRaises(ValueError): acquire('token',**pins,timeout=value)
            reader.assert_not_called()


if __name__=='__main__': unittest.main()
