import json
import unittest

from deployment.candidate import prepare
import test_artifact
import test_candidate


class CandidatePayloadTests(unittest.TestCase):
    def fixture(self):
        meta, archive, pins, tar = test_artifact.ArtifactTests().fixture()
        run, jobs = test_candidate.CandidateTests().fixture()
        run.update(id=8, head_sha=pins['source_commit'], url='https://api.github.com/repos/neknaj/NEPL3/actions/runs/8')
        for job in jobs['jobs']:
            job.update(run_id=8, head_sha=pins['source_commit'])
        kwargs = dict(pins, workflow_id=8, current_main=pins['source_commit'])
        del kwargs['source_commit']
        return run, jobs, meta, archive, kwargs, tar

    def call(self, run, jobs, meta, archive, kwargs):
        return prepare(json.dumps(run).encode(), json.dumps(jobs).encode(), json.dumps(meta).encode(), archive, **kwargs)

    def test_checked_run_and_real_doc_payload_share_one_identity(self):
        run, jobs, meta, archive, kwargs, tar = self.fixture()
        candidate, payload = self.call(run, jobs, meta, archive, kwargs)
        self.assertEqual(payload.data, tar)
        self.assertEqual((candidate.run_id,candidate.attempt), (8,1))

    def test_other_attempt_or_legacy_name_cannot_supply_payload(self):
        run, jobs, meta, archive, kwargs, _ = self.fixture()
        for name in ['doc-browser-'+kwargs['current_main'], 'doc-browser-'+kwargs['current_main']+'-8-2',
                     'doc-browser-'+kwargs['current_main']+'-9-1']:
            with self.subTest(name=name), self.assertRaises(ValueError):
                self.call(run,jobs,dict(meta,name=name),archive,kwargs)
        # Renaming the artifact is insufficient if API job identities are old.
        run['run_attempt'] = 2; kwargs['attempt'] = 2
        meta['name'] = 'doc-browser-'+kwargs['current_main']+'-8-2'
        with self.assertRaises(ValueError): self.call(run,jobs,meta,archive,kwargs)

    def test_successful_payload_does_not_override_failed_ci(self):
        run, jobs, meta, archive, kwargs, _ = self.fixture()
        jobs['jobs'][0]['conclusion'] = 'failure'
        with self.assertRaises(ValueError): self.call(run,jobs,meta,archive,kwargs)
