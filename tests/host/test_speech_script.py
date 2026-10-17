import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient
from test_mcp_editing import call


def uid(): return str(uuid.uuid4())


class SpeechScriptTest(unittest.TestCase):
    def test_mcp_script_preview_replay_conflict_undo_and_save(self):
        with tempfile.TemporaryDirectory(prefix='vvm-script-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                client = McpClient(root)
                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']
                initial = ok('create_project', title='台本の原子的追加確認')['snapshot']
                char, track, scene, second_char = uid(), uid(), uid(), uid()
                character = dict(id=char, name='検証話者', credit='VOICEVOX:検証用',
                    subtitle_style=dict(font_size=64, color='#123456'),
                    voice=dict(type='voicevox', engine_id='test-engine', core_version='0.16.0', settings=dict(
                        style_id=1, speed_scale=1, pitch_scale=0, intonation_scale=1, volume_scale=1,
                        pre_phoneme_length=0.1, post_phoneme_length=0.1, pause_length=None,
                        pause_length_scale=1, output_sampling_rate=24000, output_stereo=False,
                        enable_interrogative_upspeak=True)))
                commands = [dict(type='set_character', character=character),
                    dict(type='set_character', character=dict(character, id=second_char, name='二人目')),
                    dict(type='add_track', track=dict(id=track, name='台本', kind='audio', visible=True, muted=False)),
                    dict(type='add_scene', scene=dict(id=scene, name='台本シーン', start_us=0, duration_us=20000000)),
                    dict(type='append_speech_script', script=dict(character_id=char, track_id=track, scene_id=scene,
                        start_us=0, provisional_duration_us=5000000, gap_us=350000,
                        lines=[dict(clip_id=uid(), subtitle_text=t, spoken_text=None, character_id=second_char if i==2 else None) for i,t in enumerate(['一行目', '', '二行目'])]))]
                operation = uid()
                ok('apply_commands', operation_id=operation, commands=commands, dry_run=True)
                self.assertEqual(ok('get_project')['snapshot']['revision'], 0)
                result = ok('apply_commands', operation_id=operation, commands=commands)
                snap = result['snapshot']
                self.assertEqual([c['start_us'] for c in snap['project']['clips']], [0, 5350000])
                self.assertEqual([c['content']['character_id'] for c in snap['project']['clips']], [char, second_char])
                self.assertEqual(ok('apply_commands', operation_id=operation, commands=commands)['receipt'], result['receipt'])
                rejected = call(client, 'apply_commands', operation_id=uid(), expected_revision=0, commands=commands)
                self.assertTrue(rejected['isError'])
                path = root / '台本確認.vvm.json'
                ok('save_project', path=str(path))
                self.assertEqual(json.loads(path.read_text()), snap['project'])
                self.assertEqual(ok('undo', operation_id=uid(), expected_operation_id=operation)['snapshot']['project'], initial['project'])
                restored = ok('redo', operation_id=uid(), expected_operation_id=operation)['snapshot']
                self.assertEqual(restored['project'], snap['project'])
                # Real shared command/preview, with registered probe metadata (no synthesis claimed).
                asset = uid()
                clips = snap['project']['clips']
                ok('apply_commands', operation_id=uid(), commands=[dict(type='register_asset', asset=dict(
                    id=asset, name='layout fixture', source=None, credit='test',
                    locator=dict(type='absolute', path='/missing/layout.wav'), content_hash='a'*64,
                    probe=dict(type='audio', info=dict(sha256='a'*64, duration_us=1000000,
                        sample_rate=24000, channels=1, frames=24000, byte_length=48044)))),
                    *[dict(type='set_speech_audio', clip_id=c['id'], audio=dict(asset_id=asset,
                        input_fingerprint='b'*64, engine_version='test', lip_sync=None)) for c in clips]])
                before_layout = ok('get_project')['snapshot']
                layout = [dict(type='layout_speech', clip_ids=[c['id'] for c in clips], start_us=500000, gap_us=350000)]
                layout_op = uid()
                ok('apply_commands', operation_id=layout_op, commands=layout, dry_run=True)
                self.assertEqual(ok('get_project')['snapshot'], before_layout)
                result = ok('apply_commands', operation_id=layout_op, commands=layout)
                self.assertEqual([c['start_us'] for c in result['snapshot']['project']['clips']], [500000,1850000])
                self.assertEqual(ok('apply_commands', operation_id=layout_op, commands=layout)['receipt'], result['receipt'])
                self.assertEqual(ok('undo', operation_id=uid(), expected_operation_id=layout_op)['snapshot']['project'], before_layout['project'])
                snap = ok('redo', operation_id=uid(), expected_operation_id=layout_op)['snapshot']
                sid = snap['session_id']
                client.close(); client = None
                host.close(); host = HostProcess(root); host.ready(); client = McpClient(root)
                self.assertEqual(ok('attach_session', session_id=sid)['snapshot']['project'], snap['project'])
            finally:
                if client: client.close()
                host.close()
