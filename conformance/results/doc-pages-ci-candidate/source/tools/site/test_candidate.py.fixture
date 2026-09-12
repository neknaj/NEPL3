import copy
import json
import unittest

from deployment.candidate import verify


class CandidateTests(unittest.TestCase):
    def fixture(self):
        repo = dict(id=9, full_name='neknaj/NEPL3')
        run = dict(id=7, workflow_id=8, run_attempt=1, repository=repo, head_repository=repo,
                   url='https://api.github.com/repos/neknaj/NEPL3/actions/runs/7',
                   path='.github/workflows/ci.yml', event='push', head_branch='main', head_sha='a'*40,
                   status='completed', conclusion='success')
        # Expected gates from ci.yml, independent of the verifier's constant.
        names = ['quality', 'native (ubuntu-latest)', 'native (windows-latest)', 'native (macos-latest)',
                 'wasi and wasm compilation', 'baremetal ARMv6-M build',
                 'baremetal RP2040 emulator execution', 'Doc HTML browser layout']
        jobs = dict(total_count=8, jobs=[dict(id=i+1, run_id=7, run_attempt=1, head_sha='a'*40,
                    name=n, status='completed', conclusion='success') for i,n in enumerate(names)])
        return run, jobs

    def check(self, run, jobs, **changes):
        kwargs = dict(owner='neknaj', repository='NEPL3', repository_id=9, workflow_id=8,
                      run_id=7, attempt=1, current_main='a'*40)
        return verify(json.dumps(run).encode(), json.dumps(jobs).encode(), **dict(kwargs, **changes))

    def test_complete_push_and_manual_run(self):
        run, jobs = self.fixture()
        for event in ('push', 'workflow_dispatch'):
            run['event'] = event
            candidate = self.check(run, jobs)
            self.assertEqual((candidate.run_id, candidate.attempt, candidate.source_commit), (7, 1, 'a'*40))

    def test_wrong_run_fork_branch_attempt_or_stale_source(self):
        run, jobs = self.fixture()
        for key, value in [('event','pull_request'), ('head_branch','feature'), ('head_sha','b'*40),
                           ('run_attempt',2), ('workflow_id',88), ('id',True),
                           ('head_repository',dict(id=9,full_name='other/NEPL3')),
                           ('path','.github/workflows/other.yml'), ('conclusion','failure'), ('status','in_progress')]:
            with self.subTest(key=key), self.assertRaises(ValueError):
                self.check(dict(run, **{key:value}), jobs)
        with self.assertRaises(ValueError): self.check(run, jobs, current_main='b'*40)

    def test_missing_duplicate_and_partial_job_pages(self):
        run, jobs = self.fixture()
        for value in [dict(jobs, total_count=9), dict(total_count=7,jobs=jobs['jobs'][:-1]),
                      dict(total_count=9,jobs=jobs['jobs']+[jobs['jobs'][0]]),
                      dict(total_count=9,jobs=jobs['jobs']+[dict(jobs['jobs'][0],id=99)])]:
            with self.subTest(value=value), self.assertRaises(ValueError): self.check(run,value)

    def test_job_failure_and_identity_mismatch(self):
        run, original = self.fixture()
        for key, value in [('conclusion','skipped'), ('conclusion','failure'), ('status','queued'),
                           ('run_id',8), ('run_attempt',2), ('head_sha','b'*40), ('id',True)]:
            jobs = copy.deepcopy(original); jobs['jobs'][0][key] = value
            with self.subTest(key=key,value=value), self.assertRaises(ValueError): self.check(run,jobs)

    def test_optional_delivery_skip_is_distinct_from_failure(self):
        run, jobs = self.fixture()
        delivery = dict(jobs['jobs'][0], id=90, name='deliver source', conclusion='skipped')
        jobs['jobs'].append(delivery); jobs['total_count'] += 1
        self.check(run, jobs)
        delivery['conclusion'] = 'failure'
        with self.assertRaises(ValueError): self.check(run, jobs)
