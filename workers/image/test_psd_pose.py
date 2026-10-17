import hashlib
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

from PIL import Image
from psd_tools import PSDImage
from psd_tools.api.layers import PixelLayer
from psd_pose import choose, inspect, load, render


class PsdPoseTests(unittest.TestCase):
    def fixture(self, root):
        psd = PSDImage.new('RGB', (12, 10))
        group = psd.create_group(name='!choice')
        red = PixelLayer.frompil(Image.new('RGBA', (4, 6), (255, 0, 0, 255)), parent=group, name='*red', left=3, top=2)
        blue = PixelLayer.frompil(Image.new('RGBA', (4, 6), (0, 0, 255, 255)), parent=group, name='*blue', left=3, top=2)
        blue.visible = False
        path = root / 'original.psd'
        psd.save(path)
        digest = hashlib.sha256(path.read_bytes()).hexdigest()
        return path, digest

    def test_selected_layers_override_saved_preview_without_resizing_or_flattening_alpha(self):
        with tempfile.TemporaryDirectory() as tmp:
            path, digest = self.fixture(Path(tmp))
            before = path.read_bytes()
            psd, layers, size = load(path, digest)
            image = render(psd, layers, [dict(group='/0', option='/0/1')])
            self.assertEqual(image.size, (12, 10))
            self.assertEqual(image.getpixel((4, 4)), (0, 0, 255, 255))
            self.assertEqual(image.getpixel((0, 0))[3], 0)
            self.assertEqual(image.getchannel('A').getbbox(), (3, 2, 7, 8))
            catalog = inspect(psd, layers, digest, size)
            self.assertEqual([l['path'] for l in catalog['layers']], ['/0', '/0/0', '/0/1'])
            self.assertFalse(layers['/0/0'].visible)
            self.assertTrue(layers['/0/1'].visible)
            again, entries, _ = load(path, digest)
            self.assertEqual(render(again, entries, [dict(group='/0', option='/0/1')]).tobytes(), image.tobytes())
            self.assertEqual(path.read_bytes(), before)

    def test_unknown_duplicate_foreign_and_hidden_group_selections_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path, digest = self.fixture(Path(tmp))
            for selections in [[dict(group='/9', option='/9/0')], [dict(group='/0', option='/0')],
                               [dict(group='/0', option='/0/1')]*2, [dict(group='/0', option='/0/1', extra=True)]]:
                psd, layers, _ = load(path, digest)
                with self.assertRaises(ValueError):
                    choose(layers, selections)
            psd, layers, _ = load(path, digest)
            layers['/0'].visible = False
            with self.assertRaises(ValueError):
                render(psd, layers, [dict(group='/0', option='/0/1')])
            with self.assertRaises(ValueError):
                load(path, '0'*64)

    def test_cli_records_exact_output_hash_and_refuses_overwrite(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            path, digest = self.fixture(root)
            output = root / 'pose.png'
            args = [sys.executable, str(Path(__file__).with_name('psd_pose.py')),
                    '--source', str(path), '--sha256', digest, '--output', str(output)]
            result = subprocess.run(args, check=True, capture_output=True, text=True)
            report = json.loads(result.stdout)
            before = output.read_bytes()
            self.assertEqual(report['output']['sha256'], hashlib.sha256(before).hexdigest())
            self.assertEqual(report['selections'], [])
            result = subprocess.run(args, capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            self.assertEqual(output.read_bytes(), before)

    def test_frame_worker_uses_original_psd_and_revalidates_catalog(self):
        from psd_pose import probe
        from render_frame import psd_layer
        with tempfile.TemporaryDirectory() as tmp:
            path, digest = self.fixture(Path(tmp))
            psd, entries, size = load(path, digest)
            catalog = probe(psd, entries, digest, size)
            resources = {'asset': dict(path=str(path), sha256=digest, probe=dict(type='psd', info=catalog))}
            primitive = dict(asset_id='asset', pose=dict(source_sha256=digest, selections=[dict(group='/0', option='/0/1')]))
            target = Image.new('RGBA', (12, 10))
            transform = dict(x=0, y=0, width=12, height=10)
            psd_layer(target, primitive, transform, resources)
            self.assertEqual(target.getpixel((4, 4)), (0, 0, 255, 255))
            self.assertEqual(target.getpixel((0, 0))[3], 0)
            catalog['layers'][0]['name'] = 'forged catalog'
            with self.assertRaisesRegex(ValueError, 'catalog differs'):
                psd_layer(target, primitive, transform, resources)


if __name__ == '__main__':
    unittest.main()
