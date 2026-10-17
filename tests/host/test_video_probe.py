"""Real movie inspection and registration through the new common MCP API."""
import hashlib
import json
import os
import subprocess
import tempfile
import unittest
import uuid
from pathlib import Path
from unittest.mock import patch
from support import HostProcess, McpClient
from test_mcp_editing import call


@unittest.skipUnless(all(os.environ.get(k) for k in ('VVM_TEST_FFMPEG', 'VVM_TEST_FFPROBE', 'VVM_MEDIA_TEST_ROOT')),
                     'requires real FFmpeg/FFprobe and SSD test root')
class VideoProbe(unittest.TestCase):
    def test_real_probe_register_save_and_errors_leave_project_unchanged(self):
        with tempfile.TemporaryDirectory(prefix='video-mcp-', dir=os.environ['VVM_MEDIA_TEST_ROOT']) as work, \
                tempfile.TemporaryDirectory(prefix='vvm-vp-', dir='/tmp') as socket_root:
            work, socket_root = Path(work).resolve(), Path(socket_root).resolve()
            source = work / '検査用映像.mp4'
            subprocess.run([os.environ['VVM_TEST_FFMPEG'], '-nostdin', '-v', 'error', '-f', 'lavfi', '-i',
                            'testsrc2=size=160x120:rate=30:duration=1', '-f', 'lavfi', '-i',
                            'sine=frequency=440:sample_rate=48000:duration=1', '-c:v', 'libx264',
                            '-c:a', 'aac', '-shortest', str(source)], check=True, capture_output=True, timeout=30)
            original = source.read_bytes()
            with patch.dict(os.environ, dict(SUNGEKI_RENDER_FFPROBE=os.environ['VVM_TEST_FFPROBE'],
                                            SUNGEKI_RENDER_WORK_ROOT=str(work / 'probe-work'))):
                host, client = HostProcess(socket_root, work / 'data'), None
                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']
                try:
                    self.assertGreaterEqual(host.ready()['protocol_minor'], 84)
                    client = McpClient(socket_root, data_dir=work / 'data')
                    before = invoke('create_project', title='実動画の原本検査・登録検証')['snapshot']
                    probe = invoke('probe_video', path=str(source))['probe']
                    self.assertEqual(probe['sha256'], hashlib.sha256(original).hexdigest())
                    self.assertEqual(probe['byte_length'], len(original))
                    self.assertEqual(probe['metadata']['container'], 'mov')
                    self.assertEqual(probe['metadata']['video']['width'], 160)
                    self.assertEqual(probe['metadata']['video']['frame_count'], 30)
                    self.assertEqual(probe['metadata']['audio'][0]['sample_rate'], 48000)
                    self.assertEqual(invoke('get_project')['snapshot'], before)
                    broken = work / 'broken.mp4'
                    broken.write_bytes(b'not a movie')
                    for path, code in [('relative.mp4', 'VALIDATION_FAILED'), (str(work / 'missing.mp4'), 'ASSET_MISSING'),
                                       (str(broken), 'VALIDATION_FAILED')]:
                        result = call(client, 'probe_video', path=path)
                        self.assertTrue(result.get('isError'), result)
                        self.assertEqual(result['structuredContent']['code'], code)
                        self.assertEqual(invoke('get_project')['snapshot'], before)
                    self.assertFalse(list((work / 'probe-work').glob('video-probe-*')))
                    asset = dict(id=str(uuid.uuid4()), name='検査済みの動画', source=None, credit='検査用自作映像',
                                 locator=dict(type='absolute', path=str(source)), content_hash=probe['sha256'],
                                 probe=dict(type='video', info=probe))
                    track_id, clip_id = str(uuid.uuid4()), str(uuid.uuid4())
                    playback = dict(source_start_us=100000, source_duration_us=500000, looped=False,
                                    audio_stream=probe['metadata']['audio'][0]['index'], gain_db=-3)
                    clip = dict(id=clip_id, track_id=track_id, scene_id=None, start_us=0, duration_us=500000,
                                transform=dict(x=0, y=0, width=160, height=120, opacity=1),
                                content=dict(type='video', asset_id=asset['id'], playback=playback))
                    snap = invoke('apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        dict(type='register_asset', asset=asset),
                        dict(type='add_track', track=dict(id=track_id, name='動画', kind='visual', visible=True, muted=False)),
                        dict(type='add_clip', clip=clip)])['snapshot']
                    trim = dict(playback, source_start_us=800000, source_duration_us=200000)
                    update = dict(type='set_video', clip_id=clip_id, asset_id=asset['id'], playback=trim)
                    rejected = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[update])
                    self.assertTrue(rejected.get('isError'), rejected)
                    self.assertEqual(invoke('get_project')['snapshot'], snap)
                    operation = str(uuid.uuid4())
                    changed = invoke('apply_commands', operation_id=operation, commands=[update,
                        dict(type='resize_clip', clip_id=clip_id, duration_us=200000)])['snapshot']
                    self.assertEqual(changed['project']['clips'][0]['content']['playback'], trim)
                    undone = invoke('undo', operation_id=str(uuid.uuid4()), expected_operation_id=operation)['snapshot']
                    self.assertEqual(undone['project'], snap['project'])
                    document = work / '実動画素材の登録検証.vvm.json'
                    invoke('save_project', path=str(document))
                    saved = json.loads(document.read_text())
                    self.assertEqual(saved['assets'][0]['probe'], asset['probe'])
                    self.assertEqual(saved['clips'][0]['content'], clip['content'])
                    self.assertEqual(saved['assets'][0]['locator']['type'], 'project_relative')
                    self.assertEqual(snap['project']['assets'], [asset])
                    self.assertEqual(source.read_bytes(), original)
                finally:
                    if client: client.close()
                    host.close()
                # A fresh recovery database must resolve the saved relative
                # movie reference, not reuse the writer session's absolute path.
                moved = work / 'relocated'
                moved.mkdir()
                moved_source = moved / source.name
                source.rename(moved_source)
                moved_document = moved / document.name
                document.rename(moved_document)
                saved_bytes = moved_document.read_bytes()
                host, client = HostProcess(socket_root, work / 'reader-data'), None
                try:
                    host.ready()
                    client = McpClient(socket_root, data_dir=work / 'reader-data')
                    reopened = invoke('open_project', path=str(moved_document))['snapshot']
                    self.assertFalse(source.exists())
                    self.assertFalse(document.exists())
                    self.assertFalse(reopened['dirty'])
                    self.assertEqual(reopened['project']['assets'][0]['locator'],
                                     dict(type='absolute', path=str(moved_source)))
                    self.assertEqual(reopened['project']['assets'][0]['probe'], asset['probe'])
                    self.assertEqual(reopened['project']['clips'][0]['content'], clip['content'])
                    report = invoke('review_project', job_id=str(uuid.uuid4()), wait_ms=5000)['job']
                    self.assertEqual(report['phase'], 'succeeded')
                    self.assertFalse(report['report']['diagnostics']['issues'])
                    moved_source.write_bytes(bytes([original[0] ^ 1]) + original[1:])
                    changed_report = invoke('review_project', job_id=str(uuid.uuid4()), wait_ms=5000)['job']
                    self.assertEqual(changed_report['phase'], 'succeeded')
                    self.assertIn('VIDEO_SOURCE_CHANGED',
                                  [issue['code'] for issue in changed_report['report']['diagnostics']['issues']])
                    moved_source.unlink()
                    missing_report = invoke('review_project', job_id=str(uuid.uuid4()), wait_ms=5000)['job']
                    self.assertEqual(missing_report['phase'], 'succeeded')
                    self.assertIn('VIDEO_SOURCE_MISSING',
                                  [issue['code'] for issue in missing_report['report']['diagnostics']['issues']])
                    moved_source.write_bytes(original)
                    after = invoke('get_project')['snapshot']
                    for key in ('project', 'revision', 'dirty', 'undo_operation_id', 'redo_operation_id'):
                        self.assertEqual(after[key], reopened[key], key)
                    self.assertEqual(moved_source.read_bytes(), original)
                    self.assertEqual(moved_document.read_bytes(), saved_bytes)
                finally:
                    if client: client.close()
                    host.close()
