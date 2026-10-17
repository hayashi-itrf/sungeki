import base64
import io
from pathlib import Path
import tempfile
import unittest

from PIL import Image
from psd_preview import preview
from test_psd_pose import PsdPoseTests


class PsdPreviewTests(unittest.TestCase):
    def test_selected_pose_and_original_catalog_are_separate(self):
        with tempfile.TemporaryDirectory() as tmp:
            source, digest = PsdPoseTests().fixture(Path(tmp))
            before = source.read_bytes()
            pose = dict(source_sha256=digest, selections=[dict(group='/0', option='/0/1')])
            result = preview(source, digest, pose)
            image = Image.open(io.BytesIO(base64.b64decode(result['png_base64'])))
            self.assertEqual(image.getpixel((4, 4)), (0, 0, 255, 255))
            self.assertEqual(image.getpixel((0, 0))[3], 0)
            self.assertTrue(result['probe']['layers'][1]['visible'])
            self.assertFalse(result['probe']['layers'][2]['visible'])
            self.assertEqual(source.read_bytes(), before)
            with self.assertRaises(ValueError):
                preview(source, '0'*64, pose)

    def test_large_canvas_is_scaled_and_aspect_ratio_preserved(self):
        from psd_tools import PSDImage
        from psd_tools.api.layers import PixelLayer
        import hashlib
        with tempfile.TemporaryDirectory() as tmp:
            source = Path(tmp)/'large.psd'
            psd = PSDImage.new('RGB', (2048, 1024))
            PixelLayer.frompil(Image.new('RGBA', (20, 20), 'red'), parent=psd, name='red')
            psd.save(source)
            digest = hashlib.sha256(source.read_bytes()).hexdigest()
            result = preview(source, digest, dict(source_sha256=digest, selections=[]))
            self.assertEqual((result['width'], result['height']), (1024, 512))
            self.assertEqual(result['probe']['width'], 2048)


if __name__ == '__main__':
    unittest.main()
