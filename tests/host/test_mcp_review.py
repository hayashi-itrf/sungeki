import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call

class McpReviewTest(unittest.TestCase):
    def test_review_jobs_do_not_change_edit_observation_or_repeat_after_refresh(self):
        with tempfile.TemporaryDirectory(prefix='vvm-mcp-review-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            a = b = None
            try:
                host.ready(); a = McpClient(root); b = McpClient(root)
                definitions = a.call('tools/list')['result']['tools']
                self.assertTrue({'review_project', 'get_job', 'list_jobs', 'cancel_job'}.issubset({t['name'] for t in definitions}))
                source = call(a, 'create_project', title='MCP確認ジョブの再送と競合検証')['structuredContent']['snapshot']
                sid = source['session_id']; jid = str(uuid.uuid4())
                call(b, 'attach_session', session_id=sid)
                result = call(a, 'review_project', job_id=jid, wait_ms=2000)
                self.assertFalse(result.get('isError'), result)
                job = result['structuredContent']['job']
                self.assertEqual(job['phase'], 'succeeded')
                self.assertEqual(job['report']['diagnostics']['summary']['not_run'], 4)
                self.assertEqual(call(a, 'get_project')['structuredContent']['snapshot'], source)
                call(b, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[{'type': 'rename_project', 'title': '別クライアントが編集'}])
                refreshed_job = call(a, 'get_job', job_id=jid)['structuredContent']['job']
                self.assertTrue(refreshed_job['stale'])
                # Reading a job is not observing the current editable document revision.
                conflict = call(a, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[{'type': 'rename_project', 'title': '古い観測'}])
                self.assertEqual(conflict['structuredContent']['code'], 'CONFLICT')
                call(a, 'get_project')
                replay = call(a, 'review_project', job_id=jid)['structuredContent']['job']
                self.assertEqual(replay, refreshed_job)
                self.assertEqual(call(a, 'list_jobs', session_id=sid)['structuredContent']['job_ids'], [jid])
                self.assertEqual(call(a, 'cancel_job', job_id=jid)['structuredContent']['job'], replay)
                invalid = call(a, 'review_project', job_id=str(uuid.uuid4()), wait_ms=30001)
                self.assertTrue(invalid['isError'])
                self.assertEqual(call(a, 'list_jobs', session_id=sid)['structuredContent']['job_ids'], [jid])
                retried_id = str(uuid.uuid4())
                rejected = call(a, 'review_project', job_id=retried_id, expected_revision=0)
                self.assertTrue(rejected['isError'])
                accepted = call(a, 'review_project', job_id=retried_id, expected_revision=1, wait_ms=2000)['structuredContent']['job']
                self.assertEqual(call(a, 'review_project', job_id=retried_id)['structuredContent']['job'], accepted)
            finally:
                if a: a.close()
                if b: b.close()
                host.close()
