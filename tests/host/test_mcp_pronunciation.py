import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


class PronunciationTest(unittest.TestCase):
    def test_dictionary_edit_preview_save_undo_and_conflict(self):
        with tempfile.TemporaryDirectory(prefix='vvm-pronunciation-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            clients = []
            try:
                host.ready()
                a, b = McpClient(root), McpClient(root)
                clients += [a, b]

                def ok(client, tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                initial = ok(a, 'create_project', title='発音辞書の保存・競合検証')['snapshot']
                sid = initial['session_id']
                ok(b, 'attach_session', session_id=sid)
                entries = [{'written': 'ABC', 'spoken': 'DEF'}, {'written': 'DEF', 'spoken': 'デフ'},
                           {'written': '羅星詩杏', 'spoken': 'らすたしあん'}]
                command = {'type': 'set_pronunciation_dictionary', 'entries': entries}
                operation = str(uuid.uuid4())
                changed = ok(a, 'apply_commands', operation_id=operation, commands=[command])
                self.assertEqual(changed['snapshot']['project']['pronunciation_dictionary'], entries)
                retried = ok(a, 'apply_commands', operation_id=operation, commands=[command])
                self.assertEqual(retried['receipt'], changed['receipt'])
                result = ok(a, 'preview_pronunciation', subtitle_text='ABC DEF 羅星詩杏 羅星詩杏')
                self.assertEqual(result['final_text'], 'DEF デフ らすたしあん らすたしあん')
                self.assertEqual(result['revision'], 1)
                self.assertFalse(result['explicit_reading'])
                self.assertEqual(result['applied'][-1]['occurrences'], 2)
                explicit = ok(a, 'preview_pronunciation', subtitle_text='字幕', spoken_text='ABC')
                self.assertEqual(explicit['final_text'], 'ABC')
                self.assertTrue(explicit['explicit_reading'])
                self.assertEqual(explicit['applied'], [])
                invalid = call(a, 'preview_pronunciation', subtitle_text='字幕', spoken_text=' ')
                self.assertTrue(invalid['isError'])
                self.assertEqual(invalid['structuredContent']['code'], 'VALIDATION_FAILED')
                stale = call(b, 'preview_pronunciation', subtitle_text='ABC')
                self.assertTrue(stale['isError'])
                self.assertEqual(stale['structuredContent']['code'], 'CONFLICT')
                ok(b, 'preview_pronunciation', subtitle_text='ABC', expected_revision=1)
                # A read-only text preview is not a new editable Project observation.
                stale_edit = call(b, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[command])
                self.assertTrue(stale_edit['isError'])
                self.assertEqual(stale_edit['structuredContent']['code'], 'CONFLICT')
                invalid = call(a, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    {'type': 'rename_project', 'title': 'must roll back'},
                    {'type': 'set_pronunciation_dictionary', 'entries': entries + [entries[0]]}])
                self.assertTrue(invalid['isError'])
                before_save = ok(a, 'get_project')['snapshot']
                self.assertEqual(before_save, changed['snapshot'])
                path = root / '発音辞書の保存・競合検証.vvm.json'
                saved = ok(a, 'save_project', path=str(path))['snapshot']
                self.assertFalse(saved['dirty'])
                self.assertEqual(json.loads(path.read_text()), saved['project'])
                undone = ok(a, 'undo', operation_id=str(uuid.uuid4()), expected_operation_id=operation)['snapshot']
                self.assertEqual(undone['project'], initial['project'])
                redone = ok(a, 'redo', operation_id=str(uuid.uuid4()), expected_operation_id=operation)['snapshot']
                self.assertEqual(redone['project'], saved['project'])
                reopened = ok(a, 'open_project', path=str(path))['snapshot']
                self.assertEqual(reopened['project'], saved['project'])
            finally:
                for client in clients:
                    client.close()
                host.close()
