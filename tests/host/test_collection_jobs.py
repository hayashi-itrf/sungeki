from support import PROTOCOL_MINOR
import json
import tempfile
import time
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_image_probe import png


class CollectionJobs(unittest.TestCase):
    def test_collect_move_reopen_and_restart_history_via_real_mcp(self):
        with tempfile.TemporaryDirectory(prefix='vvm-collect-mcp-', dir='/tmp') as tmp:
            root = Path(tmp).resolve()
            image = root / 'original.png'
            image.write_bytes(png())
            host = HostProcess(root)
            client = None
            def invoke(name, **args):
                response = call(client, name, **args)
                self.assertFalse(response.get('isError'), response)
                return response['structuredContent']
            try:
                self.assertEqual(host.ready()['protocol_minor'], PROTOCOL_MINOR)
                client = McpClient(root)
                invoke('create_project', title='MCP収集と移動後編集の検証')
                probe = invoke('probe_image', path=str(image))['probe']
                asset = dict(id=str(uuid.uuid4()), name='持ち出し画像', source=None, credit='検証用自作画像',
                             locator=dict(type='absolute', path=str(image)), content_hash=probe['sha256'],
                             probe=dict(type='image', info=probe))
                snap = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                              commands=[dict(type='register_asset', asset=asset)])['snapshot']
                request = dict(session_id=snap['session_id'], expected_revision=snap['revision'],
                               job_id=str(uuid.uuid4()), destination=str(root / 'collection'))
                bad_parent = call(client, 'collect_project', **dict(request,
                    destination=str(root / 'missing-parent' / 'collection')))
                self.assertTrue(bad_parent.get('isError'))
                self.assertEqual(bad_parent['structuredContent']['code'], 'VALIDATION_FAILED')
                self.assertTrue(call(client, 'collect_project', **request, wait_ms=30001).get('isError'))
                self.assertEqual(invoke('list_collection_jobs')['job_ids'], [])
                job = invoke('collect_project', **request, wait_ms=30000)['job']
                started = time.monotonic()
                self.assertEqual(invoke('get_collection_job', job_id=request['job_id'], wait_ms=30000)['job'], job)
                self.assertLess(time.monotonic() - started, 1)
                self.assertEqual(image.read_bytes(), png())
                self.assertEqual(job['phase'], 'succeeded', job)
                self.assertEqual(job['destination'], request['destination'])
                self.assertFalse(job['output']['checkpoints_included'])
                self.assertEqual(invoke('get_project')['snapshot'], snap)
                self.assertEqual(invoke('collect_project', **request, wait_ms=0)['job'], job)
                self.assertEqual(invoke('cancel_collection_job', job_id=request['job_id'])['job'], job)
                self.assertEqual(invoke('list_collection_jobs')['job_ids'], [request['job_id']])
                self.assertTrue(call(client, 'collect_project', **dict(request, expected_revision=999)).get('isError'))
                document = Path(job['output']['project_path'])
                raw = json.loads(document.read_text())
                self.assertNotEqual(raw['id'], snap['project']['id'])
                self.assertEqual(raw['assets'][0]['locator']['type'], 'project_relative')
                self.assertEqual(raw['assets'][0]['credit'], '検証用自作画像')
            finally:
                if client:
                    client.close()
                host.close()
            image.unlink()
            (root / 'collection').rename(root / 'moved')
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                # Completed history is completion-time evidence, not a fresh existence claim.
                self.assertEqual(invoke('get_collection_job', job_id=request['job_id'])['job'], job)
                moved = invoke('open_project', path=str(root / 'moved/project.vvm.json'))['snapshot']
                self.assertFalse(moved['dirty'])
                asset_path = Path(moved['project']['assets'][0]['locator']['path'])
                self.assertTrue(asset_path.is_relative_to(root / 'moved'))
                self.assertEqual(asset_path.read_bytes(), png())
                review = invoke('review_project', job_id=str(uuid.uuid4()), wait_ms=5000)['job']
                self.assertEqual(review['phase'], 'succeeded')
                self.assertFalse(review['report']['diagnostics']['issues'])
                invoke('apply_commands', operation_id=str(uuid.uuid4()),
                       commands=[dict(type='rename_project', title='移動後の収集作品')])
                invoke('save_project', path=str(root / 'moved/project.vvm.json'))
            finally:
                if client:
                    client.close()
                host.close()

    def test_failure_codes_and_source_counts_survive_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-collect-errors-', dir='/tmp') as tmp:
            root = Path(tmp).resolve()
            host = HostProcess(root); client = None
            jobs = []
            def invoke(name, **args):
                response = call(client, name, **args)
                self.assertFalse(response.get('isError'), response)
                return response['structuredContent']
            try:
                host.ready(); client = McpClient(root)
                for reason, expected in [('missing', 'ASSET_MISSING'), ('changed', 'EXTERNAL_MODIFICATION'), ('exists', 'OUTPUT_EXISTS')]:
                    invoke('create_project', title='収集失敗原因:' + reason)
                    image = root / (reason + '.png'); image.write_bytes(png())
                    probe = invoke('probe_image', path=str(image))['probe']
                    asset = dict(id=str(uuid.uuid4()), name=reason, source=None, credit='',
                                 locator=dict(type='absolute', path=str(image)), content_hash=probe['sha256'],
                                 probe=dict(type='image', info=probe))
                    snap = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                                  commands=[dict(type='register_asset', asset=asset)])['snapshot']
                    target = root / (reason + '-output')
                    if reason == 'missing': image.unlink()
                    if reason == 'changed': image.write_bytes(b'X' * len(png()))
                    if reason == 'exists': target.mkdir(); (target / 'keep').write_text('keep')
                    jid = str(uuid.uuid4())
                    invoke('collect_project', session_id=snap['session_id'], job_id=jid,
                           expected_revision=snap['revision'], destination=str(target))
                    deadline = time.monotonic() + 10
                    while time.monotonic() < deadline:
                        job = invoke('get_collection_job', job_id=jid)['job']
                        if job['phase'] in ('failed', 'succeeded', 'cancelled', 'interrupted'): break
                        time.sleep(.02)
                    self.assertEqual(job['phase'], 'failed', job)
                    self.assertEqual(job['error']['code'], expected)
                    if reason != 'exists':
                        self.assertEqual(job['error']['details'][reason], '1')
                        self.assertFalse(target.exists())
                    else: self.assertEqual((target / 'keep').read_text(), 'keep')
                    jobs.append(job)
            finally:
                if client: client.close()
                host.close()
            host = HostProcess(root); client = None
            try:
                host.ready(); client = McpClient(root)
                for job in jobs:
                    self.assertEqual(invoke('get_collection_job', job_id=job['job_id'])['job'], job)
            finally:
                if client: client.close()
                host.close()

    def test_dry_run_pages_deduplicated_bytes_without_creating_output(self):
        with tempfile.TemporaryDirectory(prefix='vvm-collect-plan-', dir='/tmp') as tmp:
            root = Path(tmp).resolve(); host = HostProcess(root); client = None
            def invoke(name, **args):
                response = call(client, name, **args)
                self.assertFalse(response.get('isError'), response)
                return response['structuredContent']
            def finish(jid):
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    job = invoke('get_collection_job', job_id=jid)['job']
                    if job['phase'] in ('succeeded', 'failed', 'cancelled', 'interrupted'): return job
                    time.sleep(.02)
                self.fail('dry_run timeout')
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 78); client = McpClient(root)
                invoke('create_project', title='収集事前確認とページ検証')
                image = root / 'source.png'; image.write_bytes(png())
                probe = invoke('probe_image', path=str(image))['probe']
                assets = [dict(id=str(uuid.uuid4()), name='同じ原本' + str(i), source=None, credit='',
                               locator=dict(type='absolute', path=str(image)), content_hash=probe['sha256'],
                               probe=dict(type='image', info=probe)) for i in range(2)]
                snap = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                              commands=[dict(type='register_asset', asset=a) for a in assets])['snapshot']
                target = root / 'not-created'; jid = str(uuid.uuid4())
                request = dict(session_id=snap['session_id'], job_id=jid, expected_revision=snap['revision'],
                               destination=str(target), dry_run=True)
                job = invoke('collect_project', **request, wait_ms=30000)['job']
                self.assertEqual(job['phase'], 'succeeded'); self.assertIsNone(job['output'])
                self.assertTrue(job['dry_run']); self.assertTrue(job['plan']['sources_ready'])
                self.assertFalse(job['plan']['destination_exists'])
                self.assertEqual(job['plan']['source_count'], 2); self.assertEqual(job['plan']['unique_files'], 1)
                self.assertEqual(job['plan']['estimated_asset_bytes'], len(png()))
                first = invoke('read_collection_plan', job_id=jid, offset=0, limit=1)['page']
                second = invoke('read_collection_plan', job_id=jid, offset=1, limit=1)['page']
                self.assertEqual(first['items'][0]['asset_id'], assets[0]['id'])
                self.assertEqual(second['items'][0]['asset_id'], assets[1]['id'])
                self.assertEqual(first['items'][0]['status'], 'verified')
                self.assertEqual(invoke('read_collection_plan', job_id=jid, offset=2, limit=100)['page']['items'], [])
                self.assertTrue(call(client, 'read_collection_plan', job_id=jid, offset=3, limit=1).get('isError'))
                self.assertTrue(call(client, 'read_collection_plan', job_id=jid, offset=0, limit=101).get('isError'))
                self.assertTrue(call(client, 'collect_project', **dict(request, dry_run=False)).get('isError'))
                self.assertFalse(target.exists()); self.assertEqual(invoke('get_project')['snapshot'], snap)
                image.unlink(); target.mkdir(); (target / 'keep').write_text('keep')
                missing_request = dict(request, job_id=str(uuid.uuid4()))
                missing = invoke('collect_project', **missing_request, wait_ms=30000)['job']
                self.assertEqual(missing['phase'], 'succeeded')
                self.assertFalse(missing['plan']['sources_ready']); self.assertTrue(missing['plan']['destination_exists'])
                page = invoke('read_collection_plan', job_id=missing_request['job_id'], offset=0, limit=100)['page']
                self.assertEqual([i['status'] for i in page['items']], ['missing', 'missing'])
                self.assertEqual((target / 'keep').read_text(), 'keep')
                self.assertEqual(list(root.glob('.vvm-collect-*')), [])
            finally:
                if client: client.close()
                host.close()
            host = HostProcess(root); client = None
            try:
                host.ready(); client = McpClient(root)
                self.assertEqual(invoke('get_collection_job', job_id=jid)['job'], job)
                self.assertEqual(invoke('read_collection_plan', job_id=jid, offset=0, limit=1)['page'], first)
            finally:
                if client: client.close()
                host.close()


if __name__ == '__main__':
    unittest.main()
