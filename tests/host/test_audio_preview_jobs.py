from support import PROTOCOL_MINOR
import concurrent.futures
import hashlib
import os
from pathlib import Path
import tempfile
import time
import unittest
from unittest.mock import patch
import uuid

from support import HostProcess, McpClient


def call(client, name, **args):
    response = client.call('tools/call', dict(name=name, arguments=args))['result']
    if response.get('isError'):
        return response
    return response['structuredContent']


def finish(client, job_id):
    deadline = time.monotonic() + 8
    while time.monotonic() < deadline:
        job = call(client, 'get_audio_preview_job', job_id=job_id)['job']
        if job['phase'] in ('succeeded', 'failed', 'cancelled', 'interrupted'):
            return job
        time.sleep(.02)
    raise AssertionError('render job not settled')


class AudioPreviewJobs(unittest.TestCase):
    def test_failed_output_is_durable_idempotent_and_not_a_review_job(self):
        with tempfile.TemporaryDirectory(prefix='vvm-rj-', dir='/tmp') as tmp:
            root = Path(tmp)
            font = root / 'font'
            font.write_bytes(b'font for failure fixture')
            env = dict(SUNGEKI_RENDER_PYTHON='/usr/bin/false', SUNGEKI_RENDER_WORKER='/usr/bin/false',
                       SUNGEKI_RENDER_FFMPEG='/usr/bin/false', SUNGEKI_RENDER_FFPROBE='/usr/bin/false',
                       SUNGEKI_RENDER_FONT=str(font), SUNGEKI_RENDER_FONT_SHA256=hashlib.sha256(font.read_bytes()).hexdigest(),
                       SUNGEKI_RENDER_WORK_ROOT=str(root / 'render'))
            with patch.dict(os.environ, env):
                host = HostProcess(root)
                client = None
                try:
                    self.assertEqual(host.ready()['protocol_minor'], PROTOCOL_MINOR)
                    client = McpClient(root)
                    snap = call(client, 'create_project', title='MCP出力失敗と再送検証')['snapshot']
                    scene = dict(id=str(uuid.uuid4()), name='blank', start_us=0, duration_us=100000)
                    snap = call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                                commands=[dict(type='add_scene', scene=scene)])['snapshot']
                    request = dict(session_id=snap['session_id'], expected_revision=snap['revision'], job_id=str(uuid.uuid4()))
                    tools = client.call('tools/list')['result']['tools']
                    schema = next(t['inputSchema'] for t in tools if t['name'] == 'start_audio_preview_job')
                    self.assertIn('wait_ms', schema['properties'])
                    self.assertTrue(call(client, 'start_audio_preview_job', **request, wait_ms=30001).get('isError'))
                    self.assertEqual(call(client, 'list_audio_preview_jobs')['job_ids'], [])
                    initial = call(client, 'start_audio_preview_job', **request, wait_ms=30000)['job']
                    self.assertEqual(initial['phase'], 'failed')
                    job = finish(client, request['job_id'])
                    self.assertEqual(job['phase'], 'failed')
                    self.assertIsNone(job['output'])
                    rejected = call(client, 'read_audio_preview', job_id=request['job_id'], offset=0, length=1)
                    self.assertTrue(rejected.get('isError'))
                    self.assertIn('外部描画処理', job['error']['message'])
                    self.assertEqual(call(client, 'start_audio_preview_job', **request, wait_ms=0)['job'], job)
                    started = time.monotonic()
                    self.assertEqual(call(client, 'get_audio_preview_job', job_id=request['job_id'], wait_ms=30000)['job'], job)
                    self.assertLess(time.monotonic()-started, 1)
                    self.assertEqual(call(client, 'cancel_audio_preview_job', job_id=request['job_id'])['job'], job)
                    self.assertTrue(call(client, 'get_job', job_id=request['job_id']).get('isError'))
                    self.assertEqual(call(client, 'list_audio_preview_jobs')['job_ids'], [request['job_id']])
                    call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                         commands=[dict(type='rename_project', title='編集後')])
                    self.assertTrue(call(client, 'get_audio_preview_job', job_id=request['job_id'])['job']['stale'])
                    self.assertTrue(call(client, 'start_audio_preview_job', **dict(request, expected_revision=2)).get('isError'))
                    # Cancellation never publishes a successful artifact, whether queued or running.
                    cancelled = dict(request, job_id=str(uuid.uuid4()), expected_revision=2)
                    call(client, 'start_audio_preview_job', **cancelled)
                    call(client, 'cancel_audio_preview_job', job_id=cancelled['job_id'])
                    stopped = finish(client, cancelled['job_id'])
                    self.assertIn(stopped['phase'], ('cancelled', 'failed'))
                    self.assertIsNone(stopped['output'])
                    client.close(); client = None
                    host.close(); host = HostProcess(root)
                    host.ready(); client = McpClient(root)
                    restored = call(client, 'get_audio_preview_job', job_id=request['job_id'])['job']
                    self.assertEqual(restored['phase'], 'failed')
                    self.assertTrue(restored['stale'])
                    self.assertIsNone(restored['output'])
                finally:
                    if client:
                        client.close()
                    host.close()

    def test_running_cancel_and_host_shutdown_wait_for_worker_cleanup(self):
        import sys
        with tempfile.TemporaryDirectory(prefix='vvm-rc-', dir='/tmp') as tmp:
            root = Path(tmp)
            font = root / 'font'
            font.write_bytes(b'font')
            worker = root / 'worker.sh'
            worker.write_text("#!/bin/sh\ntouch worker-started\nsleep 30\n")
            worker.chmod(0o700)
            env = dict(SUNGEKI_RENDER_PYTHON=sys.executable, SUNGEKI_RENDER_WORKER=str(worker),
                       SUNGEKI_RENDER_FFMPEG=str(worker), SUNGEKI_RENDER_FFPROBE='/usr/bin/false',
                       SUNGEKI_RENDER_FONT=str(font), SUNGEKI_RENDER_FONT_SHA256=hashlib.sha256(b'font').hexdigest(),
                       SUNGEKI_RENDER_WORK_ROOT=str(root / 'render'))
            with patch.dict(os.environ, env):
                host = HostProcess(root)
                client = None
                try:
                    host.ready(); client = McpClient(root)
                    snap = call(client, 'create_project', title='稼働中の出力停止検証')['snapshot']
                    snap = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[dict(type='add_scene',
                        scene=dict(id=str(uuid.uuid4()), name='blank', start_us=0, duration_us=100000))])['snapshot']
                    for shutdown in [False, True]:
                        job_id = str(uuid.uuid4())
                        started = time.monotonic()
                        initial = call(client, 'start_audio_preview_job', job_id=job_id, session_id=snap['session_id'], expected_revision=1, wait_ms=100)['job']
                        self.assertGreaterEqual(time.monotonic()-started, .09)
                        self.assertIn(initial['phase'], ['queued', 'running'])
                        marker = root / 'render' / job_id / 'worker-started'
                        deadline = time.monotonic() + 5
                        while not marker.exists() and time.monotonic() < deadline:
                            time.sleep(.02)
                        self.assertTrue(marker.exists())
                        if shutdown:
                            client.close(); client = None
                            host.close(); host = HostProcess(root)
                            host.ready(); client = McpClient(root)
                        else:
                            reader = McpClient(root)
                            try:
                                with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                                    pending = pool.submit(call, reader, 'get_audio_preview_job', job_id=job_id, wait_ms=5000)
                                    time.sleep(.1)
                                    self.assertEqual(call(client, 'get_project')['snapshot']['project'], snap['project'])
                                    self.assertFalse(pending.done())
                                    call(client, 'cancel_audio_preview_job', job_id=job_id)
                                    self.assertEqual(pending.result(timeout=3)['job']['phase'], 'cancelled')
                            finally:
                                reader.close()
                        job = finish(client, job_id)
                        self.assertEqual(job['phase'], 'cancelled')
                        self.assertIsNone(job['output'])
                        self.assertFalse((marker.parent / 'output.mp4').exists())
                finally:
                    if client:
                        client.close()
                    host.close()
