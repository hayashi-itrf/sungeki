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
        job = call(client, 'get_render_job', job_id=job_id)['job']
        if job['phase'] in ('succeeded', 'failed', 'cancelled', 'interrupted'):
            return job
        time.sleep(.02)
    raise AssertionError('render job not settled')


def wait_reclaimed(directory):
    deadline = time.monotonic() + 5
    while time.monotonic() < deadline:
        if not (directory / 'font.ttc').exists() and not (directory / 'frame-999.png').exists():
            return
        time.sleep(.02)
    raise AssertionError('terminal render intermediates were not reclaimed')


class RenderJobs(unittest.TestCase):
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
                    self.assertGreaterEqual(host.ready()['protocol_minor'], 44)
                    client = McpClient(root)
                    snap = call(client, 'create_project', title='MCP出力失敗と再送検証')['snapshot']
                    scene = dict(id=str(uuid.uuid4()), name='blank', start_us=0, duration_us=100000)
                    snap = call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                                commands=[dict(type='add_scene', scene=scene)])['snapshot']
                    request = dict(session_id=snap['session_id'], expected_revision=snap['revision'], job_id=str(uuid.uuid4()))
                    call(client, 'start_render_job', **request)
                    job = finish(client, request['job_id'])
                    self.assertEqual(job['phase'], 'failed')
                    self.assertIsNone(job['output'])
                    rejected = call(client, 'publish_render_output', job_id=request['job_id'], path=str(root / 'forbidden.mp4'))
                    self.assertTrue(rejected.get('isError'))
                    self.assertFalse((root / 'forbidden.mp4').exists())
                    self.assertIn('外部描画処理', job['error']['message'])
                    wait_reclaimed(root / 'render' / request['job_id'])
                    self.assertEqual(font.read_bytes(), b'font for failure fixture')
                    self.assertEqual(call(client, 'start_render_job', **request)['job'], job)
                    self.assertEqual(call(client, 'cancel_render_job', job_id=request['job_id'])['job'], job)
                    self.assertTrue(call(client, 'get_job', job_id=request['job_id']).get('isError'))
                    self.assertEqual(call(client, 'list_render_jobs')['job_ids'], [request['job_id']])
                    call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                         commands=[dict(type='rename_project', title='編集後')])
                    self.assertTrue(call(client, 'get_render_job', job_id=request['job_id'])['job']['stale'])
                    self.assertTrue(call(client, 'start_render_job', **dict(request, expected_revision=2)).get('isError'))
                    # Cancellation never publishes a successful artifact, whether queued or running.
                    cancelled = dict(request, job_id=str(uuid.uuid4()), expected_revision=2)
                    call(client, 'start_render_job', **cancelled)
                    call(client, 'cancel_render_job', job_id=cancelled['job_id'])
                    stopped = finish(client, cancelled['job_id'])
                    self.assertIn(stopped['phase'], ('cancelled', 'failed'))
                    self.assertIsNone(stopped['output'])
                    client.close(); client = None
                    host.close(); host = HostProcess(root)
                    host.ready(); client = McpClient(root)
                    restored = call(client, 'get_render_job', job_id=request['job_id'])['job']
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
            worker = root / 'worker.py'
            worker.write_text("#!" + sys.executable + "\nfrom pathlib import Path\nimport time, os\nPath('frame-999.png').write_bytes(b'partial')\nPath('worker-started').write_text(str(os.getpid()))\ntime.sleep(30)\n")
            worker.chmod(0o700)
            env = dict(SUNGEKI_RENDER_FFMPEG=str(worker), SUNGEKI_RENDER_FFPROBE='/usr/bin/false',
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
                        call(client, 'start_render_job', job_id=job_id, session_id=snap['session_id'], expected_revision=1)
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
                            call(client, 'cancel_render_job', job_id=job_id)
                        job = finish(client, job_id)
                        self.assertEqual(job['phase'], 'cancelled')
                        self.assertEqual(job['error']['code'], 'CANCELLED')
                        self.assertEqual(job['error']['message'], '動画出力を取り消しました')
                        self.assertIsNone(job['output'])
                        self.assertFalse((marker.parent / 'output.mp4').exists())
                        wait_reclaimed(marker.parent)
                        self.assertEqual(font.read_bytes(), b'font')
                        self.assertTrue(marker.exists())  # Unknown diagnostic record is retained.
                        with self.assertRaises(ProcessLookupError):
                            os.kill(int(marker.read_text()), 0)
                finally:
                    if client:
                        client.close()
                    host.close()

    def test_unclassified_spawn_error_retains_work_after_shutdown_and_reopen(self):
        with tempfile.TemporaryDirectory(prefix='vvm-render-spawn-', dir='/tmp') as tmp:
            root = Path(tmp)
            font = root / 'font'
            font.write_bytes(b'font retained for diagnosis')
            executable = root / 'invalid-executable'
            executable.write_text('#!' + str(root / 'missing-interpreter') + '\n')
            executable.chmod(0o700)
            env = dict(
                SUNGEKI_RENDER_FFMPEG=str(executable),
                SUNGEKI_RENDER_FFPROBE='/usr/bin/false',
                SUNGEKI_RENDER_FONT=str(font),
                SUNGEKI_RENDER_FONT_SHA256=hashlib.sha256(font.read_bytes()).hexdigest(),
                SUNGEKI_RENDER_WORK_ROOT=str(root / 'render'),
            )
            with patch.dict(os.environ, env):
                host = HostProcess(root)
                client = None
                try:
                    host.ready()
                    client = McpClient(root)
                    snap = call(client, 'create_project', title='起動失敗時の作業記録保持')['snapshot']
                    snap = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                        dict(type='add_scene', scene=dict(id=str(uuid.uuid4()), name='blank',
                                                        start_us=0, duration_us=100000))])['snapshot']
                    job_id = str(uuid.uuid4())
                    call(client, 'start_render_job', job_id=job_id,
                         session_id=snap['session_id'], expected_revision=snap['revision'])
                    job = finish(client, job_id)
                    self.assertEqual(job['phase'], 'failed')
                    self.assertIsNone(job['output'])
                    self.assertIsNotNone(job['error'])
                    work = root / 'render' / job_id
                    client.close(); client = None
                    host.close()  # Wait for cleanup tasks; terminal alone may precede cleanup.
                    self.assertEqual((work / 'font.ttc').read_bytes(), font.read_bytes())
                    self.assertTrue((work / 'resources.json').exists())
                    self.assertFalse((work / 'output.mp4').exists())
                    host = HostProcess(root)
                    host.ready(); client = McpClient(root)
                    self.assertEqual(call(client, 'get_render_job', job_id=job_id)['job'], job)
                    self.assertEqual((work / 'font.ttc').read_bytes(), font.read_bytes())
                finally:
                    if client:
                        client.close()
                    host.close()
