from support import PROTOCOL_MINOR
"""Real stdio tracking of an interrupted build; no engine is started."""
import json
import tempfile
import time
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient, ROOT
from test_mcp_editing import call


class BuildWaitTest(unittest.TestCase):
    def test_wait_returns_same_terminal_parent_and_never_restarts_it(self):
        with tempfile.TemporaryDirectory(prefix='vvm-build-wait-', dir='/tmp') as directory:
            root = Path(directory)
            host, client = HostProcess(root), None
            try:
                self.assertEqual(host.ready(30)['protocol_minor'], PROTOCOL_MINOR)
                client = McpClient(root, response_timeout=35)

                def ok(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                tools = client.call('tools/list')['result']['tools']
                schema = next(t['inputSchema'] for t in tools if t['name'] == 'get_build_video')
                self.assertIn('wait_ms', schema['properties'])
                recipe = json.loads((ROOT/'tests/fixtures/recipes/build-dialogue.json').read_text())
                for asset in recipe['assets']:
                    asset['locator'] = {'type': 'absolute', 'path': str(root/'intentionally-missing.png')}
                invalid_id = str(uuid.uuid4())
                invalid = call(client, 'build_video', job_id=invalid_id, recipe=recipe,
                    project_path=str(root/'不正待機.vvm.json'), wait_ms=30001,
                    base_url='http://127.0.0.1:1', engine_version='0.16.1')
                self.assertTrue(invalid.get('isError'), invalid)
                self.assertEqual(ok('list_build_videos', limit=100)['job_ids'], [])
                job_id = str(uuid.uuid4())
                start_args = dict(job_id=job_id, recipe=recipe,
                    project_path=str(root/'素材不足の待機検証.vvm.json'),
                    base_url='http://127.0.0.1:1', engine_version='0.16.1')
                initial = ok('build_video', **start_args, wait_ms=30000)['job']
                self.assertEqual(initial['phase'], 'interrupted')
                replay = ok('build_video', **start_args, wait_ms=0)['job']
                self.assertEqual(replay, initial)
                terminal = ok('get_build_video', job_id=job_id, wait_ms=30000)['job']
                self.assertEqual(terminal['job_id'], job_id)
                self.assertEqual(terminal['session_id'], initial['session_id'])
                self.assertEqual(terminal['phase'], 'interrupted')
                started = time.monotonic()
                again = ok('get_build_video', job_id=job_id, wait_ms=30000)['job']
                self.assertLess(time.monotonic()-started, 2)
                self.assertEqual(again, terminal)
                self.assertEqual(ok('list_build_videos', limit=100)['job_ids'], [job_id])
                self.assertTrue(call(client, 'get_build_video', job_id=job_id, wait_ms=30001).get('isError'))
                self.assertFalse((root/'素材不足の待機検証.vvm.json').exists())
            finally:
                if client:
                    client.close()
                host.close()
