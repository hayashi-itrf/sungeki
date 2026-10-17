"""Actual host + stdio MCP with a dummy Irodori child; not real-engine evidence."""
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
import uuid
from unittest.mock import patch

from irodori_fixture import configuration
from support import HostProcess, McpClient
from test_mcp_editing import call


def uid():
    return str(uuid.uuid4())


class IrodoriSpeechTest(unittest.TestCase):
    @unittest.skipUnless(os.environ.get('VVM_TEST_SPEECH_ROOT'), 'requires an explicit external-disk VVM_TEST_SPEECH_ROOT')
    def test_mcp_irodori_apply_save_reopen_and_replay(self):
        with (tempfile.TemporaryDirectory(prefix='irodori-mcp-', dir='/tmp') as directory,
              tempfile.TemporaryDirectory(prefix='irodori-audio-', dir=os.environ['VVM_TEST_SPEECH_ROOT']) as media,
              patch.dict(os.environ, SUNGEKI_RUNTIME_ROOT=media)):
            root = Path(directory).resolve()
            config = configuration(root / 'fixture')
            data = root / 'data'
            data.mkdir(mode=0o700)
            (data / 'irodori.json').write_text(json.dumps(config))
            host = HostProcess(root, data_dir=data)
            client = None
            try:
                self.assertGreaterEqual(host.ready()['protocol_minor'], 105)
                client = McpClient(root, data_dir=data)

                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                details = ok('get_irodori_profile', profile_id='fixture')['details']
                snap = ok('create_project', title='Irodori新MCP保存再読込検証')['snapshot']
                char, track, clip = uid(), uid(), uid()
                character = dict(id=char, name='模擬Irodori', credit='Irodori test fixture',
                    subtitle_style=dict(font_size=54, color='#205b30'),
                    voice=dict(type='irodori', identity=details['identity'], settings=details['default_settings']))
                snap = ok('apply_commands', operation_id=uid(), commands=[
                    dict(type='set_character', character=character),
                    dict(type='add_track', track=dict(id=track, name='音声', kind='audio', visible=True, muted=False)),
                    dict(type='add_clip', clip=dict(id=clip, track_id=track, scene_id=None, start_us=0, duration_us=1000000,
                        transform=dict(x=120, y=780, width=1680, height=200, opacity=1),
                        content=dict(type='speech', character_id=char, subtitle_text='音声の確認です。', spoken_text=None, audio=None)))
                ])['snapshot']
                args = dict(session_id=snap['session_id'], job_id=uid(), clip_id=clip,
                            expected_revision=snap['revision'], profile_id='fixture')
                ok('start_irodori_speech_job', **args)
                deadline = time.monotonic() + 15
                while True:
                    job = ok('get_speech_job', job_id=args['job_id'])['job']
                    if job['phase'] in ('succeeded', 'failed', 'cancelled', 'interrupted'):
                        break
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.05)
                self.assertEqual(job['phase'], 'succeeded', job)
                self.assertEqual(job['application']['type'], 'applied')
                self.assertFalse(job['engine_settlement_required'])
                self.assertFalse(job['stale'])
                self.assertEqual(job['progress'], {'stage': 'speech.save_audio', 'fraction': None})
                with self.assertRaises(ProcessLookupError):
                    os.kill(int((root / 'fixture/pid').read_text()), 0)
                artifact = job['artifact']
                self.assertEqual(hashlib.sha256(Path(artifact['path']).read_bytes()).hexdigest(), artifact['sha256'])
                snap = ok('get_project')['snapshot']
                speech = snap['project']['clips'][0]
                self.assertEqual(speech['duration_us'], 100000)
                self.assertIsNotNone(speech['content']['audio']['irodori_source'])
                saved = root / 'irodori-mcp-roundtrip.vvm.json'
                ok('save_project', path=str(saved), expected_revision=snap['revision'])
                expected = json.loads(saved.read_text())
                client.close(); client = None
                host.close(); host = HostProcess(root, data_dir=data)
                host.ready()
                client = McpClient(root, data_dir=data)
                restored = ok('open_project', path=str(saved))['snapshot']
                self.assertEqual(restored['project'], expected)
                self.assertFalse(restored['dirty'])
                replayed = ok('start_irodori_speech_job', **args)['job']
                self.assertEqual(replayed['artifact'], artifact)
                self.assertEqual(replayed['progress'], job['progress'])
                requests = (root / 'fixture/requests.jsonl').read_text().splitlines()
                self.assertEqual(len(requests), 1)
                request = json.loads(requests[0])
                self.assertEqual(request['text'], '音声の確認です。')
                self.assertEqual(request['settings']['seed'], 42)
                self.assertEqual(request['settings']['voice_id'], 'none')
                before_cancel = ok('get_project')['snapshot']
                (root / 'fixture/hold-voice').touch()
                cancel_args = dict(args, job_id=uid(), expected_revision=restored['revision'])
                ok('start_irodori_speech_job', **cancel_args)
                deadline = time.monotonic() + 15
                while len((root / 'fixture/requests.jsonl').read_text().splitlines()) < 2:
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.01)
                while True:
                    running = ok('get_speech_job', job_id=cancel_args['job_id'])['job']
                    if running.get('progress', {}).get('stage') == 'speech.synthesize':
                        break
                    self.assertEqual(running['phase'], 'running')
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.02)
                self.assertEqual(running['phase'], 'running')
                self.assertEqual(running['progress'], {'stage': 'speech.synthesize', 'fraction': None})
                ok('cancel_speech_job', job_id=cancel_args['job_id'])
                while True:
                    cancelled = ok('get_speech_job', job_id=cancel_args['job_id'])['job']
                    if cancelled['phase'] in ('cancelled', 'failed', 'interrupted', 'succeeded'):
                        break
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.02)
                self.assertEqual(cancelled['phase'], 'cancelled', cancelled)
                self.assertFalse(cancelled['engine_settlement_required'])
                self.assertIsNone(cancelled['artifact'])
                self.assertEqual(cancelled['progress'], running['progress'])
                with self.assertRaises(ProcessLookupError):
                    os.kill(int((root / 'fixture/pid').read_text()), 0)
                self.assertEqual(ok('get_project')['snapshot'], before_cancel)
                self.assertEqual(hashlib.sha256(Path(artifact['path']).read_bytes()).hexdigest(), artifact['sha256'])
                self.assertEqual(json.loads(saved.read_text()), expected)
                self.assertEqual(ok('start_irodori_speech_job', **cancel_args)['job'], cancelled)
                self.assertEqual(len((root / 'fixture/requests.jsonl').read_text().splitlines()), 2)
                (root / 'fixture/hold-voice').unlink()

                # A malformed engine response must settle ownership without
                # replacing the previously usable synthesized audio.
                (root / 'fixture/invalid-audio').touch()
                before_failure = ok('get_project')['snapshot']
                failed_args = dict(args, job_id=uid(), expected_revision=before_failure['revision'])
                ok('start_irodori_speech_job', **failed_args)
                deadline = time.monotonic() + 15
                while True:
                    failed = ok('get_speech_job', job_id=failed_args['job_id'])['job']
                    if failed['phase'] in ('cancelled', 'failed', 'interrupted', 'succeeded'):
                        break
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.02)
                self.assertEqual(failed['phase'], 'failed', failed)
                self.assertFalse(failed['engine_settlement_required'])
                self.assertIsNone(failed['artifact'])
                self.assertIsNotNone(failed['error'])
                with self.assertRaises(ProcessLookupError):
                    os.kill(int((root / 'fixture/pid').read_text()), 0)
                self.assertEqual(ok('get_project')['snapshot'], before_failure)
                self.assertEqual(hashlib.sha256(Path(artifact['path']).read_bytes()).hexdigest(), artifact['sha256'])
                self.assertEqual(json.loads(saved.read_text()), expected)
                request_count = len((root / 'fixture/requests.jsonl').read_text().splitlines())
                client.close(); client = None
                host.close(); host = HostProcess(root, data_dir=data)
                host.ready(); client = McpClient(root, data_dir=data)
                recovered = ok('attach_session', session_id=snap['session_id'])['snapshot']
                for key in ('project', 'revision', 'dirty', 'undo_operation_id', 'redo_operation_id'):
                    self.assertEqual(recovered[key], before_failure[key])
                self.assertEqual(ok('start_irodori_speech_job', **failed_args)['job'], failed)
                self.assertEqual(ok('start_irodori_speech_job', **cancel_args)['job'], cancelled)
                self.assertEqual(len((root / 'fixture/requests.jsonl').read_text().splitlines()), request_count)
                # A fresh request can reuse the released engine lease.
                (root / 'fixture/invalid-audio').unlink()
                retry_args = dict(failed_args, job_id=uid())
                ok('start_irodori_speech_job', **retry_args)
                deadline = time.monotonic() + 15
                while True:
                    retry = ok('get_speech_job', job_id=retry_args['job_id'])['job']
                    if retry['phase'] in ('cancelled', 'failed', 'interrupted', 'succeeded'):
                        break
                    self.assertLess(time.monotonic(), deadline)
                    time.sleep(.02)
                self.assertEqual(retry['phase'], 'succeeded', retry)
                self.assertEqual(retry['application']['type'], 'applied')
                self.assertFalse(retry['engine_settlement_required'])
                with self.assertRaises(ProcessLookupError):
                    os.kill(int((root / 'fixture/pid').read_text()), 0)
                retried_project = ok('get_project')['snapshot']['project']
                old_assets = {a['id']: a for a in expected['assets']}
                self.assertEqual({a['id']: a for a in retried_project['assets'] if a['id'] in old_assets}, old_assets)
                self.assertEqual(len(retried_project['assets']), len(old_assets) + 1)
                new_asset = next(a for a in retried_project['assets'] if a['id'] not in old_assets)
                self.assertEqual(retried_project['clips'][0]['content']['audio']['asset_id'], new_asset['id'])
                self.assertEqual(retry['artifact']['sha256'], artifact['sha256'])
                # Successful re-synthesis intentionally registers a new immutable
                # audio asset. All other project fields and clip values persist.
                retried_project['assets'] = expected['assets']
                retried_project['clips'][0]['content']['audio']['asset_id'] = expected['clips'][0]['content']['audio']['asset_id']
                self.assertEqual(retried_project, expected)

            finally:
                if client:
                    client.close()
                host.close()


if __name__ == '__main__':
    unittest.main()
