import json
import tempfile
import unittest
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call
from test_checkpoints import uid


class SceneEditingTest(unittest.TestCase):
    def test_shared_scene_group_commands_save_restart_undo(self):
        with tempfile.TemporaryDirectory(prefix='vvm-scenes-', dir='/tmp') as directory:
            root = Path(directory); host = HostProcess(root); clients = []
            try:
                host.ready()
                a, b = McpClient(root), McpClient(root); clients.extend([a, b])
                initial = call(a, 'create_project', title='Scene and group persistence')['structuredContent']['snapshot']
                sid = initial['session_id']; call(b, 'attach_session', session_id=sid)
                scene, group, track_a, track_b, clip_a, clip_b = [uid() for _ in range(6)]
                commands = [{'type': 'add_scene', 'scene': {'id': scene, 'name': 'Intro', 'start_us': 100, 'duration_us': 100}}]
                for track in [track_a, track_b]:
                    commands.append({'type': 'add_track', 'track': {'id': track, 'name': 'Visual', 'kind': 'visual', 'visible': True, 'muted': False}})
                for clip, track, start in [(clip_a, track_a, 100), (clip_b, track_b, 120)]:
                    commands.append({'type': 'add_clip', 'clip': {'id': clip, 'track_id': track, 'scene_id': scene,
                        'start_us': start, 'duration_us': 30, 'transform': {'x': 0, 'y': 0, 'width': 100, 'height': 100, 'opacity': 1},
                        'content': {'type': 'text', 'text': 'Scene clip', 'font_size': 32, 'color': '#000000'}}})
                commands.append({'type': 'add_group', 'group': {'id': group, 'name': 'Pair', 'clip_ids': [clip_a, clip_b]}})
                def apply(*commands):
                    result = call(a, 'apply_commands', operation_id=uid(), commands=list(commands))
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']['snapshot']
                apply(*commands)
                moved = apply({'type': 'move_selection', 'scene_ids': [scene], 'group_ids': [group], 'clip_ids': [clip_a], 'delta_us': 50})
                self.assertEqual([c['start_us'] for c in moved['project']['clips']], [150, 170])
                group_moved = apply({'type': 'move_group', 'group_id': group, 'start_us': 160})
                expected = json.loads(json.dumps(moved['project']))
                for clip in expected['clips']:
                    clip['start_us'] += 10
                self.assertEqual(group_moved['project'], expected)
                group_undo = call(a, 'undo', operation_id=uid(),
                    expected_operation_id=group_moved['undo_operation_id'])['structuredContent']['snapshot']
                self.assertEqual(group_undo['project'], moved['project'])
                copy_scene, copy_group, copy_a, copy_b = [uid() for _ in range(4)]
                copied = apply({'type': 'duplicate_scene', 'scene_id': scene, 'new_scene_id': copy_scene, 'start_us': 500,
                                'clip_ids': {clip_a: copy_a, clip_b: copy_b}, 'group_ids': {group: copy_group}})
                self.assertEqual(copied['project']['groups'][1]['clip_ids'], [copy_a, copy_b])
                self.assertEqual([c['id'] for c in copied['project']['clips']],
                                 [clip_a, clip_b, copy_a, copy_b])
                self.assertEqual([c['track_id'] for c in copied['project']['clips']],
                                 [track_a, track_b, track_a, track_b])
                for original, duplicate in zip(copied['project']['clips'][:2], copied['project']['clips'][2:]):
                    self.assertEqual(original['content'], duplicate['content'])
                    self.assertEqual(original['transform'], duplicate['transform'])
                    self.assertEqual(original['duration_us'], duplicate['duration_us'])
                    self.assertEqual(duplicate['scene_id'], copy_scene)
                laid_out = apply({'type': 'layout_scenes', 'scene_ids': [copy_scene, scene], 'start_us': 0, 'gap_us': 20})
                self.assertEqual([c['start_us'] for c in laid_out['project']['clips']], [120, 140, 0, 20])
                # Earlier valid commands must roll back if a later membership or
                # identifier check fails. Check the entire snapshot, including undo.
                invalid_commands = [
                    {'type': 'duplicate_scene', 'scene_id': scene, 'new_scene_id': uid(),
                     'start_us': 800, 'clip_ids': {clip_a: copy_a, clip_b: uid()},
                     'group_ids': {group: uid()}},
                    {'type': 'duplicate_scene', 'scene_id': scene, 'new_scene_id': uid(),
                     'start_us': 800, 'clip_ids': {clip_a: uid()}, 'group_ids': {group: uid()}},
                    {'type': 'set_clip_scene', 'clip_id': clip_a, 'scene_id': uid()},
                    {'type': 'add_group', 'group': {'id': uid(), 'name': 'Missing member',
                                                  'clip_ids': [clip_a, uid()]}},
                ]
                for command in invalid_commands:
                    with self.subTest(command=command['type']):
                        rejected = call(a, 'apply_commands', operation_id=uid(), commands=[
                            {'type': 'rename_project', 'title': 'must not survive'}, command])
                        self.assertTrue(rejected.get('isError'), rejected)
                        self.assertEqual(call(a, 'get_project')['structuredContent']['snapshot'], laid_out)
                conflict = call(b, 'apply_commands', operation_id=uid(), commands=[{'type': 'delete_scene', 'scene_id': scene}])
                self.assertEqual(conflict['structuredContent']['code'], 'CONFLICT')
                self.assertEqual(call(b, 'get_project')['structuredContent']['snapshot'], laid_out)
                invalid = call(a, 'apply_commands', operation_id=uid(), commands=[{'type': 'move_scene', 'scene_id': scene, 'start_us': 50}])
                self.assertEqual(invalid['structuredContent']['code'], 'VALIDATION_FAILED')
                self.assertEqual(call(a, 'get_project')['structuredContent']['snapshot'], laid_out)
                path = str(root/'scene-group-persistence.vvm.json')
                saved = call(a, 'save_project', path=path)['structuredContent']['snapshot']
                self.assertFalse(saved['dirty'])
                self.assertEqual(json.loads(Path(path).read_text()), saved['project'])
                for client in clients: client.close()
                clients.clear(); host.process.kill(); host.process.wait(timeout=5); host.close()
                host = HostProcess(root); host.ready(); a = McpClient(root); clients.append(a)
                reopened = call(a, 'open_project', path=path)['structuredContent']['snapshot']
                self.assertEqual(reopened['project'], saved['project'])
                self.assertEqual(reopened['session_id'], sid)
                removed = apply({'type': 'delete_scene', 'scene_id': copy_scene})
                self.assertEqual(len(removed['project']['clips']), 2)
                self.assertEqual(len(removed['project']['groups']), 1)
                restored = call(a, 'undo', operation_id=uid(), expected_operation_id=removed['undo_operation_id'])['structuredContent']['snapshot']
                self.assertEqual(restored['project'], saved['project'])
                self.assertFalse(restored['dirty'])
                detached = apply({'type': 'ungroup_scene', 'scene_id': copy_scene})
                self.assertIsNone(detached['project']['clips'][2]['scene_id'])
                self.assertEqual(detached['project']['clips'][2]['start_us'], 0)
                self.assertEqual(len(detached['project']['groups']), 2)
                ungrouped = apply({'type': 'ungroup', 'group_id': copy_group})
                self.assertEqual(ungrouped['project']['clips'], detached['project']['clips'])
                self.assertEqual(ungrouped['project']['scenes'], detached['project']['scenes'])
                self.assertEqual([g['id'] for g in ungrouped['project']['groups']], [group])
                undone = call(a, 'undo', operation_id=uid(),
                              expected_operation_id=ungrouped['undo_operation_id'])['structuredContent']['snapshot']
                self.assertEqual(undone['project'], detached['project'])
                scene_undo = call(a, 'undo', operation_id=uid(),
                    expected_operation_id=undone['undo_operation_id'])['structuredContent']['snapshot']
                self.assertEqual(scene_undo['project'], saved['project'])
                self.assertFalse(scene_undo['dirty'])
            finally:
                for client in clients: client.close()
                host.close()
