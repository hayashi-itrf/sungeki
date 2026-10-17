import hashlib
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient, ROOT
from test_mcp_editing import call


class SubtitleExportTest(unittest.TestCase):
    def test_mcp_export_revision_retry_conflict_and_project_unchanged(self):
        with tempfile.TemporaryDirectory(prefix='vvm-sub-export-', dir='/tmp') as directory:
            root = Path(directory)
            host, client = HostProcess(root), None
            try:
                host.ready()
                client = McpClient(root)
                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']
                def rejected(**args):
                    result = call(client, 'export_subtitles', **args)
                    self.assertTrue(result.get('isError'), result)
                    return result
                # A real host opens a structural fixture; no TTS/media render is requested.
                source = root / '字幕出力の検証.vvm.json'
                project = json.loads((ROOT / 'tests/fixtures/psd-speech-project.json').read_text())
                for track in project['tracks']:
                    track['visible'] = True
                source.write_text(json.dumps(project, ensure_ascii=False))
                initial = ok('open_project', path=str(source))['snapshot']
                sid, revision = initial['session_id'], initial['revision']
                def args(name='字幕.srt', fmt='srt', rev=revision):
                    return dict(session_id=sid, expected_revision=rev, path=str(root / name), format=fmt)
                first = ok('export_subtitles', **args())['publication']
                text = (root / '字幕.srt').read_text()
                speech = [c for c in project['clips'] if c['content']['type'] == 'speech']
                self.assertTrue(speech)
                self.assertIn(speech[0]['content']['subtitle_text'], text)
                self.assertFalse(first['already_present'])
                self.assertFalse(first['stale'])
                self.assertEqual(first['source_revision'], revision)
                self.assertEqual(first['sha256'], hashlib.sha256(text.encode()).hexdigest())
                self.assertEqual(first['byte_length'], len(text.encode()))
                self.assertTrue(ok('export_subtitles', **args())['publication']['already_present'])
                ok('export_subtitles', **args('字幕.vtt', 'web_vtt'), include_text=True)
                self.assertTrue((root / '字幕.vtt').read_text().startswith('WEBVTT\n\n'))
                rejected(**args('誤った形式.srt', 'web_vtt'))
                self.assertFalse((root / '誤った形式.srt').exists())
                rejected(**args('古い版.srt', rev=revision + 1))
                self.assertFalse((root / '古い版.srt').exists())
                (root / '既存.srt').write_text('保持する字幕')
                rejected(**args('既存.srt'))
                self.assertEqual((root / '既存.srt').read_text(), '保持する字幕')
                current = ok('get_project', session_id=sid)['snapshot']
                self.assertEqual(current, initial)
                self.assertEqual(json.loads(source.read_text()), project)
                self.assertFalse(list(root.glob('.vvm-output-*.tmp')))
                # Editing changes revision; the old request must not silently use the new title.
                ok('apply_commands', session_id=sid, expected_revision=revision,
                   operation_id=str(uuid.uuid4()), commands=[dict(type='rename_project', title='更新後')])
                rejected(**args('編集前.srt'))
                self.assertFalse((root / '編集前.srt').exists())
            finally:
                if client:
                    client.close()
                host.close()
