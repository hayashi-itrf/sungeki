import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class ReviewUnavailableTest(unittest.TestCase):
    def test_known_start_rejection_keeps_editing_available_and_recovers_without_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-review-unavailable-', dir='/tmp') as directory:
            root = Path(directory)
            work = root / 'data/recovery.sqlite.review-work'
            work.parent.mkdir(mode=0o700)
            work.write_bytes(b'not a work directory')
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']
                before = invoke('create_project', title='画像検査を開始できない場合の復帰')['snapshot']
                sid, jid = before['session_id'], str(uuid.uuid4())
                rejected = call(client, 'review_project', job_id=jid)
                self.assertTrue(rejected['isError'])
                error = rejected['structuredContent']
                self.assertEqual(error['code'], 'CAPABILITY_UNAVAILABLE')
                self.assertEqual(error['details']['job_created'], 'false')
                self.assertTrue(error['retryable'])
                self.assertEqual(invoke('list_jobs', session_id=sid)['job_ids'], [])
                edited = invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    {'type': 'rename_project', 'title': '作業領域が使えなくても編集は継続'}])['snapshot']
                self.assertEqual(edited['revision'], before['revision'] + 1)
                self.assertEqual(work.read_bytes(), b'not a work directory')
                work.unlink()  # Own isolated fault fixture only; no host/app restart.
                completed = invoke('review_project', job_id=str(uuid.uuid4()), wait_ms=5000)['job']
                self.assertEqual(completed['phase'], 'succeeded')
                self.assertEqual(completed['source_revision'], edited['revision'])
                self.assertEqual(invoke('get_project')['snapshot']['project'], edited['project'])
            finally:
                if client:
                    client.close()
                host.close()
