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
        job = call(client, 'get_frame_preview_job', job_id=job_id)['job']
        if job['phase'] in ('succeeded', 'failed', 'cancelled', 'interrupted'):
            return job
        time.sleep(.02)
    raise AssertionError('render job not settled')


class FramePreviewJobs(unittest.TestCase):
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
                    self.assertGreaterEqual(host.ready()['protocol_minor'], 72)
                    client = McpClient(root)
                    snap = call(client, 'create_project', title='MCP出力失敗と再送検証')['snapshot']
                    scene = dict(id=str(uuid.uuid4()), name='blank', start_us=0, duration_us=100000)
                    track = str(uuid.uuid4())
                    text = dict(id=str(uuid.uuid4()), track_id=track, scene_id=None, start_us=0, duration_us=100000,
                        transform=dict(x=0,y=0,width=100,height=100,opacity=1),
                        content=dict(type='text',text='font validation',font_size=20,color='#000000'))
                    snap = call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                                commands=[dict(type='add_scene', scene=scene), dict(type='add_track', track=dict(id=track,name='text',kind='visual',visible=True,muted=False)),dict(type='add_clip',clip=text)])['snapshot']
                    request = dict(session_id=snap['session_id'], expected_revision=snap['revision'], job_id=str(uuid.uuid4()), frame_index=0)
                    call(client, 'preview_frame', **request)
                    job = finish(client, request['job_id'])
                    self.assertEqual(job['phase'], 'failed')
                    self.assertIsNone(job['output'])
                    self.assertTrue(call(client, 'read_frame_preview', job_id=request['job_id'], offset=0, length=1).get('isError'))
                    self.assertTrue(call(client, 'publish_frame_preview', job_id=request['job_id'], path=str(root / 'failed.png')).get('isError'))
                    self.assertFalse((root / 'failed.png').exists())
                    self.assertIn('字幕フォント', job['error']['message'])
                    self.assertEqual(call(client, 'preview_frame', **request)['job'], job)
                    self.assertTrue(call(client, 'preview_frame', **dict(request, frame_index=1)).get('isError'))
                    rejected = call(client, 'preview_frame', **dict(request, job_id=str(uuid.uuid4()), frame_index=3))
                    self.assertEqual(rejected['structuredContent']['code'], 'VALIDATION_FAILED')
                    self.assertEqual(call(client, 'cancel_frame_preview_job', job_id=request['job_id'])['job'], job)
                    self.assertTrue(call(client, 'get_job', job_id=request['job_id']).get('isError'))
                    self.assertEqual(call(client, 'list_frame_preview_jobs')['job_ids'], [request['job_id']])
                    call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                         commands=[dict(type='rename_project', title='編集後')])
                    self.assertTrue(call(client, 'get_frame_preview_job', job_id=request['job_id'])['job']['stale'])
                    self.assertTrue(call(client, 'preview_frame', **dict(request, expected_revision=2)).get('isError'))
                    # Cancellation never publishes a successful artifact, whether queued or running.
                    cancelled = dict(request, job_id=str(uuid.uuid4()), expected_revision=2)
                    call(client, 'preview_frame', **cancelled)
                    call(client, 'cancel_frame_preview_job', job_id=cancelled['job_id'])
                    stopped = finish(client, cancelled['job_id'])
                    self.assertIn(stopped['phase'], ('cancelled', 'failed'))
                    self.assertIsNone(stopped['output'])
                    client.close(); client = None
                    host.close(); host = HostProcess(root)
                    host.ready(); client = McpClient(root)
                    restored = call(client, 'get_frame_preview_job', job_id=request['job_id'])['job']
                    self.assertEqual(restored['phase'], 'failed')
                    self.assertTrue(restored['stale'])
                    self.assertIsNone(restored['output'])
                finally:
                    if client:
                        client.close()
                    host.close()

    def test_running_native_cancel_and_host_shutdown_wait_for_completion(self):
        import sys
        with tempfile.TemporaryDirectory(prefix='vvm-rc-', dir='/tmp') as tmp:
            root = Path(tmp)
            font = root / 'font'
            font.write_bytes(b'font')
            env = dict(SUNGEKI_RENDER_FFMPEG='/usr/bin/false', SUNGEKI_RENDER_FFPROBE='/usr/bin/false',
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
                    commands=[]
                    for index in range(24):
                        track=str(uuid.uuid4())
                        commands.extend([dict(type='add_track',track=dict(id=track,name='shape',kind='visual',visible=True,muted=False)),
                            dict(type='add_clip',clip=dict(id=str(uuid.uuid4()),track_id=track,scene_id=None,start_us=0,duration_us=100000,
                                transform=dict(x=0,y=0,width=1920,height=1080,opacity=.5),content=dict(type='shape',shape='rectangle',fill='#123456')))])
                    snap=call(client,'apply_commands',operation_id=str(uuid.uuid4()),commands=commands)['snapshot']
                    for shutdown in [False, True]:
                        job_id = str(uuid.uuid4())
                        call(client, 'preview_frame', job_id=job_id, session_id=snap['session_id'], expected_revision=snap['revision'], frame_index=0)
                        deadline = time.monotonic() + 5
                        while call(client,'get_frame_preview_job',job_id=job_id)['job']['phase']=='queued':
                            self.assertLess(time.monotonic(),deadline)
                            time.sleep(.01)
                        self.assertEqual(call(client,'get_frame_preview_job',job_id=job_id)['job']['phase'],'running')
                        if shutdown:
                            client.close(); client = None
                            host.close(); host = HostProcess(root)
                            host.ready(); client = McpClient(root)
                        else:
                            call(client, 'cancel_frame_preview_job', job_id=job_id)
                        job = finish(client, job_id)
                        self.assertEqual(job['phase'], 'cancelled')
                        self.assertIsNone(job['output'])
                        self.assertFalse((root / 'render' / job_id / 'output.mp4').exists())
                finally:
                    if client:
                        client.close()
                    host.close()
