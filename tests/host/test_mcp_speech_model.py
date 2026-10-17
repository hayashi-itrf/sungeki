import json
import tempfile
import unittest
import uuid
from pathlib import Path
from support import HostProcess, McpClient, connect, exchange, hello
from test_mcp_editing import call


class SpeechModelTest(unittest.TestCase):
    def test_speech_and_character_share_atomic_edit_history_and_save(self):
        with tempfile.TemporaryDirectory(prefix='vvm-speech-model-', dir='/tmp') as directory:
            root = Path(directory)
            host = HostProcess(root)
            client = None
            try:
                host.ready()
                with connect(root) as connection:
                    self.assertEqual(exchange(connection, hello(minor=21))['error']['code'], 'PROTOCOL_MISMATCH')
                client = McpClient(root)

                def ok(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                initial = ok('create_project', title='音声モデルの保存とUndo検証20260922')['snapshot']
                char_id, track_id, clip_id = [str(uuid.uuid4()) for _ in range(3)]
                character = dict(id=char_id, name='検証話者', credit='VOICEVOX:検証用',
                    subtitle_style=dict(font_size=64, color='#123456', layout=None, outline=None),
                    voice=dict(type='voicevox', engine_id='test-engine', core_version='0.16.0', settings=dict(
                        style_id=1, speed_scale=1, pitch_scale=0, intonation_scale=1, volume_scale=1,
                        pre_phoneme_length=0.1, post_phoneme_length=0.1, pause_length=None,
                        pause_length_scale=1, output_sampling_rate=24000, output_stereo=False,
                        enable_interrogative_upspeak=True)))
                clip = dict(id=clip_id, track_id=track_id, scene_id=None, start_us=0, duration_us=2000000,
                    transform=dict(x=100, y=200, width=600, height=100, opacity=1),
                    content=dict(type='speech', gain_db=None, audio=None, character_id=char_id, subtitle_text='表示する字幕', spoken_text='よみあげるぶんしょう'))
                commands = [dict(type='set_character', character=character),
                    dict(type='add_track', track=dict(id=track_id, name='発話', kind='audio', visible=True, muted=False)),
                    dict(type='add_clip', clip=clip)]
                operation = str(uuid.uuid4())
                result = ok('apply_commands', operation_id=operation, commands=commands)
                self.assertEqual(result['snapshot']['project']['clips'], [clip])
                self.assertEqual(result['snapshot']['project']['characters'], [character])
                self.assertEqual(ok('apply_commands', operation_id=operation, commands=commands)['receipt'], result['receipt'])
                error = call(client, 'apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    dict(type='rename_project', title='must rollback'), dict(type='remove_character', character_id=char_id)])
                self.assertTrue(error['isError'])
                self.assertEqual(ok('get_project')['snapshot'], result['snapshot'])
                path = root / '音声モデルの保存とUndo検証20260922.vvm.json'
                saved = ok('save_project', path=str(path))['snapshot']
                self.assertEqual(json.loads(path.read_text()), saved['project'])
                self.assertEqual(ok('undo', operation_id=str(uuid.uuid4()), expected_operation_id=operation)['snapshot']['project'], initial['project'])
                self.assertEqual(ok('redo', operation_id=str(uuid.uuid4()), expected_operation_id=operation)['snapshot']['project'], saved['project'])
                asset_id = str(uuid.uuid4())
                asset = dict(id=asset_id, name='音声参照の検証', source=None, credit='テスト',
                    locator=dict(type='absolute', path=str(root / 'missing.wav')), content_hash='a' * 64,
                    probe=dict(type='audio', info=dict(sha256='a' * 64, duration_us=10000,
                        sample_rate=24000, channels=1, frames=240, byte_length=524)))
                audio = dict(asset_id=asset_id, input_fingerprint='b' * 64, engine_version='1', lip_sync=None)
                referenced = ok('apply_commands', operation_id=str(uuid.uuid4()), commands=[
                    dict(type='register_asset', asset=asset), dict(type='set_speech_audio', clip_id=clip_id, audio=audio)])['snapshot']
                self.assertEqual(referenced['project']['clips'][0]['content']['audio'], audio)
                checkpoint_id = str(uuid.uuid4())
                ok('create_checkpoint', checkpoint_id=checkpoint_id, name='音声参照あり', expected_revision=referenced['revision'])
                ok('save_project')
                self.assertEqual(json.loads(path.read_text())['clips'][0]['content']['audio'], audio)
                changed = ok('apply_commands', operation_id=str(uuid.uuid4()), commands=[dict(
                    type='set_speech', clip_id=clip_id, character_id=char_id, subtitle_text='修正した字幕', spoken_text=None)])['snapshot']
                self.assertIsNone(changed['project']['clips'][0]['content']['spoken_text'])
                self.assertEqual(changed['project']['clips'][0]['duration_us'], 2000000)
                self.assertIsNone(changed['project']['clips'][0]['content']['audio'])
                self.assertEqual(changed['project']['assets'], [asset])
                undone = ok('undo', operation_id=str(uuid.uuid4()), expected_operation_id=changed['undo_operation_id'])['snapshot']
                self.assertEqual(undone['project'], referenced['project'])
                client.close(); client = None
                host.close()
                host = HostProcess(root); host.ready()
                client = McpClient(root)
                recovered = ok('attach_session', session_id=initial['session_id'])['snapshot']
                self.assertEqual(recovered['project'], referenced['project'])
                restored = ok('restore_checkpoint', checkpoint_id=checkpoint_id, operation_id=str(uuid.uuid4()),
                    expected_revision=recovered['revision'])['snapshot']
                self.assertEqual(restored['project']['clips'][0]['content']['audio'], audio)
                self.assertFalse(ok('get_capabilities')['capabilities']['speech'])
            finally:
                if client:
                    client.close()
                host.close()
