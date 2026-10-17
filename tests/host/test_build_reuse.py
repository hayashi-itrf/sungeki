"""Real MCP transport/schema rejection; does not exercise successful synthesis."""
import json
from pathlib import Path
import tempfile
import unittest
import uuid
from support import HostProcess, McpClient, ROOT
from test_mcp_editing import call


class BuildReuseTest(unittest.TestCase):
    def test_reuse_schema_and_missing_source_leave_no_parent(self):
        with tempfile.TemporaryDirectory(prefix='vvm-reuse-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 123)
                client = McpClient(root)
                tools = client.call('tools/list')['result']['tools']
                schema = next(t['inputSchema'] for t in tools if t['name'] == 'build_video')
                self.assertIn('reuse_from', schema['properties'])
                job_id = str(uuid.uuid4())
                result = call(client, 'build_video', job_id=job_id,
                    recipe=json.loads((ROOT/'tests/fixtures/recipes/build-dialogue.json').read_text()),
                    project_path=str(root/'再利用元がない制作.vvm.json'),
                    base_url='http://127.0.0.1:1', engine_version='0.16.1',
                    reuse_from={'parent_job_id': str(uuid.uuid4()), 'speech': {'0': 0}})
                self.assertTrue(result.get('isError'), result)
                self.assertIn('再利用元の制作がありません', str(result))
                self.assertFalse((root/'再利用元がない制作.vvm.json').exists())
                listing = call(client, 'list_build_videos', limit=100)
                self.assertFalse(listing.get('isError'), listing)
                self.assertNotIn(job_id, str(listing['structuredContent']))
            finally:
                if client is not None:
                    client.close()
                host.close()
