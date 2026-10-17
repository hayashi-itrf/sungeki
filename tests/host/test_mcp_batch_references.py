import copy
import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import ROOT, HostProcess, McpClient
from test_mcp_editing import call


class BatchReferenceTest(unittest.TestCase):
    def test_preview_pages_apply_save_restart_receipt_and_undo(self):
        with tempfile.TemporaryDirectory(prefix='vvm-refs-', dir='/tmp') as directory:
            root = Path(directory)
            host, client = HostProcess(root), None
            try:
                self.assertGreaterEqual(host.ready(seconds=30)['protocol_minor'], 143)
                client = McpClient(root)

                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                before = ok('create_project', title='同一バッチの参照とID復旧の検証')['snapshot']
                ok('save_project', path=str(root / '参照検証.vvm.json'))
                character = json.loads((ROOT / 'tests/fixtures/psd-speech-project.json').read_text())['characters'][0]
                character['id'] = '$person'
                character['name'] = '$visual'
                commands = [
                    dict(type='add_track', track=dict(id='$visual', name='$person', kind='visual', visible=True, muted=False)),
                    dict(type='set_character', character=character),
                    dict(type='add_track', track=dict(id='$voice', name='voice', kind='audio', visible=False, muted=False)),
                    dict(type='add_scene', scene=dict(id='$scene', name='本編', start_us=0, duration_us=4000000)),
                ]
                for i in range(4):
                    commands.append(dict(type='add_clip', clip=dict(id=f'$text{i}', track_id='$visual', scene_id='$scene',
                        start_us=i*1000000, duration_us=500000,
                        transform=dict(x=0, y=0, width=400, height=100, opacity=1),
                        content=dict(type='text', text='$person', font_size=24, color='#000000'))))
                commands.append(dict(type='add_group', group=dict(id='$group', name='まとまり', clip_ids=['$text0', '$text1'])))
                copies = {f'$text{i}': str(uuid.uuid4()) for i in range(4)}
                copy_scene, copy_group = str(uuid.uuid4()), str(uuid.uuid4())
                commands.append(dict(type='duplicate_scene', scene_id='$scene', new_scene_id=copy_scene,
                    start_us=4000000, clip_ids=copies, group_ids={'$group': copy_group}))
                commands.append(dict(type='append_speech_script', script=dict(character_id='$person', track_id='$voice',
                    scene_id=None, start_us=8000000, provisional_duration_us=1000000, gap_us=0,
                    lines=[dict(clip_id=str(uuid.uuid4()), character_id=None, subtitle_text='$person', spoken_text=None),
                           dict(clip_id=str(uuid.uuid4()), character_id='$person', subtitle_text='二行目', spoken_text='$voice')])))
                commands.append(dict(type='ripple', clip_ids=['$text3'], boundary_us=3000000, delta_us=-500000))
                commands.append(dict(type='set_text', clip_id='$text0', text='置換しない $visual'))
                args = dict(operation_id=str(uuid.uuid4()), expected_revision=0, commands=commands)
                first = ok('apply_commands', **args, dry_run=True, limit=2)['preview']
                second = ok('apply_commands', **args, dry_run=True, offset=first['next_offset'], limit=2)['preview']
                self.assertEqual(first['created_ids'], second['created_ids'])
                self.assertEqual(first['after_hash'], second['after_hash'])
                self.assertEqual(ok('get_project')['snapshot']['project'], before['project'])
                applied = ok('apply_commands', **args)
                report = applied['receipt']
                self.assertEqual(report['summary']['added'], 17)
                self.assertEqual(report['summary']['removed'], 0)
                self.assertEqual(report['summary']['modified'], 0)
                self.assertEqual(report['summary']['reordered_collections'], [])
                self.assertEqual(len(report['changed_ids']['clips']), 10)
                self.assertEqual(len(report['changed_ids']['groups']), 2)
                self.assertEqual(report['diagnostics']['summary']['passed'], 1)
                self.assertEqual(report['diagnostics']['summary']['not_run'], 5)
                self.assertFalse(report['diagnostics']['summary']['complete'])
                ids = applied['receipt']['created_ids']
                self.assertEqual(ids, first['created_ids'])
                self.assertEqual(len(applied['snapshot']['project']['clips']), 10)
                self.assertEqual(applied['snapshot']['project']['groups'][1]['id'], copy_group)
                self.assertTrue(all(c['scene_id'] == copy_scene for c in applied['snapshot']['project']['clips'][4:8]))
                project = applied['snapshot']['project']
                self.assertEqual(project['clips'][3]['start_us'], 2500000)
                self.assertEqual(project['clips'][7]['start_us'], 7000000)
                self.assertEqual(project['tracks'][0]['id'], ids['visual'])
                self.assertEqual(project['tracks'][0]['name'], '$person')
                self.assertEqual(project['characters'][0], dict(character, id=ids['person']))
                self.assertEqual(project['clips'][0]['content']['text'], '置換しない $visual')
                self.assertTrue(all(c['track_id'] == ids['visual'] for c in project['clips'][:8]))
                self.assertTrue(all(c['track_id'] == ids['voice'] for c in project['clips'][8:]))
                self.assertEqual(project['clips'][9]['content']['character_id'], ids['person'])
                self.assertEqual(project['clips'][9]['content']['spoken_text'], '$voice')
                bad = copy.deepcopy(commands)
                bad[-1]['clip_id'] = '$missing'
                failure = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), expected_revision=1, commands=bad)
                self.assertTrue(failure.get('isError'), failure)
                self.assertEqual(ok('get_project')['snapshot'], applied['snapshot'])
                client.close(); client = None
                host.close(); host = HostProcess(root); host.ready(seconds=30)
                client = McpClient(root)
                ok('attach_session', session_id=before['session_id'])
                retried = ok('apply_commands', **args)
                self.assertEqual(retried['receipt'], applied['receipt'])
                self.assertEqual(retried['snapshot']['project'], project)
                changed_args = dict(args, commands=commands[:-1])
                self.assertTrue(call(client, 'apply_commands', **changed_args).get('isError'))
                undone = ok('undo', operation_id=str(uuid.uuid4()), expected_revision=1,
                            expected_operation_id=args['operation_id'])['snapshot']
                self.assertEqual(undone['project'], before['project'])
                self.assertFalse(undone['dirty'])
                retried = ok('apply_commands', **args)
                self.assertEqual(retried['receipt'], applied['receipt'])
                self.assertEqual(retried['snapshot']['project'], before['project'])
            finally:
                if client: client.close()
                host.close()
