from support import PROTOCOL_MINOR
import concurrent.futures
import hashlib
import os
from pathlib import Path
import tempfile
import time
import unittest
import uuid
import wave
from unittest.mock import patch
from support import HostProcess,McpClient
from test_render_jobs import call

def finish(c,j):
    deadline=time.monotonic()+8
    while time.monotonic()<deadline:
        row=call(c,'get_loudness_job',job_id=j)['job']
        if row['phase'] in ('succeeded','failed','cancelled','interrupted'):return row
        time.sleep(.02)
    raise AssertionError('measurement did not settle')

def setup(c,root):
    snap=call(c,'create_project',title='音量測定job検証')['snapshot']
    path=root/'original.wav'
    with wave.open(str(path),'wb') as w:
        w.setparams((1,2,48000,0,'NONE','not compressed'));w.writeframes(b'\x00\x00'*48000)
    original=path.read_bytes();sha=hashlib.sha256(original).hexdigest()
    asset=dict(id=str(uuid.uuid4()),locator=dict(type='absolute',path=str(path)),name='fixture',source=None,credit='',content_hash=sha,
        probe=dict(type='audio',info=dict(sha256=sha,duration_us=1000000,sample_rate=48000,channels=1,frames=48000,byte_length=len(original))))
    snap=call(c,'apply_commands',operation_id=str(uuid.uuid4()),commands=[dict(type='register_asset',asset=asset)])['snapshot']
    return snap,dict(session_id=snap['session_id'],expected_revision=snap['revision'],asset_id=asset['id'],job_id=str(uuid.uuid4())),path,original

class LoudnessJobs(unittest.TestCase):
    def test_failed_worker_replay_revision_source_integrity_and_restart(self):
        with tempfile.TemporaryDirectory(prefix='vvm-lj-',dir='/tmp') as tmp:
            root=Path(tmp)
            with patch.dict(os.environ,{'SUNGEKI_RENDER_FFMPEG':'/usr/bin/false','SUNGEKI_RENDER_WORK_ROOT':str(root/'work')}):
                host=HostProcess(root);client=None
                try:
                    self.assertEqual(host.ready()['protocol_minor'],PROTOCOL_MINOR);client=McpClient(root);snapshot,args,path,original=setup(client,root)
                    self.assertTrue(call(client,'start_loudness_job',**args,wait_ms=30001).get('isError'))
                    self.assertEqual(call(client,'list_loudness_jobs')['job_ids'],[])
                    job=call(client,'start_loudness_job',**args,wait_ms=30000)['job']
                    self.assertEqual(job['phase'],'failed');self.assertIsNone(job['output'])
                    self.assertEqual(call(client,'start_loudness_job',**args,wait_ms=0)['job'],job)
                    started=time.monotonic()
                    self.assertEqual(call(client,'get_loudness_job',job_id=args['job_id'],wait_ms=30000)['job'],job)
                    self.assertLess(time.monotonic()-started,1)
                    self.assertEqual(call(client,'get_project')['snapshot'],snapshot)
                    self.assertTrue(call(client,'start_loudness_job',**dict(args,asset_id='other')).get('isError'))
                    self.assertTrue(call(client,'get_render_job',job_id=args['job_id']).get('isError'))
                    call(client,'apply_commands',operation_id=str(uuid.uuid4()),commands=[dict(type='rename_project',title='edited')])
                    self.assertTrue(call(client,'get_loudness_job',job_id=args['job_id'])['job']['stale'])
                    path.write_bytes(original[:-1]+b'\x01')
                    changed=dict(args,job_id=str(uuid.uuid4()),expected_revision=2)
                    call(client,'start_loudness_job',**changed)
                    rejected=finish(client,changed['job_id']);self.assertEqual(rejected['error']['code'],'EXTERNAL_MODIFICATION')
                    path.write_bytes(original);self.assertEqual(list((root/'work').iterdir()),[])
                    client.close();client=None;host.close();host=HostProcess(root);host.ready();client=McpClient(root)
                    self.assertEqual(call(client,'get_loudness_job',job_id=changed['job_id'])['job'],rejected)
                finally:
                    if client:client.close()
                    host.close()

    def test_running_queued_cancel_queue_bound_and_shutdown_cleanup(self):
        with tempfile.TemporaryDirectory(prefix='vvm-lc-',dir='/tmp') as tmp:
            root=Path(tmp);worker=root/'worker.sh';marker=root/'started'
            worker.write_text('#!/bin/sh\necho started > '+str(marker)+'\nsleep 30\n');worker.chmod(0o700)
            with patch.dict(os.environ,{'SUNGEKI_RENDER_FFMPEG':str(worker),'SUNGEKI_RENDER_WORK_ROOT':str(root/'work')}):
                host=HostProcess(root);client=None
                try:
                    self.assertEqual(host.ready()['protocol_minor'],PROTOCOL_MINOR);client=McpClient(root);snapshot,args,path,original=setup(client,root)
                    started=time.monotonic()
                    initial=call(client,'start_loudness_job',**args,wait_ms=100)['job']
                    self.assertGreaterEqual(time.monotonic()-started,.09)
                    self.assertIn(initial['phase'],['queued','running'])
                    deadline=time.monotonic()+5
                    while not marker.exists() and time.monotonic()<deadline:time.sleep(.01)
                    self.assertTrue(marker.exists())
                    queued=[]
                    for _ in range(3):
                        q=dict(args,job_id=str(uuid.uuid4()));queued.append(q)
                        self.assertEqual(call(client,'start_loudness_job',**q)['job']['phase'],'queued')
                    self.assertTrue(call(client,'start_loudness_job',**dict(args,job_id=str(uuid.uuid4()))).get('isError'))
                    for q in queued:call(client,'cancel_loudness_job',job_id=q['job_id'])
                    reader=McpClient(root)
                    try:
                        with concurrent.futures.ThreadPoolExecutor(max_workers=1) as pool:
                            pending=pool.submit(call,reader,'get_loudness_job',job_id=args['job_id'],wait_ms=5000)
                            time.sleep(.1)
                            self.assertEqual(call(client,'get_project')['snapshot']['project'],snapshot['project'])
                            self.assertFalse(pending.done())
                            call(client,'cancel_loudness_job',job_id=args['job_id'])
                            self.assertEqual(pending.result(timeout=3)['job']['phase'],'cancelled')
                    finally:reader.close()
                    for q in [args]+queued:self.assertEqual(finish(client,q['job_id'])['phase'],'cancelled')
                    self.assertEqual(list((root/'work').iterdir()),[])
                    self.assertEqual(path.read_bytes(),original)
                    marker.unlink();again=dict(args,job_id=str(uuid.uuid4()));call(client,'start_loudness_job',**again)
                    deadline=time.monotonic()+5
                    while not marker.exists() and time.monotonic()<deadline:time.sleep(.01)
                    self.assertTrue(marker.exists());client.close();client=None;host.close()
                    self.assertEqual(list((root/'work').iterdir()),[])
                    host=HostProcess(root);host.ready();client=McpClient(root)
                    self.assertEqual(call(client,'get_loudness_job',job_id=again['job_id'])['job']['phase'],'cancelled')
                finally:
                    if client:client.close()
                    host.close()
