from support import PROTOCOL_MINOR
"""Real host/MCP wait semantics with a deliberately slow failure worker, not media QA."""
import concurrent.futures
import hashlib
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from unittest.mock import patch
import uuid

from support import HostProcess, McpClient
from test_render_jobs import call


def document_state(snapshot):
    # Notifications share a host-wide cursor; document state is session-specific.
    return {key: value for key, value in snapshot.items() if key != 'event_cursor'}


class RenderWait(unittest.TestCase):
    def test_wait_timeout_concurrent_cancel_and_terminal_fast_path(self):
        for tool in ("start_render_job", "start_playback_preview"):
            with self.subTest(tool=tool):
                self.check_start_wait(tool)

    def check_start_wait(self, tool):
        with tempfile.TemporaryDirectory(prefix='vvm-rwait-', dir='/tmp') as tmp:
            root = Path(tmp)
            font = root / 'font'
            font.write_bytes(b'wait fixture')
            worker = root / 'worker.py'
            worker.write_text("from pathlib import Path\nimport time\nPath('ready').touch()\ntime.sleep(30)\n")
            env = dict(SUNGEKI_RENDER_PYTHON=sys.executable, SUNGEKI_RENDER_WORKER=str(worker),
                       SUNGEKI_RENDER_FFMPEG='/usr/bin/false', SUNGEKI_RENDER_FFPROBE='/usr/bin/false',
                       SUNGEKI_RENDER_FONT=str(font), SUNGEKI_RENDER_FONT_SHA256=hashlib.sha256(font.read_bytes()).hexdigest(),
                       SUNGEKI_RENDER_WORK_ROOT=str(root / 'render'))
            with patch.dict(os.environ, env):
                host = HostProcess(root)
                client = reader = None
                try:
                    self.assertEqual(host.ready()['protocol_minor'], PROTOCOL_MINOR)
                    client, reader = McpClient(root), McpClient(root)
                    snap = call(client, 'create_project', title='Bounded render wait')['snapshot']
                    snap = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[dict(
                        type='add_scene', scene=dict(id=str(uuid.uuid4()), name='blank', start_us=0, duration_us=100000)
                    )])['snapshot']
                    job_id = str(uuid.uuid4())
                    args = dict(session_id=snap['session_id'], expected_revision=snap['revision'], job_id=job_id)
                    self.assertTrue(call(client, tool, **args, wait_ms=30001).get('isError'))
                    self.assertEqual(call(client, 'list_render_jobs')['job_ids'], [])
                    self.assertFalse((root / 'render' / job_id).exists())
                    started = time.monotonic()
                    initial = call(client, tool, **args, wait_ms=100)['job']
                    self.assertGreaterEqual(time.monotonic() - started, .09)
                    self.assertEqual(initial['job_id'], job_id)
                    self.assertIn(initial['phase'], ['queued', 'running'])
                    deadline = time.monotonic() + 5
                    while not (root / 'render' / job_id / 'ready').exists():
                        self.assertLess(time.monotonic(), deadline)
                        time.sleep(.02)
                    started = time.monotonic()
                    job = call(client, 'get_render_job', job_id=job_id, wait_ms=100)['job']
                    self.assertGreaterEqual(time.monotonic() - started, .09)
                    self.assertEqual((job['job_id'], job['phase']), (job_id, 'running'))
                    self.assertTrue(call(client, 'get_render_job', job_id=job_id, wait_ms=30001).get('isError'))
                    other = call(client, 'create_project', title='Other session during render')['snapshot']
                    call(client, 'attach_session', session_id=snap['session_id'])
                    with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                        pending = pool.submit(call, reader, tool, **args, wait_ms=5000)
                        time.sleep(.1)
                        # A different connection can read the same Project and cancel while waiting.
                        self.assertEqual(call(client, 'get_project')['snapshot']['project'], snap['project'])
                        # Editing must also finish before the waiter does, without cross-session effects.
                        edited = call(client, 'apply_commands', session_id=snap['session_id'],
                                      expected_revision=snap['revision'], operation_id=str(uuid.uuid4()),
                                      commands=[dict(type='rename_project', title='Edited while rendering')])['snapshot']
                        expected_project = dict(snap['project'], title='Edited while rendering')
                        self.assertEqual(edited['project'], expected_project)
                        self.assertEqual(edited['revision'], snap['revision'] + 1)
                        self.assertFalse(pending.done())
                        self.assertEqual(document_state(call(client, 'get_project', session_id=other['session_id'])['snapshot']), document_state(other))
                        self.assertEqual(call(client, 'list_render_jobs', session_id=other['session_id'])['job_ids'], [])
                        self.assertEqual(call(client, 'list_render_jobs', session_id=snap['session_id'])['job_ids'], [job_id])
                        call(client, 'cancel_render_job', job_id=job_id)
                        stopped = pending.result(timeout=3)['job']
                    self.assertEqual(stopped['phase'], 'cancelled')
                    self.assertIsNone(stopped['output'])
                    self.assertEqual(stopped['session_id'], snap['session_id'])
                    self.assertEqual(stopped['source_revision'], snap['revision'])
                    self.assertTrue(stopped['stale'])
                    started = time.monotonic()
                    self.assertEqual(call(client, 'get_render_job', job_id=job_id, wait_ms=30000)['job'], stopped)
                    self.assertEqual(call(client, tool, **args, wait_ms=30000)['job'], stopped)
                    self.assertLess(time.monotonic() - started, 1)
                    self.assertEqual(call(client, 'list_render_jobs')['job_ids'], [job_id])
                    after = call(client, 'get_project')['snapshot']
                    self.assertEqual(document_state(after), document_state(edited))
                    self.assertEqual(document_state(call(client, 'get_project', session_id=other['session_id'])['snapshot']), document_state(other))
                finally:
                    if reader:
                        reader.close()
                    if client:
                        client.close()
                    host.close()
