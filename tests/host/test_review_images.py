import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_image_probe import png


class ReviewImagesTest(unittest.TestCase):
    def test_real_jobs_capture_report_failures_and_leave_project_unchanged(self):
        with tempfile.TemporaryDirectory(prefix='vvm-review-images-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                def invoke(name, **args):
                    response = call(client, name, **args)
                    self.assertFalse(response.get('isError'), response)
                    return response['structuredContent']
                source = root / '原本.png'
                source.write_bytes(png())
                probe = invoke('probe_image', path=str(source))['probe']
                invoke('create_project', title='確認レポート・原本の欠落と変更の検証')
                aid = str(uuid.uuid4())
                asset = {'id': aid, 'name': '確認用の画像', 'source': None, 'credit': '試験用',
                         'locator': {'type': 'absolute', 'path': str(source)},
                         'probe': {'type': 'image', 'info': probe}, 'content_hash': probe['sha256']}
                before = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                                commands=[{'type': 'register_asset', 'asset': asset}])['snapshot']
                work = root / 'data/recovery.sqlite.review-work'
                def review():
                    jid = str(uuid.uuid4())
                    job = invoke('review_project', job_id=jid, wait_ms=5000)['job']
                    self.assertEqual(job['phase'], 'succeeded', job)
                    self.assertFalse((work / jid).exists())
                    self.assertEqual(invoke('get_project')['snapshot'], before)
                    return job['report']
                valid = review()
                self.assertEqual(valid['diagnostics']['summary']['passed'], 2)
                self.assertEqual(valid['diagnostics']['summary']['not_run'], 4)
                self.assertEqual(len(valid['asset_fingerprint']), 64)
                source.write_bytes(b'changed since registration')
                changed = review()
                self.assertEqual(changed['diagnostics']['summary']['failed'], 1)
                self.assertEqual(changed['diagnostics']['issues'][0]['code'], 'ASSET_CHANGED')
                self.assertEqual(changed['diagnostics']['issues'][0]['asset_id'], aid)
                self.assertIsNone(changed['asset_fingerprint'])
                source.unlink()
                missing = review()
                self.assertEqual(missing['diagnostics']['issues'][0]['code'], 'ASSET_MISSING')
                self.assertIsNone(missing['asset_fingerprint'])
                source.write_bytes(png())
                self.assertEqual(review()['asset_fingerprint'], valid['asset_fingerprint'])
                blocked_id = str(uuid.uuid4())
                blocked = work / blocked_id
                blocked.mkdir()
                (blocked / 'keep').write_bytes(b'preexisting')
                failed = invoke('review_project', job_id=blocked_id, wait_ms=5000)['job']
                self.assertEqual(failed['phase'], 'failed')
                self.assertEqual(failed['error']['code'], 'IO_ERROR')
                self.assertIsNone(failed['report'])
                self.assertEqual((blocked / 'keep').read_bytes(), b'preexisting')
                self.assertEqual(invoke('get_project')['snapshot'], before)
            finally:
                if client:
                    client.close()
                host.close()
