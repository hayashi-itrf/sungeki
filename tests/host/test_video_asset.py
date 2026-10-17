"""Common registration/persistence only; byte fixture is NOT a decoded movie."""
import hashlib
import json
import tempfile
import time
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class VideoAsset(unittest.TestCase):
    def test_register_save_reopen_collect_and_reject_invalid_metadata(self):
        with tempfile.TemporaryDirectory(prefix='vvm-video-asset-', dir='/tmp') as tmp:
            root = Path(tmp).resolve()
            source = root / 'original.mov'
            original = bytes([42]) * 4096
            source.write_bytes(original)
            fixture = Path(__file__).resolve().parents[1] / 'fixtures/video-project.json'
            asset = json.loads(fixture.read_text())['assets'][0]
            asset['locator']['path'] = str(source)
            asset['content_hash'] = asset['probe']['info']['sha256'] = hashlib.sha256(original).hexdigest()
            host, client = HostProcess(root), None
            def invoke(name, **args):
                response = call(client, name, **args)
                self.assertFalse(response.get('isError'), response)
                return response['structuredContent']
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 82)
                client = McpClient(root)
                invoke('create_project', title='動画素材の登録・保存・持ち出し検証')
                bad = json.loads(json.dumps(asset))
                bad['probe']['info']['metadata']['video']['clock']['time_base']['denominator'] = 0
                result = call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                              commands=[dict(type='register_asset', asset=bad)])
                self.assertTrue(result.get('isError'), result)
                self.assertFalse(invoke('get_project')['snapshot']['project']['assets'])
                snap = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                              commands=[dict(type='register_asset', asset=asset)])['snapshot']
                self.assertEqual(snap['project']['assets'], [asset])
                invoke('save_project', path=str(root / 'saved.vvm.json'))
                invoke('collect_project', job_id=(job_id := str(uuid.uuid4())),
                       session_id=snap['session_id'], expected_revision=snap['revision'],
                       destination=str(root / 'package'))
                deadline = time.monotonic() + 10
                while time.monotonic() < deadline:
                    job = invoke('get_collection_job', job_id=job_id)['job']
                    if job['phase'] in ('succeeded', 'failed', 'cancelled', 'interrupted'):
                        break
                    time.sleep(.02)
                self.assertEqual(job['phase'], 'succeeded', job)
                self.assertEqual(source.read_bytes(), original)
            finally:
                if client: client.close()
                host.close()
            source.unlink()
            (root / 'package').rename(root / 'moved')
            host, client = HostProcess(root), None
            try:
                host.ready()
                client = McpClient(root)
                snap = invoke('open_project', path=str(root / 'moved/project.vvm.json'))['snapshot']
                restored = snap['project']['assets'][0]
                self.assertEqual(restored['probe'], asset['probe'])
                local = Path(restored['locator']['path'])
                self.assertTrue(local.is_relative_to(root / 'moved'))
                self.assertEqual(local.read_bytes(), original)
                self.assertEqual(local.suffix, '.mov')
            finally:
                if client: client.close()
                host.close()
