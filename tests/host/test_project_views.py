import tempfile
import unittest
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_checkpoints import uid


class ProjectViewsTest(unittest.TestCase):
    def test_filtered_pages_order_membership_and_revision_consistency(self):
        with tempfile.TemporaryDirectory(prefix='vvm-views-', dir='/tmp') as directory:
            root = Path(directory); host = HostProcess(root); clients = []
            try:
                host.ready(); a, b = McpClient(root), McpClient(root); clients.extend([a, b])
                created = call(a, 'create_project', title='Project views fixture')['structuredContent']['snapshot']
                sid = created['session_id']; call(b, 'attach_session', session_id=sid)
                tracks = [uid(), uid()]; scenes = [uid(), uid()]; clips = [uid() for _ in range(5)]; groups = [uid(), uid()]
                commands = [{'type': 'add_track', 'track': {'id': t, 'name': f'Track {i}', 'kind': 'visual', 'visible': True, 'muted': False}} for i, t in enumerate(tracks)]
                commands += [{'type': 'add_scene', 'scene': {'id': s, 'name': f'Scene {i}', 'start_us': i*200, 'duration_us': 100}} for i, s in enumerate(scenes)]
                for i, cid in enumerate(clips):
                    commands.append({'type': 'add_clip', 'clip': {'id': cid, 'track_id': tracks[i % 2], 'scene_id': scenes[i // 2] if i < 4 else None,
                        'start_us': (i // 2)*200 + (i % 2)*10, 'duration_us': 20,
                        'transform': {'x': 0, 'y': 0, 'width': 100, 'height': 100, 'opacity': 1},
                        'content': {'type': 'text', 'text': f'Clip {i}', 'font_size': 32, 'color': '#000000'}}})
                commands += [{'type': 'add_group', 'group': {'id': g, 'name': f'Group {i}', 'clip_ids': clips[i*2:i*2+2]}} for i, g in enumerate(groups)]
                original = call(a, 'apply_commands', operation_id=uid(), commands=commands)['structuredContent']['snapshot']
                def read(**args):
                    result = call(a, 'get_project', **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']['result']
                summary = read(view='summary')
                self.assertEqual(summary['revision'], 1)
                self.assertEqual(summary['data']['counts'], {'tracks': 2, 'scenes': 2, 'groups': 2, 'clips': 5, 'assets': 0, 'characters': 0})
                self.assertNotIn('clips', summary['data'])
                self.assertEqual(summary['data']['duration_us'], 420)
                first = read(view='scenes', limit=1)
                second = read(view='scenes', limit=1, offset=first['data']['next_offset'], expected_revision=first['revision'])
                self.assertEqual(first['data']['entries'][0]['scene']['id'], scenes[0])
                self.assertEqual(second['data']['entries'][0]['scene']['id'], scenes[1])
                self.assertEqual(first['data']['entries'][0]['clip_count'], 2)
                self.assertEqual(first['data']['entries'][0]['group_count'], 1)
                matched = read(view='scenes', clip_ids=[clips[2]])['data']
                self.assertEqual(matched['total'], 1)
                self.assertEqual(matched['entries'][0]['clip_count'], 2)
                filtered = read(view='timeline', scene_ids=[scenes[0]], clip_ids=[clips[1], clips[1], clips[2]])['data']
                self.assertEqual(filtered['total'], 1)
                entry = filtered['entries'][0]
                self.assertEqual(entry['clip']['id'], clips[1]); self.assertEqual(entry['group_id'], groups[0])
                self.assertEqual(entry['track_order'], 1); self.assertEqual(entry['clip_order'], 1)
                self.assertEqual([t['id'] for t in filtered['tracks']], [tracks[1]])
                self.assertEqual(read(view='timeline', clip_ids=[])['data']['total'], 0)
                self.assertEqual(read(view='timeline', clip_ids=[clips[4]])['data']['entries'][0]['group_id'], None)
                page_a = read(view='timeline', limit=2)
                page_b = read(view='timeline', limit=2, offset=2, expected_revision=1)
                page_c = read(view='timeline', limit=2, offset=4, expected_revision=1)
                combined = page_a['data']['entries'] + page_b['data']['entries'] + page_c['data']['entries']
                self.assertEqual([e['clip']['id'] for e in combined], clips)
                self.assertIsNone(page_c['data']['next_offset'])
                for args, code in [({'view': 'timeline', 'offset': 1}, 'VALIDATION_FAILED'),
                                   ({'view': 'scenes', 'scene_ids': [uid()]}, 'NOT_FOUND'),
                                   ({'view': 'timeline', 'clip_ids': [uid()]}, 'NOT_FOUND'),
                                   ({'view': 'timeline', 'offset': 6, 'expected_revision': 1}, 'VALIDATION_FAILED'),
                                   ({'view': 'timeline', 'limit': 0}, 'VALIDATION_FAILED'),
                                   ({'view': 'timeline', 'limit': 51}, 'VALIDATION_FAILED')]:
                    self.assertEqual(call(a, 'get_project', **args)['structuredContent']['code'], code)
                self.assertEqual(call(a, 'get_project', view='full')['structuredContent']['snapshot'], original)
                # A partial read updates this MCP client's observed revision for the next edit.
                call(b, 'get_project', view='summary')
                changed = call(b, 'apply_commands', operation_id=uid(), commands=[{'type': 'rename_project', 'title': 'Concurrent'}])
                self.assertFalse(changed.get('isError'))
                stale = call(a, 'get_project', view='timeline', offset=2, expected_revision=1)
                self.assertEqual(stale['structuredContent']['code'], 'CONFLICT')
                self.assertEqual(stale['structuredContent']['details']['current_revision'], '2')
                self.assertEqual(read(view='summary')['data']['title'], 'Concurrent')
            finally:
                for client in clients: client.close()
                host.close()
