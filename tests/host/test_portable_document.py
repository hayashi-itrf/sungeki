import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_image_probe import png


class PortableDocumentTest(unittest.TestCase):
    def test_moved_bundle_resolves_sources_and_preserves_save_undo_and_save_as(self):
        with tempfile.TemporaryDirectory(prefix='vvm-portable-', dir='/tmp') as directory:
            root = Path(directory).resolve()
            bundle = root / 'bundle'
            (bundle / 'assets').mkdir(parents=True)
            image = bundle / 'assets/original.png'
            image.write_bytes(png())
            host = HostProcess(root, data_dir=root / 'writer')
            client = None
            def invoke(name, **args):
                response = call(client, name, **args)
                self.assertFalse(response.get('isError'), response)
                return response['structuredContent']
            try:
                host.ready(); client = McpClient(root, data_dir=root / 'writer')
                invoke('create_project', title='持ち出し読込検証')
                probe = invoke('probe_image', path=str(image))['probe']
                asset = dict(id=str(uuid.uuid4()), name='画像', source=None, credit='試験画像',
                             locator=dict(type='absolute', path=str(image)), content_hash=probe['sha256'],
                             probe=dict(type='image', info=probe))
                invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=[dict(type='register_asset', asset=asset)])
                document = bundle / 'project.vvm.json'
                invoke('save_project', path=str(document))
                self.assertEqual(json.loads(document.read_text())['assets'][0]['locator'], dict(type='project_relative', path='assets/original.png'))
                before = invoke('get_project')['snapshot']
                moved = root / 'moved'
                bundle.rename(moved)
                document = moved / 'project.vvm.json'
                # Same inode under a different relative root must not silently
                # reuse the old session's absolute source references.
                self.assertTrue(call(client, 'open_project', path=str(document)).get('isError'))
                self.assertEqual(invoke('get_project')['snapshot'], before)
            finally:
                if client:
                    client.close()
                host.close()
            host = HostProcess(root, data_dir=root / 'reader'); client = None
            try:
                host.ready(); client = McpClient(root, data_dir=root / 'reader')
                snap = invoke('open_project', path=str(document))['snapshot']
                self.assertFalse(snap['dirty'])
                self.assertFalse(bundle.exists())
                expected = dict(type='absolute', path=str(moved / 'assets/original.png'))
                self.assertEqual(snap['project']['assets'][0]['locator'], expected)
                jid = str(uuid.uuid4())
                report = invoke('review_project', job_id=jid, wait_ms=5000)['job']
                self.assertEqual(report['phase'], 'succeeded')
                self.assertFalse(report['report']['diagnostics']['issues'])
                op = str(uuid.uuid4())
                invoke('apply_commands', operation_id=op, commands=[dict(type='rename_project', title='移動後の編集')])
                undone = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=op)['snapshot']
                self.assertEqual(undone['project'], snap['project'])
                invoke('save_project', path=str(document))
                self.assertNotIn(str(root), document.read_text())
                (root / 'elsewhere').mkdir()
                outside = root / 'elsewhere/copy.vvm.json'
                invoke('save_project', path=str(outside))
                self.assertEqual(json.loads(outside.read_text())['assets'][0]['locator'], expected)
                self.assertEqual(invoke('get_project')['snapshot']['project']['assets'][0]['locator'], expected)
                self.assertEqual((moved / 'assets/original.png').read_bytes(), png())
            finally:
                if client:
                    client.close()
                host.close()
