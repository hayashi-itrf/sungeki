"""Checkpoint restoration must not be mistaken for restoration of external files."""
import tempfile
import unittest
import uuid
from pathlib import Path

from support import HostProcess, McpClient
from test_image_probe import png
from test_mcp_editing import call


def uid():
    return str(uuid.uuid4())


class CheckpointAssetsTest(unittest.TestCase):
    def test_restored_checkpoint_reports_changed_and_missing_image_without_edits(self):
        with tempfile.TemporaryDirectory(prefix='vvm-checkpoint-assets-', dir='/tmp') as directory:
            root = Path(directory)
            host, client = HostProcess(root), None
            try:
                host.ready()
                client = McpClient(root)

                def invoke(tool, **args):
                    response = call(client, tool, **args)
                    self.assertFalse(response.get('isError'), response)
                    return response['structuredContent']

                source = root / 'checkpoint-image.png'
                original = png()
                changed = original[:-1] + bytes([original[-1] ^ 1])
                source.write_bytes(original)
                probe = invoke('probe_image', path=str(source))['probe']
                invoke('create_project', title='保存時点の外部素材変更・欠落検証')
                asset_id, track_id, clip_id, checkpoint_id = [uid() for _ in range(4)]
                asset = {'id': asset_id, 'name': '検証画像', 'source': None, 'credit': '自作試験素材',
                         'locator': {'type': 'absolute', 'path': str(source)},
                         'probe': {'type': 'image', 'info': probe}, 'content_hash': probe['sha256']}
                saved = invoke('apply_commands', operation_id=uid(), commands=[
                    {'type': 'register_asset', 'asset': asset},
                    {'type': 'add_track', 'track': {'id': track_id, 'name': '画像',
                                                  'kind': 'visual', 'visible': True, 'muted': False}},
                    {'type': 'add_clip', 'clip': {'id': clip_id, 'track_id': track_id, 'scene_id': None,
                        'start_us': 0, 'duration_us': 1000000,
                        'transform': {'x': 0, 'y': 0, 'width': 200, 'height': 300, 'opacity': 1},
                        'content': {'type': 'image', 'asset_id': asset_id}}},
                ])['snapshot']
                invoke('create_checkpoint', checkpoint_id=checkpoint_id, name='素材変更前',
                       expected_revision=saved['revision'])
                invoke('save_project', path=str(root / '保存時点の素材検証.vvm.json'))

                for mutation, code in [('same_size', 'ASSET_CHANGED'), ('grown', 'ASSET_CHANGED'),
                                       ('missing', 'ASSET_MISSING')]:
                    with self.subTest(mutation=mutation):
                        edited = invoke('apply_commands', operation_id=uid(), commands=[
                            {'type': 'rename_project', 'title': code}])['snapshot']
                        if mutation == 'same_size':
                            # Same-size byte change: the hash gate must reject it before decoding.
                            source.write_bytes(changed)
                        elif mutation == 'grown':
                            source.write_bytes(original + b'changed')
                        else:
                            source.unlink()
                        restored = invoke('restore_checkpoint', checkpoint_id=checkpoint_id,
                            operation_id=uid(), expected_revision=edited['revision'])['snapshot']
                        self.assertEqual(restored['project'], saved['project'])
                        self.assertGreater(restored['revision'], edited['revision'])
                        comparison = invoke('compare_checkpoints', checkpoint_id=checkpoint_id,
                            target={'type': 'current', 'expected_revision': restored['revision']})['comparison']
                        self.assertEqual(comparison['total_changes'], 0)
                        job = invoke('review_project', job_id=uid(), wait_ms=5000)['job']
                        self.assertEqual(job['phase'], 'succeeded')
                        diagnostics = job['report']['diagnostics']
                        self.assertIn(code, {issue['code'] for issue in diagnostics['issues']})
                        self.assertGreater(diagnostics['summary']['failed'], 0)
                        self.assertFalse(diagnostics['summary']['complete'])
                        self.assertEqual(invoke('get_project')['snapshot'], restored)
                        if source.exists():
                            self.assertEqual(source.read_bytes(), changed if mutation == 'same_size'
                                             else original + b'changed')
                        else:
                            self.assertEqual(code, 'ASSET_MISSING')
                        undone = invoke('undo', operation_id=uid(),
                            expected_operation_id=restored['undo_operation_id'])['snapshot']
                        self.assertEqual(undone['project'], edited['project'])
            finally:
                if client:
                    client.close()
                host.close()
