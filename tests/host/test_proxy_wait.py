from support import PROTOCOL_MINOR
"""Proxy lifecycle over real MCP; fake probe and bytes are not video quality evidence."""
import concurrent.futures
import hashlib
import json
import os
from pathlib import Path
import tempfile
import time
import unittest
import uuid
from unittest.mock import patch
from support import HostProcess, McpClient
from test_render_jobs import call


class ProxyWait(unittest.TestCase):
    def test_start_get_replay_cancel_and_restart(self):
        for slow in (False, True):
            with self.subTest(slow=slow), tempfile.TemporaryDirectory(prefix='vvm-pw-', dir='/tmp') as tmp:
                root = Path(tmp)
                marker = root / 'started'
                worker = root / 'probe.sh'
                worker.write_text('#!/bin/sh\necho started > "' + str(marker) + '"\n' +
                                  ('sleep 30\n' if slow else 'exit 1\n'))
                worker.chmod(0o700)
                source = root / 'original.mov'
                original = bytes([42]) * 4096
                source.write_bytes(original)
                fixture = Path(__file__).resolve().parents[1] / 'fixtures/video-project.json'
                asset = json.loads(fixture.read_text())['assets'][0]
                asset['locator']['path'] = str(source)
                asset['content_hash'] = asset['probe']['info']['sha256'] = hashlib.sha256(original).hexdigest()
                with patch.dict(os.environ, {'SUNGEKI_RENDER_FFMPEG': '/usr/bin/false',
                                             'SUNGEKI_RENDER_FFPROBE': str(worker),
                                             'SUNGEKI_RENDER_WORK_ROOT': str(root / 'work')}):
                    host, client = HostProcess(root), None
                    try:
                        self.assertEqual(host.ready()['protocol_minor'], PROTOCOL_MINOR)
                        client = McpClient(root)
                        call(client, 'create_project', title='軽量動画の待機・取消検証')
                        snap = call(client, 'apply_commands', operation_id=str(uuid.uuid4()),
                                    commands=[dict(type='register_asset', asset=asset)])['snapshot']
                        args = dict(job_id=str(uuid.uuid4()), session_id=snap['session_id'],
                                    expected_revision=snap['revision'], asset_ids=[asset['id']], preset='compact')
                        self.assertTrue(call(client, 'generate_proxies', **args, wait_ms=30001).get('isError'))
                        self.assertEqual(call(client, 'list_proxy_jobs')['job_ids'], [])
                        started = time.monotonic()
                        job = call(client, 'generate_proxies', **args, wait_ms=100 if slow else 30000)['job']
                        if slow:
                            self.assertGreaterEqual(time.monotonic() - started, .09)
                            self.assertIn(job['phase'], ('queued', 'running'))
                            reader = McpClient(root)
                            try:
                                with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                                    pending = pool.submit(call, reader, 'get_proxy_job', job_id=args['job_id'], wait_ms=5000)
                                    time.sleep(.1)
                                    self.assertEqual(call(client, 'get_project')['snapshot'], snap)
                                    self.assertFalse(pending.done())
                                    call(client, 'cancel_proxy_job', job_id=args['job_id'])
                                    job = pending.result(timeout=3)['job']
                                    self.assertEqual(job['phase'], 'cancelled')
                            finally:
                                reader.close()
                        else:
                            self.assertEqual(job['phase'], 'failed')
                        self.assertEqual(job['completed'], [])
                        self.assertEqual(call(client, 'generate_proxies', **args, wait_ms=0)['job'], job)
                        started = time.monotonic()
                        self.assertEqual(call(client, 'get_proxy_job', job_id=args['job_id'], wait_ms=30000)['job'], job)
                        self.assertLess(time.monotonic() - started, 1)
                        self.assertEqual(call(client, 'list_proxy_jobs')['job_ids'], [args['job_id']])
                        self.assertEqual(call(client, 'get_project')['snapshot'], snap)
                        self.assertEqual(source.read_bytes(), original)
                        self.assertEqual(list((root / 'work' / 'proxy-cache').iterdir()), [])
                        client.close()
                        client = None
                        host.close()
                        host = HostProcess(root)
                        host.ready()
                        client = McpClient(root)
                        self.assertEqual(call(client, 'get_proxy_job', job_id=args['job_id'])['job'], job)
                    finally:
                        if client:
                            client.close()
                        host.close()
