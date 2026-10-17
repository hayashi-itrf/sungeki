"""Restore keeps source identity; review diagnoses external changes separately.

PSD/video/VRM fixtures verify byte identity only, never decoding or visual quality.
"""
import hashlib
import json
import tempfile
import unittest
import uuid
import wave
from pathlib import Path

from support import HostProcess, McpClient
from test_mcp_editing import call


class CheckpointMediaSourcesTest(unittest.TestCase):
    def test_audio_sources(self):
        self.verify_sources('audio')

    def test_psd_sources(self):
        self.verify_sources('psd')

    def test_video_sources(self):
        self.verify_sources('video')

    def test_vrm_sources(self):
        self.verify_sources('vrm')

    def verify_sources(self, kind):
        uid = lambda: str(uuid.uuid4())
        with tempfile.TemporaryDirectory(prefix='vvm-cp-media-', dir='/tmp') as directory:
            root = Path(directory)
            host, client = HostProcess(root), None
            try:
                host.ready()
                client = McpClient(root)

                def invoke(tool, **args):
                    result = call(client, tool, **args)
                    self.assertFalse(result.get('isError'), result)
                    return result['structuredContent']

                source = root / f'source.{kind}'
                if kind == 'audio':
                    with wave.open(str(source), 'wb') as wav:
                        wav.setnchannels(1)
                        wav.setsampwidth(2)
                        wav.setframerate(24000)
                        wav.writeframes(bytes(48000))
                    probe = invoke('probe_audio', path=str(source))['probe']
                else:
                    source.write_bytes(bytes([42]) * 4096)
                    filename = 'vrm-assets-project.json' if kind == 'vrm' else f'{kind}-project.json'
                    fixture = Path(__file__).parents[1] / 'fixtures' / filename
                    probe = json.loads(fixture.read_text())['assets'][0]['probe']['info']
                    probe['sha256'] = hashlib.sha256(source.read_bytes()).hexdigest()
                    probe['byte_length'] = source.stat().st_size
                original = source.read_bytes()
                invoke('create_project', title=f'保存時点の{kind}原本検証')
                asset = {'id': uid(), 'name': f'{kind}原本', 'source': None, 'credit': '試験用',
                         'locator': {'type': 'absolute', 'path': str(source)},
                         'probe': {'type': kind, 'info': probe}, 'content_hash': probe['sha256']}
                saved = invoke('apply_commands', operation_id=uid(),
                    commands=[{'type': 'register_asset', 'asset': asset}])['snapshot']
                checkpoint = uid()
                invoke('create_checkpoint', checkpoint_id=checkpoint, name='変更前',
                       expected_revision=saved['revision'])
                invoke('save_project', path=str(root / f'{kind}原本の復元検証.vvm.json'))
                for mutation in ('changed', 'grown', 'missing'):
                    with self.subTest(kind=kind, mutation=mutation):
                        edited = invoke('apply_commands', operation_id=uid(), commands=[
                            {'type': 'remove_asset', 'asset_id': asset['id']}])['snapshot']
                        if mutation == 'changed':
                            source.write_bytes(original[:-1] + bytes([original[-1] ^ 1]))
                        elif mutation == 'grown':
                            source.write_bytes(original + b'changed')
                        else:
                            source.unlink()
                        restored = invoke('restore_checkpoint', checkpoint_id=checkpoint,
                            operation_id=uid(), expected_revision=edited['revision'])['snapshot']
                        self.assertEqual(restored['project'], saved['project'])
                        job = invoke('review_project', job_id=uid(), wait_ms=5000)['job']
                        self.assertEqual(job['phase'], 'succeeded')
                        expected = f'{kind.upper()}_SOURCE_' + ('MISSING' if mutation == 'missing' else 'CHANGED')
                        issues = job['report']['diagnostics']['issues']
                        self.assertTrue(any(i['code'] == expected and i['asset_id'] == asset['id']
                                            and i['severity'] == 'error' for i in issues), issues)
                        self.assertFalse(job['report']['diagnostics']['summary']['complete'])
                        self.assertEqual(invoke('get_project')['snapshot'], restored)
            finally:
                if client:
                    client.close()
                host.close()
