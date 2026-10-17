import json
import math
import os
import platform
import statistics
import tempfile
import time
import unittest
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_checkpoints import uid


class LargeDocumentTest(unittest.TestCase):
    def test_5000_clips_edit_save_recover_and_partial_read(self):
        with tempfile.TemporaryDirectory(prefix='vvm-large-', dir='/tmp') as directory:
            root = Path(directory); host = HostProcess(root); client = None
            try:
                info = host.ready(); client = McpClient(root)
                def invoke(name, **args):
                    result = call(client, name, **args)
                    self.assertFalse(result.get('isError'), result.get('structuredContent', {}).get('message'))
                    return result['structuredContent']
                initial = invoke('create_project', title='5000 clips / 50 tracks transfer validation')['snapshot']
                sid = initial['session_id']; tracks = [uid() for _ in range(50)]
                clips = [uid() for _ in range(5000)]
                commands = [{'type': 'add_track', 'track': {'id': tid, 'name': f'Track {i}',
                    'kind': 'visual', 'visible': True, 'muted': False}} for i, tid in enumerate(tracks)]
                commands += [{'type': 'add_clip', 'clip': {'id': cid, 'track_id': tracks[i % 50],
                    'scene_id': None, 'start_us': (i // 50) * 1000000, 'duration_us': 1000000,
                    'transform': {'x': 0, 'y': 0, 'width': 100, 'height': 100, 'opacity': 1},
                    'content': {'type': 'text', 'text': f'Large fixture clip {i}', 'font_size': 32, 'color': '#000000'}}}
                    for i, cid in enumerate(clips)]
                current = invoke('apply_commands', operation_id=uid(), commands=commands)['snapshot']
                self.assertEqual(len(current['project']['clips']), 5000)
                encoded_bytes = len(json.dumps(current['project'], separators=(',', ':')).encode())
                self.assertGreater(encoded_bytes, 65536)
                durations = []
                for index in range(20):
                    started = time.perf_counter()
                    current = invoke('apply_commands', operation_id=uid(), commands=[{
                        'type': 'set_text', 'clip_id': clips[0], 'text': f'Edit {index}'}])['snapshot']
                    durations.append((time.perf_counter() - started) * 1000)
                final_project = current['project']
                page = invoke('get_project', view='timeline', clip_ids=[clips[-1]])['result']
                self.assertEqual(page['data']['entries'][0]['clip_order'], 4999)
                self.assertEqual(page['data']['entries'][0]['clip']['id'], clips[-1])
                undo = invoke('undo', operation_id=uid(), expected_operation_id=current['undo_operation_id'])['snapshot']
                self.assertEqual(undo['project']['clips'][0]['content']['text'], 'Edit 18')
                redone = invoke('redo', operation_id=uid(), expected_operation_id=undo['redo_operation_id'])['snapshot']
                self.assertEqual(redone['project'], final_project)
                path = root / '5000-clips-50-tracks.vvm.json'
                saved = invoke('save_project', path=str(path))['snapshot']
                self.assertFalse(saved['dirty'])
                self.assertEqual(json.loads(path.read_text()), final_project)
                client.close(); client = None
                host.process.kill(); host.process.wait(timeout=5); host.close()
                host = HostProcess(root); host.ready(); client = McpClient(root)
                reopened = invoke('open_project', path=str(path))['snapshot']
                self.assertEqual(reopened['session_id'], sid)
                self.assertEqual(reopened['project'], final_project)
                self.assertFalse(reopened['dirty'])
                report = {'scope': 'Real host + stdio MCP; not GUI latency or drag FPS',
                    'platform': platform.platform(), 'processor': platform.machine(), 'host': info,
                    'clips': 5000, 'tracks': 50, 'project_json_bytes': encoded_bytes,
                    'edit_ms': durations, 'median_ms': statistics.median(durations),
                    'p95_ms': sorted(durations)[math.ceil(len(durations)*.95)-1],
                    'save_restart_reopen_equal': True, 'undo_redo_equal': True}
                if output := os.environ.get('SUNGEKI_PERF_REPORT'):
                    Path(output).write_text(json.dumps(report, indent=2) + '\n')
            finally:
                if client: client.close()
                host.close()
