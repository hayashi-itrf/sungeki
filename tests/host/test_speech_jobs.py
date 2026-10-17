from support import PROTOCOL_MINOR
import hashlib
import io
import json
import os
import tempfile
import threading
import time
import unittest
import uuid
import wave
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
from pathlib import Path
from unittest.mock import patch
from support import HostProcess, McpClient
from test_mcp_editing import call

ENGINE = '074fc39e-678b-4c13-8916-ffca8d505d1d'

class SpeechJobTest(unittest.TestCase):
    @unittest.skipUnless(os.environ.get('VVM_TEST_SPEECH_ROOT'), 'requires an explicit external-disk VVM_TEST_SPEECH_ROOT')
    def test_mcp_synthesis_replay_stale_cancel_and_recovery(self):
        requests = []
        audio = io.BytesIO()
        with wave.open(audio, 'wb') as writer:
            writer.setnchannels(1)
            writer.setsampwidth(2)
            writer.setframerate(24000)
            writer.writeframes(b'\0' * 4800 + b'\x00\x20' * 2400 + b'\0' * 4800)
        wav = audio.getvalue()
        gate = threading.Event()
        entered = threading.Event()

        class Handler(BaseHTTPRequestHandler):
            def log_message(self, *args): pass
            def reply(self, body):
                self.send_response(200)
                self.send_header('Content-Length', str(len(body)))
                self.end_headers()
                self.wfile.write(body)
            def do_GET(self):
                requests.append(self.path)
                values = {'/engine_manifest': dict(uuid=ENGINE, name='test'), '/version': '1', '/core_versions': ['1'],
                    '/speakers?core_version=1': [dict(speaker_uuid=ENGINE, name='test', version='1', styles=[dict(id=1, name='normal')])]}
                self.reply(json.dumps(values[self.path]).encode())
            def do_POST(self):
                requests.append(self.path)
                self.rfile.read(int(self.headers.get('Content-Length', '0')))
                if self.path.startswith('/audio_query'):
                    body = dict(accent_phrases=[], speedScale=1, pitchScale=0, intonationScale=1, volumeScale=1,
                        prePhonemeLength=0.1, postPhonemeLength=0.1, outputSamplingRate=24000, outputStereo=False)
                    self.reply(json.dumps(body).encode())
                else:
                    entered.set()
                    if not gate.wait(5): raise AssertionError('test synthesis gate timeout')
                    self.reply(wav)

        server = ThreadingHTTPServer(('127.0.0.1', 0), Handler)
        threading.Thread(target=server.serve_forever, daemon=True).start()
        try:
            with tempfile.TemporaryDirectory(prefix='vvm-speech-api-', dir='/tmp') as directory, tempfile.TemporaryDirectory(prefix='speech-api-', dir=os.environ['VVM_TEST_SPEECH_ROOT']) as media, patch.dict(os.environ, SUNGEKI_RUNTIME_ROOT=media):
                root = Path(directory)
                host = HostProcess(root)
                client = None
                try:
                    self.assertEqual(host.ready()['protocol_minor'], PROTOCOL_MINOR)
                    client = McpClient(root)
                    def ok(tool, **args):
                        result = call(client, tool, **args)
                        self.assertFalse(result.get('isError'), result)
                        return result['structuredContent']
                    initial = ok('create_project', title='合成APIの原本保存と再送検証20260922')['snapshot']
                    char, track, clip = [str(uuid.uuid4()) for _ in range(3)]
                    character = dict(id=char, name='試験話者', credit='疑似エンジン・品質未検証', subtitle_style=dict(font_size=64, color='#ffffff'),
                        voice=dict(type='voicevox', engine_id=ENGINE, core_version='1', settings=dict(style_id=1, speed_scale=1, pitch_scale=0, intonation_scale=1, volume_scale=1,
                        pre_phoneme_length=0.1, post_phoneme_length=0.1, pause_length=None, pause_length_scale=1, output_sampling_rate=24000, output_stereo=False, enable_interrogative_upspeak=True)))
                    snapshot = ok('apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        dict(type='set_character', character=character),
                        dict(type='add_track', track=dict(id=track, name='発話', kind='audio', visible=True, muted=False)),
                        dict(type='add_clip', clip=dict(id=clip, track_id=track, scene_id=None, start_us=0, duration_us=1000000,
                            transform=dict(x=0, y=0, width=100, height=100, opacity=1), content=dict(type='speech', audio=None, character_id=char, subtitle_text='字幕', spoken_text=None)))])['snapshot']
                    args = dict(session_id=initial['session_id'], job_id=str(uuid.uuid4()), expected_revision=snapshot['revision'], clip_id=clip, base_url=f'http://127.0.0.1:{server.server_port}', engine_version='1')
                    started = time.monotonic()
                    first = ok('start_speech_job', **args, wait_ms=100)['job']
                    self.assertGreaterEqual(time.monotonic() - started, .09)
                    self.assertIn(first['phase'], ['queued', 'running'])
                    self.assertEqual(first['source_revision'], snapshot['revision'])
                    self.assertTrue(entered.wait(3))
                    started = time.monotonic()
                    pending = ok('get_speech_job', job_id=args['job_id'], wait_ms=100)['job']
                    self.assertGreaterEqual(time.monotonic() - started, .09)
                    self.assertEqual(pending['job_id'], args['job_id'])
                    self.assertEqual(pending['phase'], 'running')
                    self.assertEqual(ok('get_project')['snapshot']['project'], snapshot['project'])
                    self.assertTrue(call(client, 'get_speech_job', job_id=args['job_id'], wait_ms=30001).get('isError'))
                    second_args = dict(args, job_id=str(uuid.uuid4()))
                    ok('start_speech_job', **second_args)
                    cancelled = ok('cancel_speech_job', job_id=second_args['job_id'])['job']
                    self.assertEqual(cancelled['phase'], 'cancelled')
                    self.assertEqual(ok('start_speech_job', **second_args)['job'], cancelled)
                    threading.Timer(.15, gate.set).start()
                    started = time.monotonic()
                    job = ok('start_speech_job', **args, wait_ms=3000)['job']
                    self.assertLess(time.monotonic() - started, 2)
                    started = time.monotonic()
                    self.assertEqual(ok('get_speech_job', job_id=args['job_id'], wait_ms=30000)['job'], job)
                    self.assertLess(time.monotonic() - started, 1)
                    self.assertEqual(job['phase'], 'succeeded', job)
                    self.assertEqual(job['application']['type'], 'applied')
                    artifact = job['artifact']
                    self.assertEqual(Path(artifact['path']).read_bytes(), wav)
                    self.assertEqual(artifact['sha256'], hashlib.sha256(wav).hexdigest())
                    self.assertEqual(artifact['duration_us'], 300000)
                    timing = artifact['lip_sync']
                    self.assertEqual(timing['source_sha256'], artifact['sha256'])
                    self.assertEqual(timing['open'], [dict(start_frame=2400, end_frame=4800)])
                    self.assertEqual(timing['source_frames'], artifact['frames'])
                    self.assertFalse(job['engine_settlement_required'])
                    applied = ok('get_project')['snapshot']
                    self.assertEqual(applied['revision'], snapshot['revision'] + 1)
                    self.assertFalse(job['stale'])
                    self.assertEqual(applied['project']['clips'][0]['duration_us'], artifact['duration_us'])
                    self.assertEqual(applied['project']['clips'][0]['content']['audio']['lip_sync'], timing)
                    audio = applied['project']['clips'][0]['content']['audio']
                    self.assertEqual(applied['project']['assets'][0]['id'], audio['asset_id'])
                    self.assertEqual(applied['project']['assets'][0]['content_hash'], artifact['sha256'])
                    self.assertEqual(job['application']['overlapping_clip_ids'], [])
                    self.assertEqual(ok('start_speech_job', **args)['job'], job)
                    self.assertEqual(ok('cancel_speech_job', job_id=args['job_id'])['job'], job)
                    self.assertEqual(len(requests), 12)
                    self.assertTrue(call(client, 'start_speech_job', **dict(args, engine_version='other'))['isError'])
                    ok('apply_commands', operation_id=str(uuid.uuid4()), commands=[dict(type='set_speech', clip_id=clip, character_id=char, subtitle_text='更新後', spoken_text=None)])
                    self.assertTrue(ok('start_speech_job', **args)['job']['stale'])
                    listed = ok('list_speech_jobs', session_id=args['session_id'], limit=100)['job_ids']
                    self.assertEqual(set(listed), {args['job_id'], second_args['job_id']})
                    self.assertEqual(ok('list_jobs', session_id=args['session_id'])['job_ids'], [])
                    client.close(); client = None
                    host.close(); host = HostProcess(root); host.ready()
                    client = McpClient(root)
                    restored = ok('get_speech_job', job_id=args['job_id'])['job']
                    self.assertEqual(restored['artifact'], artifact)
                    self.assertTrue(restored['stale'])
                    self.assertEqual(len(requests), 12)
                finally:
                    gate.set()
                    if client: client.close()
                    host.close()
        finally:
            gate.set()
            server.shutdown()
            server.server_close()
