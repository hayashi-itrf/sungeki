import os
import json
import hashlib
import wave
import tempfile
import time
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_image_probe import png


class ReviewFreshnessTest(unittest.TestCase):
    def test_background_source_changes_preserve_report_and_do_not_edit_project(self):
        self.check_source_changes('image')

    def test_audio_source_changes_preserve_report_and_do_not_edit_project(self):
        self.check_source_changes('audio')

    def test_psd_byte_changes_preserve_report_and_do_not_edit_project(self):
        self.check_source_changes('psd')

    def test_video_byte_changes_preserve_report_and_do_not_edit_project(self):
        self.check_source_changes('video')

    def check_source_changes(self, kind):
        with tempfile.TemporaryDirectory(prefix='vvm-review-freshness-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 20)
                client = McpClient(root)
                def invoke(name, **args):
                    response = call(client, name, **args)
                    self.assertFalse(response.get('isError'), response)
                    return response['structuredContent']
                source = root / {'audio': '音声.wav', 'image': '画像.png', 'psd': '立ち絵.psd', 'video': '動画.mp4'}[kind]
                if kind in ('psd', 'video'):
                    # Raw identity only; this synthetic source is never decoded.
                    original = bytes([42]) * 4096
                    source.write_bytes(original)
                    fixture = Path(__file__).parents[1] / f'fixtures/{kind}-project.json'
                    probe = json.loads(fixture.read_text())['assets'][0]['probe']['info']
                    probe['sha256'] = hashlib.sha256(original).hexdigest()
                    probe['byte_length'] = len(original)
                elif kind == 'audio':
                    with wave.open(str(source), 'wb') as wav:
                        wav.setnchannels(1); wav.setsampwidth(2); wav.setframerate(24000)
                        wav.writeframes(bytes(48000))
                    original = source.read_bytes()
                    probe = invoke('probe_audio', path=str(source))['probe']
                else:
                    original = png()
                    source.write_bytes(original)
                    probe = invoke('probe_image', path=str(source))['probe']
                invoke('create_project', title='原本変更の非同期照合検証')
                asset = {'id': str(uuid.uuid4()), 'name': '照合素材', 'source': None, 'credit': '試験用',
                         'locator': {'type': 'absolute', 'path': str(source)},
                         'probe': {'type': kind, 'info': probe}, 'content_hash': probe['sha256']}
                before = invoke('apply_commands', operation_id=str(uuid.uuid4()),
                    commands=[{'type': 'register_asset', 'asset': asset}])['snapshot']
                jid = str(uuid.uuid4())
                job = invoke('review_project', job_id=jid, wait_ms=5000)['job']
                report = job['report']
                self.assertEqual(job['phase'], 'succeeded')
                def observed(status, after=0):
                    deadline = time.monotonic() + 12
                    while time.monotonic() < deadline:
                        job = invoke('get_job', job_id=jid)['job']
                        freshness = job['asset_freshness']
                        self.assertEqual(job['report'], report)
                        if not freshness['checking'] and freshness['status'] == status and (freshness['checked_at_ms'] or 0) > after:
                            return job
                        time.sleep(0.05)
                    self.fail(f'freshness timeout: {job}')
                matching = observed('matching')
                self.assertFalse(matching['stale'])
                old_stat = source.stat()
                changed = bytearray(original)
                changed[-1] ^= 1
                source.write_bytes(changed)
                os.utime(source, ns=(old_stat.st_atime_ns, old_stat.st_mtime_ns))
                # Neither equal size nor restored mtime may hide a content change.
                stale = observed('changed', matching['asset_freshness']['checked_at_ms'])
                self.assertTrue(stale['stale'])
                self.assertTrue(stale['asset_freshness']['last_known_changed'])
                source.unlink()
                missing = observed('changed', stale['asset_freshness']['checked_at_ms'])
                self.assertIn('見つかりません', missing['asset_freshness']['message'])
                source.mkdir()
                unknown = observed('unavailable', missing['asset_freshness']['checked_at_ms'])
                self.assertTrue(unknown['stale'])  # Read failure cannot erase a known mismatch.
                source.rmdir()
                source.write_bytes(original)
                restored = observed('matching', unknown['asset_freshness']['checked_at_ms'])
                self.assertFalse(restored['stale'])
                self.assertFalse(restored['asset_freshness']['last_known_changed'])
                self.assertEqual(invoke('get_project')['snapshot'], before)
                self.assertEqual(list((root / 'data/recovery.sqlite.review-work').iterdir()), [])
            finally:
                if client:
                    client.close()
                host.close()
